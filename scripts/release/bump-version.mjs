// Raises the version everywhere it lives: package.json (which tauri.conf.json points to),
// the Cargo workspace, the two workspace entries of Cargo.lock and CHANGELOG.md.
//
//   pnpm release:bump patch|minor|major|X.Y.Z [--dry-run]

import { readFileSync, writeFileSync } from "node:fs";
import path from "node:path";

import { REPO, ROOT, compareVersions, isMain, parseArgs, parseVersion } from "./lib.mjs";

export function nextVersion(current, request) {
    const [major, minor, patch] = parseVersion(current);
    switch (request) {
        case "major":
            return `${major + 1}.0.0`;
        case "minor":
            return `${major}.${minor + 1}.0`;
        case "patch":
            return `${major}.${minor}.${patch + 1}`;
        default:
            parseVersion(request);
            return request;
    }
}

function replaceOnce(text, pattern, replacement, what) {
    if (!pattern.test(text)) {
        throw new Error(`Could not find ${what}.`);
    }
    return text.replace(pattern, replacement);
}

export function updatePackageJson(text, version) {
    return replaceOnce(text, /("version":\s*")[^"]+(")/, `$1${version}$2`, "version in package.json");
}

export function updateCargoToml(text, version) {
    return replaceOnce(
        text,
        /(\[workspace\.package\][^[]*?\nversion = ")[^"]+(")/,
        `$1${version}$2`,
        "[workspace.package] version in Cargo.toml",
    );
}

export function updateCargoLock(text, version, names) {
    let result = text;
    for (const name of names) {
        result = replaceOnce(
            result,
            new RegExp(`(name = "${name}"\\r?\\nversion = ")[^"]+(")`),
            `$1${version}$2`,
            `${name} in Cargo.lock`,
        );
    }
    return result;
}

/** Turns the "Unreleased" section into a release and opens a fresh one. */
export function updateChangelog(text, version, date, previous, repo = REPO) {
    if (!/^## \[Unreleased\]/m.test(text)) {
        throw new Error('CHANGELOG.md has no "## [Unreleased]" section.');
    }
    let result = text.replace(
        /^## \[Unreleased\][^\S\r\n]*$/m,
        `## [Unreleased]\n\n## [${version}] - ${date}`,
    );
    const base = `https://github.com/${repo}`;
    const released =
        previous && previous !== version
            ? `[${version}]: ${base}/compare/v${previous}...v${version}`
            : `[${version}]: ${base}/releases/tag/v${version}`;
    const unreleased = `[Unreleased]: ${base}/compare/v${version}...HEAD`;
    if (/^\[Unreleased\]: .*$/m.test(result)) {
        result = result.replace(/^\[Unreleased\]: .*$/m, `${unreleased}\n${released}`);
    } else {
        result = `${result.trimEnd()}\n\n${unreleased}\n${released}\n`;
    }
    return result;
}

/** The text of one released section, without its heading. */
export function changelogSection(text, version) {
    const lines = text.split(/\r?\n/);
    const start = lines.findIndex((line) => line.startsWith(`## [${version}]`));
    if (start === -1) {
        return null;
    }
    const rest = lines.slice(start + 1);
    const end = rest.findIndex((line) => /^(## \[|\[[^\]]+\]: )/.test(line));
    return (end === -1 ? rest : rest.slice(0, end)).join("\n").trim();
}

function main() {
    const { values, positional } = parseArgs(process.argv.slice(2), ["dry-run", "force"]);
    const request = positional[0];
    if (!request) {
        throw new Error("Usage: pnpm release:bump patch|minor|major|X.Y.Z [--dry-run] [--force]");
    }

    const file = (name) => path.join(ROOT, name);
    const packageJson = readFileSync(file("package.json"), "utf8");
    const current = JSON.parse(packageJson).version;
    const version = nextVersion(current, request);
    if (compareVersions(version, current) < 0 && !values.force) {
        throw new Error(`${version} is lower than the current ${current}. Use --force if intended.`);
    }

    const date = new Date().toISOString().slice(0, 10);
    const changelog = readFileSync(file("CHANGELOG.md"), "utf8");
    const edits = {
        "package.json": updatePackageJson(packageJson, version),
        "Cargo.toml": updateCargoToml(readFileSync(file("Cargo.toml"), "utf8"), version),
        "Cargo.lock": updateCargoLock(readFileSync(file("Cargo.lock"), "utf8"), version, [
            "coreverse-launcher",
            "launcher-core",
        ]),
        "CHANGELOG.md": updateChangelog(changelog, version, date, current),
    };

    const section = changelogSection(
        updateChangelog(changelog, version, date, current),
        version,
    );
    if (!section) {
        console.warn("Warning: the Unreleased section of CHANGELOG.md is empty.");
    }

    for (const [name, content] of Object.entries(edits)) {
        if (!values["dry-run"]) {
            writeFileSync(file(name), content);
        }
        console.log(`${values["dry-run"] ? "would update" : "updated"} ${name}`);
    }
    console.log(`\n${current} -> ${version}. Next:`);
    console.log("  pnpm release:check");
    console.log(`  git commit -am "release: v${version}"`);
    console.log(`  git tag v${version} && git push origin main v${version}`);
}

if (isMain(import.meta.url)) {
    try {
        main();
    } catch (error) {
        console.error(error.message);
        process.exitCode = 1;
    }
}
