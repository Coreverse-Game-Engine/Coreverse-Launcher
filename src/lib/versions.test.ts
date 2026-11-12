import { describe, expect, it } from "vitest";

import type { InstalledEngine } from "@/lib/tauri";
import { parseVersion, previousVersion } from "@/lib/versions";

function engine(version: string, os = "linux", architecture = "x86_64"): InstalledEngine {
    return {
        version,
        status: "stable",
        os,
        architecture,
        sha256: "a".repeat(64),
        sizeBytes: 1,
        installedAt: "2026-01-01T00:00:00.000Z",
    };
}

const linux = { os: "linux", architecture: "x86_64" };

describe("parseVersion", () => {
    it("reads MAJOR.MINOR.PATCH and nothing else", () => {
        expect(parseVersion("1.2.3")).toEqual([1, 2, 3]);
        expect(parseVersion("1.2")).toBeNull();
        expect(parseVersion("v1.2.3")).toBeNull();
        expect(parseVersion("1.2.3-beta")).toBeNull();
    });
});

describe("previousVersion", () => {
    it("is the newest older version, compared numerically", () => {
        const installed = [engine("1.0.0"), engine("1.10.0"), engine("1.4.0"), engine("2.0.0")];
        expect(previousVersion(installed, "1.12.0", linux)).toBe("1.10.0");
    });

    it("ignores other platforms and versions that are not older", () => {
        const installed = [engine("1.5.0", "macos", "arm64"), engine("2.0.0")];
        expect(previousVersion(installed, "2.0.0", linux)).toBeNull();
        expect(previousVersion(installed, "1.0.0", linux)).toBeNull();
    });

    it("is null without a platform or with nothing installed", () => {
        expect(previousVersion([engine("1.0.0")], "2.0.0", null)).toBeNull();
        expect(previousVersion([], "2.0.0", linux)).toBeNull();
    });
});
