import assert from "node:assert/strict";
import { mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "node:test";

import {
    changelogSection,
    nextVersion,
    updateCargoLock,
    updateCargoToml,
    updateChangelog,
    updatePackageJson,
} from "./bump-version.mjs";
import { collectProblems } from "./check-version.mjs";
import { ARCHES, assetName, compareVersions, expectedAssets, parseVersion } from "./lib.mjs";
import { downloadsJson, latestJson, parseMsiProperties, sha256Sums } from "./make-release.mjs";
import { aurFiles, homebrewCask, scoopManifest, wingetManifests } from "./manifests.mjs";
import { setPubkey, validatePubkey } from "./set-updater-pubkey.mjs";
import { findOne, stage } from "./stage-assets.mjs";

const fakeAssets = (version) =>
    Object.fromEntries(
        expectedAssets(version).map((asset, index) => [
            asset.name,
            { sha256: String(index).padStart(64, "a"), size: 1000 + index },
        ]),
    );

test("versions are plain X.Y.Z and compare numerically", () => {
    assert.deepEqual(parseVersion("1.20.3"), [1, 20, 3]);
    assert.throws(() => parseVersion("1.0.0-beta.1"));
    assert.equal(compareVersions("0.9.0", "0.10.0"), -1);
    assert.equal(nextVersion("0.1.9", "patch"), "0.1.10");
    assert.equal(nextVersion("0.1.9", "minor"), "0.2.0");
    assert.equal(nextVersion("0.1.9", "major"), "1.0.0");
    assert.equal(nextVersion("0.1.9", "2.0.0"), "2.0.0");
});

test("asset names follow the contract", () => {
    assert.equal(
        assetName("0.1.0", "linux", "arm64", "pkg.tar.zst"),
        "Coreverse-Launcher_0.1.0_linux_arm64.pkg.tar.zst",
    );
    assert.equal(expectedAssets("0.1.0").length, 12 * ARCHES.length);
});

test("bump edits only the version lines", () => {
    assert.match(updatePackageJson('{\n "version": "0.1.0",\n "x": 1\n}', "0.2.0"), /"0.2.0"/);
    const toml = '[workspace]\nmembers = []\n\n[workspace.package]\nversion = "0.1.0"\nedition = "2024"\n';
    assert.match(updateCargoToml(toml, "0.2.0"), /version = "0.2.0"\nedition/);
    const lock = 'name = "launcher-core"\nversion = "0.1.0"\n\nname = "serde"\nversion = "1.0.0"\n';
    const bumped = updateCargoLock(lock, "0.2.0", ["launcher-core"]);
    assert.match(bumped, /launcher-core"\nversion = "0.2.0"/);
    assert.match(bumped, /serde"\nversion = "1.0.0"/);
    assert.throws(() => updateCargoLock(lock, "0.2.0", ["missing"]));
});

test("the changelog gains a release section and compare links", () => {
    const text = "# Changelog\n\n## [Unreleased]\n\n### Added\n\n- A thing\n\n[Unreleased]: x\n";
    const result = updateChangelog(text, "0.2.0", "2026-11-08", "0.1.0");
    assert.match(result, /## \[Unreleased\]\n\n## \[0\.2\.0\] - 2026-11-08\n\n### Added/);
    assert.match(result, /\[Unreleased\]: .*compare\/v0\.2\.0\.\.\.HEAD\n\[0\.2\.0\]: .*v0\.1\.0\.\.\.v0\.2\.0/);
    assert.equal(changelogSection(result, "0.2.0"), "### Added\n\n- A thing");
    assert.equal(changelogSection(result, "9.9.9"), null);
});

test("staging picks the bundler files and renames them", () => {
    const root = mkdtempSync(path.join(tmpdir(), "stage-"));
    const bundle = path.join(root, "bundle");
    mkdirSync(path.join(bundle, "msi"), { recursive: true });
    writeFileSync(path.join(bundle, "msi", "Coreverse Launcher_0.1.0_x64_en-US.msi"), "msi");
    writeFileSync(path.join(bundle, "msi", "Coreverse Launcher_0.1.0_x64_en-US.msi.sig"), "sig");
    const staged = stage({
        os: "windows",
        arch: "x86_64",
        bundleDir: bundle,
        out: path.join(root, "out"),
        version: "0.1.0",
    });
    assert.deepEqual(staged.map((file) => path.basename(file)), [
        "Coreverse-Launcher_0.1.0_windows_x86_64.msi",
        "Coreverse-Launcher_0.1.0_windows_x86_64.msi.sig",
    ]);
    assert.throws(() => findOne(path.join(bundle, "msi"), ".exe"), /found 0/);
});

test("latest.json lists all six updater platforms", () => {
    const signatures = Object.fromEntries(
        expectedAssets("0.1.0")
            .filter((asset) => asset.format.endsWith("-sig"))
            .map((asset) => [asset.name.slice(0, -4), "SIG\n"]),
    );
    const json = latestJson({ version: "0.1.0", tag: "v0.1.0", notes: "n", date: new Date(0), signatures });
    assert.deepEqual(Object.keys(json.platforms).sort(), [
        "darwin-aarch64",
        "darwin-x86_64",
        "linux-aarch64",
        "linux-x86_64",
        "windows-aarch64",
        "windows-x86_64",
    ]);
    assert.equal(json.platforms["darwin-aarch64"].signature, "SIG");
    assert.match(json.platforms["windows-x86_64"].url, /download\/v0\.1\.0\/.*_windows_x86_64\.msi$/);
});

test("downloads.json offers only user facing files", () => {
    const assets = fakeAssets("0.1.0");
    const json = downloadsJson({ version: "0.1.0", tag: "v0.1.0", notes: "", date: new Date(0), assets });
    assert.equal(json.schema, 1);
    assert.equal(json.files.length, 7 * 2);
    assert.ok(json.files.every((file) => !file.name.endsWith(".sig") && !file.name.endsWith(".zip")));
    assert.ok(json.files.every((file) => file.sha256.length === 64 && file.size > 0));
    assert.match(json.install.linux.script, /install\.sh \| sh$/);
});

test("checksums are written in sha256sum format", () => {
    const text = sha256Sums({ b: { sha256: "2" }, a: { sha256: "1" } });
    assert.equal(text, "1  a\n2  b\n");
});

test("msiinfo output yields the product and upgrade codes", () => {
    const text = "Property\tValue\r\ns72\tl0\r\nProperty\tProperty\r\nProductCode\t{AAA}\r\nUpgradeCode\t{BBB}\r\nManufacturer\tCoreverse\r\n";
    assert.deepEqual(parseMsiProperties(text), { productCode: "{AAA}", upgradeCode: "{BBB}" });
});

test("package manager manifests carry the right hashes and urls", () => {
    const assets = fakeAssets("0.1.0");
    const input = { version: "0.1.0", tag: "v0.1.0", date: "2026-11-08", assets, msi: {} };
    const winget = wingetManifests(input);
    assert.equal(Object.keys(winget).length, 3);
    assert.match(winget["Coreverse.Launcher.installer.yaml"], /Architecture: arm64/);
    const scoop = JSON.parse(scoopManifest(input));
    assert.match(scoop.architecture.arm64.url, /windows_arm64\.zip$/);
    assert.match(scoop.autoupdate.architecture["64bit"].url, /\$\{version\}/);
    assert.match(homebrewCask(input), /auto_updates true/);
    const aur = aurFiles(input);
    assert.match(aur.PKGBUILD, /source_aarch64=/);
    assert.match(aur[".SRCINFO"], /pkgname = coreverse-launcher-bin/);
});

test("the updater key must be a Tauri public key", () => {
    const key = Buffer.from("untrusted comment: minisign public key: ABC\nRWQ...").toString("base64");
    assert.equal(validatePubkey(`${key}\n`), key);
    assert.throws(() => validatePubkey("hello"));
    assert.match(setPubkey('"pubkey": "REPLACE_WITH_X",', key), new RegExp(key));
});

test("this repository's version files agree", () => {
    const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..");
    const problems = collectProblems(root);
    assert.deepEqual(problems, []);
    assert.ok(readFileSync(path.join(root, "package.json"), "utf8").includes('"version"'));
});
