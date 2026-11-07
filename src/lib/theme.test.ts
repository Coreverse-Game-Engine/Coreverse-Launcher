import { describe, expect, it, vi } from "vitest";

import {
    THEME_STORAGE_KEY,
    applyTheme,
    isTheme,
    readStoredTheme,
    resolveTheme,
    storeTheme,
} from "@/lib/theme";

describe("theme storage", () => {
    it("recognises valid themes only", () => {
        expect(isTheme("dark")).toBe(true);
        expect(isTheme("purple")).toBe(false);
    });

    it("falls back to the system theme", () => {
        expect(readStoredTheme()).toBe("system");
        window.localStorage.setItem(THEME_STORAGE_KEY, "purple");
        expect(readStoredTheme()).toBe("system");
    });

    it("round-trips a stored theme", () => {
        storeTheme("dark");
        expect(readStoredTheme()).toBe("dark");
    });
});

describe("applying a theme", () => {
    it("resolves the system theme through matchMedia", () => {
        vi.spyOn(window, "matchMedia").mockReturnValue({ matches: true } as MediaQueryList);
        expect(resolveTheme("system")).toBe("dark");
        expect(resolveTheme("light")).toBe("light");
    });

    it("toggles the dark class and the colour scheme", () => {
        expect(applyTheme("dark")).toBe("dark");
        expect(document.documentElement).toHaveClass("dark");
        expect(document.documentElement.style.colorScheme).toBe("dark");

        expect(applyTheme("light")).toBe("light");
        expect(document.documentElement).not.toHaveClass("dark");
    });
});
