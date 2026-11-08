// Writes the updater's public key into src-tauri/tauri.conf.json.
//
//   pnpm tauri signer generate -w ~/.tauri/coreverse-launcher.key
//   pnpm release:pubkey ~/.tauri/coreverse-launcher.key.pub

import { existsSync, readFileSync, writeFileSync } from "node:fs";
import path from "node:path";

import { ROOT, isMain } from "./lib.mjs";

/** Accepts the contents of the `.pub` file (base64 of a minisign public key). */
export function validatePubkey(value) {
    const text = value.trim();
    let decoded = "";
    try {
        decoded = Buffer.from(text, "base64").toString("utf8");
    } catch {
        // Handled below.
    }
    if (!decoded.startsWith("untrusted comment:")) {
        throw new Error("That is not a Tauri updater public key (the contents of the .pub file).");
    }
    return text;
}

export function setPubkey(config, pubkey) {
    const pattern = /("pubkey":\s*")[^"]*(")/;
    if (!pattern.test(config)) {
        throw new Error("tauri.conf.json has no plugins.updater.pubkey entry.");
    }
    return config.replace(pattern, `$1${pubkey}$2`);
}

if (isMain(import.meta.url)) {
    try {
        const argument = process.argv[2];
        if (!argument) {
            throw new Error("Usage: pnpm release:pubkey <path to the .pub file or the key itself>");
        }
        const raw = existsSync(argument) ? readFileSync(argument, "utf8") : argument;
        const file = path.join(ROOT, "src-tauri", "tauri.conf.json");
        writeFileSync(file, setPubkey(readFileSync(file, "utf8"), validatePubkey(raw)));
        console.log("Updated plugins.updater.pubkey in src-tauri/tauri.conf.json.");
    } catch (error) {
        console.error(error.message);
        process.exitCode = 1;
    }
}
