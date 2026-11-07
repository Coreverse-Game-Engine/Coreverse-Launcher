import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";

import { ThemeProvider } from "@/components/theme-provider";
import { SettingsPage } from "@/pages/settings";
import { THEME_STORAGE_KEY } from "@/lib/theme";

describe("SettingsPage", () => {
    it("switches the theme and remembers it", async () => {
        render(
            <ThemeProvider>
                <SettingsPage />
            </ThemeProvider>,
        );

        const dark = await screen.findByRole("button", { name: "Dark" });
        expect(dark).toHaveAttribute("aria-pressed", "false");

        await userEvent.click(dark);

        await waitFor(() => expect(document.documentElement).toHaveClass("dark"));
        expect(dark).toHaveAttribute("aria-pressed", "true");
        expect(window.localStorage.getItem(THEME_STORAGE_KEY)).toBe("dark");
    });
});
