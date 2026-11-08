import { render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const tauri = vi.hoisted(() => ({
    getAppInfo: vi.fn(),
    getInstallInfo: vi.fn(),
    checkForUpdate: vi.fn(),
    installUpdate: vi.fn(),
    restartApp: vi.fn(),
    onUpdateEvent: vi.fn(),
}));

vi.mock("@/lib/tauri", () => ({
    ...tauri,
    describeError: (error: unknown) => (error instanceof Error ? error.message : String(error)),
    isLauncherError: () => false,
}));

import { UpdatesProvider } from "@/components/updates-provider";
import { useUpdates } from "@/hooks/use-updates";

function Probe() {
    const { phase, update } = useUpdates();
    return (
        <p>
            {phase}:{update?.version ?? "-"}
        </p>
    );
}

function renderProvider() {
    return render(
        <UpdatesProvider>
            <Probe />
        </UpdatesProvider>,
    );
}

describe("UpdatesProvider", () => {
    beforeEach(() => {
        tauri.getAppInfo.mockResolvedValue({ name: "Coreverse Launcher", version: "0.1.0" });
        tauri.getInstallInfo.mockResolvedValue({ channel: "installer", selfUpdate: true });
        tauri.checkForUpdate.mockResolvedValue(null);
        tauri.onUpdateEvent.mockResolvedValue(() => undefined);
    });

    it("checks once at startup and reports an available update", async () => {
        tauri.checkForUpdate.mockResolvedValue({
            version: "0.2.0",
            currentVersion: "0.1.0",
            notes: null,
        });
        renderProvider();

        expect(await screen.findByText("available:0.2.0")).toBeInTheDocument();
        expect(tauri.checkForUpdate).toHaveBeenCalledTimes(1);
    });

    it("keeps a failed startup check out of the way", async () => {
        tauri.checkForUpdate.mockRejectedValue(new Error("offline"));
        vi.spyOn(console, "warn").mockImplementation(() => undefined);
        renderProvider();

        await waitFor(() => expect(tauri.checkForUpdate).toHaveBeenCalled());
        expect(await screen.findByText("idle:-")).toBeInTheDocument();
    });

    it("does not check when the installation cannot update itself", async () => {
        tauri.getInstallInfo.mockResolvedValue({ channel: "systemPackage", selfUpdate: false });
        renderProvider();

        await waitFor(() => expect(tauri.getInstallInfo).toHaveBeenCalled());
        expect(tauri.checkForUpdate).not.toHaveBeenCalled();
        expect(screen.getByText("idle:-")).toBeInTheDocument();
    });
});
