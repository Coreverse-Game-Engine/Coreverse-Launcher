// Shared constants and helpers of the release scripts. Plain Node, no dependencies.

import { createHash } from "node:crypto";
import { createReadStream, readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

export const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..");

export const REPO = "Coreverse-Game-Engine/Coreverse-Launcher";
export const APP_NAME = "Coreverse Launcher";
export const PRODUCT = "Coreverse-Launcher";
export const WINGET_ID = "Coreverse.Launcher";
export const SCOOP_BUCKET_REPO = "Coreverse-Game-Engine/scoop-bucket";
export const BREW_TAP_REPO = "Coreverse-Game-Engine/homebrew-tap";
export const AUR_PACKAGE = "coreverse-launcher-bin";

/** Spelled like Coreverse DB spells them (`launcher_core::Platform`). */
export const ARCHES = ["x86_64", "arm64"];

/**
 * Every file one release must contain, per OS and architecture. `download` marks what the
 * Website offers to people; the rest is for the updater and the package managers.
 */
export const ASSET_KINDS = [
    { format: "msi", os: "windows", ext: "msi", download: true },
    { format: "msi-sig", os: "windows", ext: "msi.sig", download: false },
    { format: "portable", os: "windows", ext: "zip", download: false },
    { format: "dmg", os: "macos", ext: "dmg", download: true },
    { format: "pkg", os: "macos", ext: "pkg", download: true },
    { format: "app-archive", os: "macos", ext: "app.tar.gz", download: false },
    { format: "app-archive-sig", os: "macos", ext: "app.tar.gz.sig", download: false },
    { format: "appimage", os: "linux", ext: "AppImage", download: true },
    { format: "appimage-sig", os: "linux", ext: "AppImage.sig", download: false },
    { format: "deb", os: "linux", ext: "deb", download: true },
    { format: "rpm", os: "linux", ext: "rpm", download: true },
    { format: "pacman", os: "linux", ext: "pkg.tar.zst", download: true },
];

/** `Coreverse-Launcher_0.1.0_linux_arm64.deb` */
export function assetName(version, os, arch, ext) {
    return `${PRODUCT}_${version}_${os}_${arch}.${ext}`;
}

/** Every asset name a complete release has. */
export function expectedAssets(version) {
    const names = [];
    for (const kind of ASSET_KINDS) {
        for (const arch of ARCHES) {
            names.push({ ...kind, arch, name: assetName(version, kind.os, arch, kind.ext) });
        }
    }
    return names;
}

const SEMVER = /^(\d+)\.(\d+)\.(\d+)$/;

/** Only plain `X.Y.Z`: Windows MSI versions cannot carry text pre-release tags. */
export function parseVersion(text) {
    const match = SEMVER.exec(text);
    if (!match) {
        throw new Error(`"${text}" is not a plain X.Y.Z version.`);
    }
    return [Number(match[1]), Number(match[2]), Number(match[3])];
}

export function compareVersions(a, b) {
    const left = parseVersion(a);
    const right = parseVersion(b);
    for (let index = 0; index < 3; index += 1) {
        if (left[index] !== right[index]) {
            return left[index] < right[index] ? -1 : 1;
        }
    }
    return 0;
}

export function read(file, root = ROOT) {
    return readFileSync(path.join(root, file), "utf8");
}

export async function sha256File(file) {
    const hash = createHash("sha256");
    for await (const chunk of createReadStream(file)) {
        hash.update(chunk);
    }
    return hash.digest("hex");
}

/** Reads `--name value` and `--flag` arguments. */
export function parseArgs(argv, flags = []) {
    const values = {};
    const positional = [];
    for (let index = 0; index < argv.length; index += 1) {
        const argument = argv[index];
        if (!argument.startsWith("--")) {
            positional.push(argument);
        } else if (flags.includes(argument.slice(2))) {
            values[argument.slice(2)] = true;
        } else {
            const value = argv[index + 1];
            if (value === undefined) {
                throw new Error(`${argument} needs a value.`);
            }
            values[argument.slice(2)] = value;
            index += 1;
        }
    }
    return { values, positional };
}

/** True when the file runs as a script rather than being imported (by the tests). */
export function isMain(metaUrl) {
    return process.argv[1] !== undefined && pathToFileURL(process.argv[1]).href === metaUrl;
}
