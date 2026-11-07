import { describe, expect, it } from "vitest";

import { formatBytes, percent } from "@/lib/format";

describe("formatBytes", () => {
    it("formats common sizes", () => {
        expect(formatBytes(0)).toBe("0 B");
        expect(formatBytes(1023)).toBe("1023 B");
        expect(formatBytes(1536)).toBe("1.5 KB");
        expect(formatBytes(480 * 1024 * 1024)).toBe("480 MB");
    });

    it("returns an empty string for invalid input", () => {
        expect(formatBytes(-1)).toBe("");
        expect(formatBytes(Number.NaN)).toBe("");
    });
});

describe("percent", () => {
    it("rounds down to whole percent and never exceeds 100", () => {
        expect(percent(50, 200)).toBe(25);
        expect(percent(1, 3)).toBe(33);
        expect(percent(300, 200)).toBe(100);
    });

    it("is null while the total is unknown", () => {
        expect(percent(10, 0)).toBeNull();
    });
});
