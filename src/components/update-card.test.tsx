import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { UpdatesContextValue } from "@/lib/updates";
import { makeUpdates } from "@/test/updates";

const updates = vi.hoisted(() => ({ value: {} as UpdatesContextValue }));

vi.mock("@/hooks/use-updates", () => ({ useUpdates: () => updates.value }));

import { UpdateCard } from "@/components/update-card";

const available = { version: "0.2.0", currentVersion: "0.1.0", notes: "Faster start" };

describe("UpdateCard", () => {
    beforeEach(() => {
        updates.value = makeUpdates();
    });

    it("shows the installed version and checks on request", async () => {
        render(<UpdateCard />);

        expect(screen.getByText("Installed version: 0.1.0")).toBeInTheDocument();
        await userEvent.click(screen.getByRole("button", { name: "Check for updates" }));

        expect(updates.value.check).toHaveBeenCalled();
    });

    it("offers an available update and installs it on click", async () => {
        updates.value = makeUpdates({ phase: "available", update: available });
        render(<UpdateCard />);

        expect(screen.getByText("Version 0.2.0 is available.")).toBeInTheDocument();
        expect(screen.getByText("Faster start")).toBeInTheDocument();
        await userEvent.click(screen.getByRole("button", { name: "Download and install" }));

        expect(updates.value.install).toHaveBeenCalled();
    });

    it("shows the download progress", () => {
        updates.value = makeUpdates({
            phase: "downloading",
            update: available,
            progress: { downloaded: 512, total: 1024 },
        });
        render(<UpdateCard />);

        expect(screen.getByRole("progressbar")).toHaveAttribute("aria-valuenow", "50");
    });

    it("asks for a restart once the update is installed", async () => {
        updates.value = makeUpdates({ phase: "ready", update: available });
        render(<UpdateCard />);

        await userEvent.click(screen.getByRole("button", { name: "Restart now" }));

        expect(updates.value.restart).toHaveBeenCalled();
    });

    it("tells Scoop users to update with Scoop instead of offering an update", () => {
        updates.value = makeUpdates({ installInfo: { channel: "scoop", selfUpdate: false } });
        render(<UpdateCard />);

        expect(screen.getByText(/scoop update coreverse-launcher/)).toBeInTheDocument();
        expect(screen.queryByRole("button", { name: "Check for updates" })).toBeNull();
    });

    it("shows a failed check", () => {
        updates.value = makeUpdates({
            phase: "error",
            error: { code: "update_failed", message: "boom" },
        });
        render(<UpdateCard />);

        expect(screen.getByRole("alert")).toHaveTextContent(/could not be checked or installed/);
    });
});
