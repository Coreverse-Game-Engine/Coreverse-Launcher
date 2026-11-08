// Copies what `tauri build` produced under its canonical release names.
//
//   node scripts/release/stage-assets.mjs --os linux --arch x86_64 \
//       --bundle-dir target/release/bundle --out release-out

import { copyFileSync, existsSync, mkdirSync, readdirSync } from "node:fs";
import path from "node:path";

import { ROOT, assetName, isMain, parseArgs, parseVersion, read } from "./lib.mjs";

/** What each OS' bundler leaves behind: folder, file suffix and the canonical extension. */
export const BUNDLER_OUTPUT = {
    linux: [
        { dir: "deb", suffix: ".deb", ext: "deb" },
        { dir: "appimage", suffix: ".AppImage", ext: "AppImage" },
        { dir: "appimage", suffix: ".AppImage.sig", ext: "AppImage.sig" },
    ],
    windows: [
        { dir: "msi", suffix: ".msi", ext: "msi" },
        { dir: "msi", suffix: ".msi.sig", ext: "msi.sig" },
    ],
    macos: [
        { dir: "dmg", suffix: ".dmg", ext: "dmg" },
        { dir: "macos", suffix: ".app.tar.gz", ext: "app.tar.gz" },
        { dir: "macos", suffix: ".app.tar.gz.sig", ext: "app.tar.gz.sig" },
    ],
};

/** Exactly one file of the folder must end with `suffix`. */
export function findOne(dir, suffix) {
    const matches = existsSync(dir) ? readdirSync(dir).filter((name) => name.endsWith(suffix)) : [];
    if (matches.length !== 1) {
        throw new Error(`Expected one *${suffix} in ${dir}, found ${matches.length}.`);
    }
    return path.join(dir, matches[0]);
}

export function stage({ os, arch, bundleDir, out, version }) {
    const rules = BUNDLER_OUTPUT[os];
    if (!rules) {
        throw new Error(`Unknown --os "${os}" (linux, windows or macos).`);
    }
    mkdirSync(out, { recursive: true });
    const staged = [];
    for (const rule of rules) {
        const source = findOne(path.join(bundleDir, rule.dir), rule.suffix);
        const target = path.join(out, assetName(version, os, arch, rule.ext));
        copyFileSync(source, target);
        staged.push(target);
    }
    return staged;
}

if (isMain(import.meta.url)) {
    try {
        const { values } = parseArgs(process.argv.slice(2));
        const version = JSON.parse(read("package.json")).version;
        parseVersion(version);
        for (const key of ["os", "arch", "bundle-dir", "out"]) {
            if (!values[key]) {
                throw new Error(`--${key} is required.`);
            }
        }
        const files = stage({
            os: values.os,
            arch: values.arch,
            bundleDir: path.resolve(ROOT, values["bundle-dir"]),
            out: path.resolve(ROOT, values.out),
            version,
        });
        for (const file of files) {
            console.log(`staged ${path.basename(file)}`);
        }
    } catch (error) {
        console.error(error.message);
        process.exitCode = 1;
    }
}
