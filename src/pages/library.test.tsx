import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { EnginesContextValue } from "@/lib/engines";

const engines = vi.hoisted(() => ({ value: {} as EnginesContextValue }));

vi.mock("@/hooks/use-engines", () => ({ useEngines: () => engines.value }));

import { LibraryPage } from "@/pages/library";

const installedEngine = {
    version: "1.0.0",
    status: "stable",
    os: "linux",
    architecture: "x86_64",
    sha256: "a".repeat(64),
    sizeBytes: 2048,
    installedAt: "2026-01-01T00:00:00.000Z",
};

const latestRelease = {
    version: "2.0.0",
    status: "stable",
    release_date: "2026-02-01",
    release_notes_summary: "Faster everything",
    artifacts: [
        {
            os: "linux",
            architecture: "x86_64",
            download_url: "https://example.com/engine.zip",
            sha256: "b".repeat(64),
            size_bytes: 1024 * 1024,
        },
    ],
} as unknown as EnginesContextValue["latest"];

function makeEngines(overrides: Partial<EnginesContextValue> = {}): EnginesContextValue {
    return {
        installed: [],
        platform: { os: "linux", architecture: "x86_64" },
        latest: null,
        updateCheck: "ready",
        progress: null,
        error: null,
        dismissError: vi.fn(),
        checkForUpdates: vi.fn().mockResolvedValue(undefined),
        install: vi.fn().mockResolvedValue(undefined),
        cancelInstall: vi.fn().mockResolvedValue(undefined),
        uninstall: vi.fn().mockResolvedValue(undefined),
        ...overrides,
    };
}

describe("LibraryPage", () => {
    beforeEach(() => {
        engines.value = makeEngines();
    });

    it("explains that an update check is not possible offline but still lists engines", async () => {
        engines.value = makeEngines({ updateCheck: "offline", installed: [installedEngine] });
        render(<LibraryPage />);

        expect(await screen.findByText(/seem to be offline/i)).toBeInTheDocument();
        expect(screen.getByText(/Version 1\.0\.0/)).toBeInTheDocument();
    });

    it("offers the latest release and installs it on click", async () => {
        engines.value = makeEngines({ latest: latestRelease });
        render(<LibraryPage />);

        await userEvent.click(await screen.findByRole("button", { name: "Install 2.0.0" }));

        expect(engines.value.install).toHaveBeenCalledWith("2.0.0", { deletePrevious: false });
    });

    it("offers to delete the old version, unchecked by default", async () => {
        engines.value = makeEngines({ latest: latestRelease, installed: [installedEngine] });
        render(<LibraryPage />);

        const checkbox = await screen.findByRole("checkbox", {
            name: "Delete the old version (1.0.0) after installing",
        });
        expect(checkbox).not.toBeChecked();

        await userEvent.click(screen.getByRole("button", { name: "Install 2.0.0" }));
        expect(engines.value.install).toHaveBeenCalledWith("2.0.0", { deletePrevious: false });
    });

    it("installs with deletePrevious when the box is ticked", async () => {
        engines.value = makeEngines({ latest: latestRelease, installed: [installedEngine] });
        render(<LibraryPage />);

        await userEvent.click(await screen.findByRole("checkbox"));
        await userEvent.click(screen.getByRole("button", { name: "Install 2.0.0" }));

        expect(engines.value.install).toHaveBeenCalledWith("2.0.0", { deletePrevious: true });
    });

    it("does not offer to delete anything when no older version is installed", async () => {
        engines.value = makeEngines({ latest: latestRelease });
        render(<LibraryPage />);

        await screen.findByRole("button", { name: "Install 2.0.0" });
        expect(screen.queryByRole("checkbox")).not.toBeInTheDocument();
    });

    it("shows the phases of an incremental update", async () => {
        engines.value = makeEngines({
            latest: latestRelease,
            installed: [installedEngine],
            progress: { version: "2.0.0", phase: "patching", done: 1, total: 4 },
        });
        render(<LibraryPage />);

        expect(await screen.findByText(/Applying the update/)).toBeInTheDocument();
    });

    it("does not offer an install when the release has no build for this platform", async () => {
        engines.value = makeEngines({
            latest: latestRelease,
            platform: { os: "macos", architecture: "arm64" },
        });
        render(<LibraryPage />);

        expect(await screen.findByRole("button", { name: "Install 2.0.0" })).toBeDisabled();
        expect(screen.getByText(/no build for macos/i)).toBeInTheDocument();
    });

    it("shows download progress with a cancel button", async () => {
        engines.value = makeEngines({
            latest: latestRelease,
            progress: { version: "2.0.0", phase: "downloading", done: 50, total: 100 },
        });
        render(<LibraryPage />);

        expect(await screen.findByRole("progressbar")).toHaveAttribute("aria-valuenow", "50");
        await userEvent.click(screen.getByRole("button", { name: "Cancel" }));
        expect(engines.value.cancelInstall).toHaveBeenCalledOnce();
    });

    it("asks for confirmation before uninstalling", async () => {
        engines.value = makeEngines({ installed: [installedEngine] });
        render(<LibraryPage />);

        await userEvent.click(await screen.findByRole("button", { name: "Uninstall" }));
        expect(engines.value.uninstall).not.toHaveBeenCalled();

        await userEvent.click(screen.getByRole("button", { name: "Yes, uninstall" }));
        expect(engines.value.uninstall).toHaveBeenCalledWith("1.0.0");
    });

    it("shows a translated error for a known error code", async () => {
        engines.value = makeEngines({
            error: { code: "checksum_mismatch", message: "raw message" },
        });
        render(<LibraryPage />);

        expect(await screen.findByRole("alert")).toHaveTextContent(/does not match its checksum/i);
    });
});
