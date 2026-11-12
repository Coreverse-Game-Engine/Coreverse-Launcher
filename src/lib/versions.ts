import type { InstalledEngine, PlatformInfo } from "@/lib/tauri";

type Parts = [number, number, number];

/** Parses a plain `MAJOR.MINOR.PATCH` version, the only form Coreverse DB publishes. */
export function parseVersion(version: string): Parts | null {
    const match = /^(\d+)\.(\d+)\.(\d+)$/.exec(version);
    return match ? [Number(match[1]), Number(match[2]), Number(match[3])] : null;
}

function compare(a: Parts, b: Parts): number {
    return a[0] - b[0] || a[1] - b[1] || a[2] - b[2];
}

/**
 * The installed version a new install of `target` would be built from (and that "delete the old
 * version" removes): the newest one that is older than `target` and built for this platform.
 * The launcher picks it the same way in `launcher_core::engine::delta::choose_base`.
 */
export function previousVersion(
    installed: InstalledEngine[],
    target: string,
    platform: PlatformInfo | null,
): string | null {
    const targetParts = parseVersion(target);
    if (!targetParts || !platform) {
        return null;
    }
    let best: { version: string; parts: Parts } | null = null;
    for (const engine of installed) {
        if (engine.os !== platform.os || engine.architecture !== platform.architecture) {
            continue;
        }
        const parts = parseVersion(engine.version);
        if (!parts || compare(parts, targetParts) >= 0) {
            continue;
        }
        if (!best || compare(parts, best.parts) > 0) {
            best = { version: engine.version, parts };
        }
    }
    return best ? best.version : null;
}
