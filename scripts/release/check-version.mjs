// Checks that every place that carries the version agrees, and (with --release) that the
// repository is ready to be tagged.
//
//   pnpm release:check [--tag vX.Y.Z] [--release]

import { ROOT, REPO, isMain, parseArgs, read } from "./lib.mjs";
import { changelogSection } from "./bump-version.mjs";

const PLACEHOLDER = "REPLACE_WITH";

function lockVersion(lock, name) {
    const match = new RegExp(`name = "${name}"\\r?\\nversion = "([^"]+)"`).exec(lock);
    return match ? match[1] : null;
}

/** Returns a list of problems; empty means fine. */
export function collectProblems(root, { tag, release } = {}) {
    const problems = [];
    const packageVersion = JSON.parse(read("package.json", root)).version;
    const cargo = /\[workspace\.package\][^[]*?\nversion = "([^"]+)"/.exec(read("Cargo.toml", root));
    const lock = read("Cargo.lock", root);
    const tauri = JSON.parse(read("src-tauri/tauri.conf.json", root));

    const versions = {
        "package.json": packageVersion,
        "Cargo.toml [workspace.package]": cargo ? cargo[1] : null,
        "Cargo.lock coreverse-launcher": lockVersion(lock, "coreverse-launcher"),
        "Cargo.lock launcher-core": lockVersion(lock, "launcher-core"),
    };
    for (const [where, version] of Object.entries(versions)) {
        if (version !== packageVersion) {
            problems.push(`${where} is ${version ?? "missing"}, package.json says ${packageVersion}.`);
        }
    }
    if (tauri.version !== "../package.json") {
        problems.push('src-tauri/tauri.conf.json "version" must be "../package.json".');
    }
    if (tag && tag !== `v${packageVersion}`) {
        problems.push(`The tag is ${tag}, but the version is ${packageVersion} (expected v${packageVersion}).`);
    }

    if (release) {
        const updater = tauri.plugins?.updater;
        if (!updater?.pubkey || updater.pubkey.includes(PLACEHOLDER)) {
            problems.push("plugins.updater.pubkey is still a placeholder. Run pnpm release:pubkey.");
        }
        const endpoints = updater?.endpoints ?? [];
        if (!endpoints.some((url) => url.startsWith(`https://github.com/${REPO}/releases/`))) {
            problems.push(`plugins.updater.endpoints must point to ${REPO} releases.`);
        }
        const overlay = JSON.parse(read("src-tauri/tauri.release.conf.json", root));
        if (overlay.bundle?.createUpdaterArtifacts !== true) {
            problems.push("tauri.release.conf.json must set bundle.createUpdaterArtifacts.");
        }
        const env = read(".env.production", root);
        for (const name of ["PUBLIC_SUPABASE_URL", "PUBLIC_SUPABASE_ANON_KEY"]) {
            if (!new RegExp(`^${name}=\\S+`, "m").test(env)) {
                problems.push(`.env.production must set ${name}: release builds would not reach the DB.`);
            }
        }
        const section = changelogSection(read("CHANGELOG.md", root), packageVersion);
        if (!section) {
            problems.push(`CHANGELOG.md has no filled "## [${packageVersion}]" section.`);
        }
    }
    return problems;
}

if (isMain(import.meta.url)) {
    const { values } = parseArgs(process.argv.slice(2), ["release"]);
    const problems = collectProblems(ROOT, {
        tag: values.tag,
        release: values.release === true,
    });
    if (problems.length > 0) {
        for (const problem of problems) {
            console.error(`- ${problem}`);
        }
        process.exitCode = 1;
    } else {
        console.log("Versions are in sync.");
    }
}
