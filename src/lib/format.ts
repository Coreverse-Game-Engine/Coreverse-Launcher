const UNITS = ["B", "KB", "MB", "GB", "TB"] as const;

/** `1536` -> `1.5 KB`. */
export function formatBytes(bytes: number): string {
    if (!Number.isFinite(bytes) || bytes < 0) {
        return "";
    }
    let value = bytes;
    let unit = 0;
    while (value >= 1024 && unit < UNITS.length - 1) {
        value /= 1024;
        unit += 1;
    }
    const digits = unit === 0 || value >= 100 ? 0 : 1;
    return `${value.toFixed(digits)} ${UNITS[unit]}`;
}

/** Whole percent of `done / total`, or `null` while the total is not known. */
export function percent(done: number, total: number): number | null {
    if (total <= 0) {
        return null;
    }
    return Math.min(100, Math.floor((done / total) * 100));
}
