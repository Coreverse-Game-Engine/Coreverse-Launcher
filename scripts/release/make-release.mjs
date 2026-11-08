// Turns the staged build outputs of one release into everything the release needs besides
// the builds themselves: SHA256SUMS, latest.json (updater), downloads.json (Website), the
// release notes and the package manager manifests.
//
//   node scripts/release/make-release.mjs --assets release-assets --tag v0.1.0 --out release-meta

import { execFileSync } from "node:child_process";
import { copyFileSync, mkdirSync, readFileSync, readdirSync, statSync, writeFileSync } from "node:fs";
import path from "node:path";

import { changelogSection } from "./bump-version.mjs";
import {
    ASSET_KINDS,
    ARCHES,
    REPO,
    ROOT,
    WINGET_ID,
    BREW_TAP_REPO,
    SCOOP_BUCKET_REPO,
    AUR_PACKAGE,
    assetName,
    expectedAssets,
    isMain,
    parseArgs,
    parseVersion,
    sha256File,
} from "./lib.mjs";
import { aurFiles, homebrewCask, scoopManifest, wingetManifests } from "./manifests.mjs";

/** The updater's platform names and the asset each one downloads. */
const UPDATER_PLATFORMS = [
    ["darwin-x86_64", "macos", "x86_64", "app.tar.gz"],
    ["darwin-aarch64", "macos", "arm64", "app.tar.gz"],
    ["windows-x86_64", "windows", "x86_64", "msi"],
    ["windows-aarch64", "windows", "arm64", "msi"],
    ["linux-x86_64", "linux", "x86_64", "AppImage"],
    ["linux-aarch64", "linux", "arm64", "AppImage"],
];

const LABELS = {
    msi: "Windows installer (MSI)",
    dmg: "macOS disk image (DMG)",
    pkg: "macOS installer (PKG)",
    appimage: "Linux AppImage",
    deb: "Debian package (.deb)",
    rpm: "RPM package (.rpm)",
    pacman: "Arch package (.pkg.tar.zst)",
};

const DISTROS = {
    deb: ["Debian", "Ubuntu", "Linux Mint", "Pop!_OS"],
    rpm: ["Fedora", "openSUSE"],
    pacman: ["Arch Linux", "CachyOS", "Manjaro", "EndeavourOS"],
    appimage: ["Any distribution"],
};

export function releaseUrl(tag, name) {
    return `https://github.com/${REPO}/releases/download/${tag}/${name}`;
}

export function sha256Sums(assets) {
    return `${Object.keys(assets)
        .sort()
        .map((name) => `${assets[name].sha256}  ${name}`)
        .join("\n")}\n`;
}

export function latestJson({ version, tag, notes, date, signatures }) {
    const platforms = {};
    for (const [key, os, arch, ext] of UPDATER_PLATFORMS) {
        const name = assetName(version, os, arch, ext);
        platforms[key] = { signature: signatures[name].trim(), url: releaseUrl(tag, name) };
    }
    return { version, notes, pub_date: date.toISOString(), platforms };
}

/** What the Website's download panel reads. See docs/distribution.md. */
export function downloadsJson({ version, tag, notes, date, assets }) {
    const files = [];
    for (const kind of ASSET_KINDS.filter((item) => item.download)) {
        for (const arch of ARCHES) {
            const name = assetName(version, kind.os, arch, kind.ext);
            files.push({
                os: kind.os,
                arch,
                format: kind.format,
                label: LABELS[kind.format],
                ...(DISTROS[kind.format] ? { distros: DISTROS[kind.format] } : {}),
                name,
                url: releaseUrl(tag, name),
                size: assets[name].size,
                sha256: assets[name].sha256,
            });
        }
    }
    const releaseBase = `https://github.com/${REPO}/releases`;
    return {
        schema: 1,
        product: "coreverse-launcher",
        version,
        tag,
        releaseDate: date.toISOString(),
        releaseUrl: `${releaseBase}/tag/${tag}`,
        notes,
        checksums: `${releaseBase}/download/${tag}/SHA256SUMS`,
        files,
        install: {
            windows: {
                winget: `winget install --id ${WINGET_ID} --exact`,
                scoop: [
                    `scoop bucket add coreverse https://github.com/${SCOOP_BUCKET_REPO}`,
                    "scoop install coreverse-launcher",
                ],
            },
            macos: {
                homebrew: [
                    `brew tap ${BREW_TAP_REPO.replace("homebrew-", "")}`,
                    "brew install --cask coreverse-launcher",
                ],
            },
            linux: {
                script: `curl -fsSL ${releaseBase}/latest/download/install.sh | sh`,
                aur: `yay -S ${AUR_PACKAGE}`,
            },
        },
    };
}

/** `msiinfo` (msitools) prints the Property table as tab separated rows. */
export function parseMsiProperties(text) {
    const wanted = new Set(["ProductCode", "UpgradeCode"]);
    const found = {};
    for (const line of text.split(/\r?\n/)) {
        const [key, value] = line.split("\t");
        if (wanted.has(key) && value) {
            found[key === "ProductCode" ? "productCode" : "upgradeCode"] = value.trim();
        }
    }
    return found;
}

function readMsiCodes(file) {
    try {
        return parseMsiProperties(
            execFileSync("msiinfo", ["export", file, "Property"], { encoding: "utf8" }),
        );
    } catch {
        console.warn(`Warning: could not read the product code of ${path.basename(file)}.`);
        return {};
    }
}

function writeAll(dir, files) {
    mkdirSync(dir, { recursive: true });
    for (const [name, content] of Object.entries(files)) {
        writeFileSync(path.join(dir, name), content);
    }
}

async function main() {
    const { values } = parseArgs(process.argv.slice(2));
    for (const key of ["assets", "tag", "out"]) {
        if (!values[key]) {
            throw new Error(`--${key} is required.`);
        }
    }
    const version = values.tag.replace(/^v/, "");
    parseVersion(version);
    const tag = `v${version}`;
    const assetsDir = path.resolve(ROOT, values.assets);
    const out = path.resolve(ROOT, values.out);

    const present = new Set(readdirSync(assetsDir));
    const missing = expectedAssets(version).filter((asset) => !present.has(asset.name));
    if (missing.length > 0) {
        throw new Error(`Missing release files:\n${missing.map((a) => `  ${a.name}`).join("\n")}`);
    }

    const assets = {};
    const signatures = {};
    for (const name of [...present].sort()) {
        const file = path.join(assetsDir, name);
        assets[name] = { sha256: await sha256File(file), size: statSync(file).size };
        if (name.endsWith(".sig")) {
            signatures[name.slice(0, -4)] = readFileSync(file, "utf8");
        }
    }

    const msi = {};
    for (const arch of ARCHES) {
        msi[arch] = readMsiCodes(path.join(assetsDir, assetName(version, "windows", arch, "msi")));
    }

    const changelog = readFileSync(path.join(ROOT, "CHANGELOG.md"), "utf8");
    const notes = changelogSection(changelog, version) ?? "";
    const date = new Date();
    const release = { version, tag, notes, date, assets, signatures };
    const manifestInput = { version, tag, date: date.toISOString().slice(0, 10), assets, msi };

    writeAll(out, {
        SHA256SUMS: sha256Sums(assets),
        "latest.json": `${JSON.stringify(latestJson(release), null, 2)}\n`,
        "downloads.json": `${JSON.stringify(downloadsJson(release), null, 2)}\n`,
        "release-notes.md": `${notes}\n`,
    });
    copyFileSync(path.join(ROOT, "scripts", "install", "install.sh"), path.join(out, "install.sh"));

    const manifests = path.join(out, "manifests");
    writeAll(path.join(manifests, "winget"), wingetManifests(manifestInput));
    writeAll(path.join(manifests, "scoop"), { "coreverse-launcher.json": scoopManifest(manifestInput) });
    writeAll(path.join(manifests, "homebrew"), { "coreverse-launcher.rb": homebrewCask(manifestInput) });
    writeAll(path.join(manifests, "aur"), aurFiles(manifestInput));

    console.log(`Release ${tag}: ${Object.keys(assets).length} files checked, metadata in ${out}`);
}

if (isMain(import.meta.url)) {
    main().catch((error) => {
        console.error(error.message);
        process.exitCode = 1;
    });
}
