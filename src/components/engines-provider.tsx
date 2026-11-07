import { useCallback, useEffect, useMemo, useState, type ReactNode } from "react";
import {
    CoreverseApiError,
    getLatestRelease,
    type Release,
} from "@Coreverse-Game-Engine/db-client";

import { isDbConfigured } from "@/lib/db";
import { EnginesContext, type EnginesContextValue, type UpdateCheck } from "@/lib/engines";
import {
    cancelInstall as cancelInstallCommand,
    describeError,
    getInstallStatus,
    getPlatform,
    installEngine,
    isLauncherError,
    listInstalledEngines,
    onInstallEvent,
    uninstallEngine,
    type InstallStatus,
    type InstalledEngine,
    type LauncherError,
    type PlatformInfo,
} from "@/lib/tauri";

function toLauncherError(error: unknown): LauncherError {
    return isLauncherError(error) ? error : { code: "unknown", message: describeError(error) };
}

/**
 * Keeps the installed engines, the latest published release and the running installation in one
 * place, so it survives navigating between pages. All state-changing work happens in Rust.
 */
export function EnginesProvider({ children }: { children: ReactNode }) {
    const [installed, setInstalled] = useState<InstalledEngine[]>([]);
    const [platform, setPlatform] = useState<PlatformInfo | null>(null);
    const [latest, setLatest] = useState<Release | null>(null);
    const [updateCheck, setUpdateCheck] = useState<UpdateCheck>("checking");
    const [progress, setProgress] = useState<InstallStatus | null>(null);
    const [error, setError] = useState<LauncherError | null>(null);

    const refreshInstalled = useCallback(async () => {
        try {
            setInstalled(await listInstalledEngines());
        } catch (caught) {
            console.error(`Could not list the installed engines: ${describeError(caught)}`);
        }
    }, []);

    const checkForUpdates = useCallback(async () => {
        if (!isDbConfigured()) {
            setUpdateCheck("unconfigured");
            return;
        }
        setUpdateCheck("checking");
        try {
            const response = await getLatestRelease();
            setLatest(response.status === 200 ? response.data : null);
            setUpdateCheck("ready");
        } catch (caught) {
            if (caught instanceof CoreverseApiError && caught.status === 404) {
                setLatest(null);
                setUpdateCheck("ready");
            } else {
                // A CoreverseApiError means the server answered; anything else (no response at
                // all) is treated as being offline. The installed engines keep working either way.
                setUpdateCheck(caught instanceof CoreverseApiError ? "error" : "offline");
            }
        }
    }, []);

    useEffect(() => {
        let disposed = false;
        let unlisten: (() => void) | undefined;

        void refreshInstalled();
        void checkForUpdates();
        getPlatform()
            .then((value) => {
                if (!disposed) {
                    setPlatform(value);
                }
            })
            .catch(() => undefined);
        // The UI may have been reloaded while an installation kept running in Rust.
        getInstallStatus()
            .then((status) => {
                if (!disposed && status) {
                    setProgress(status);
                }
            })
            .catch(() => undefined);

        onInstallEvent((event) => {
            switch (event.type) {
                case "progress":
                    setProgress({
                        version: event.version,
                        phase: event.phase,
                        done: event.done,
                        total: event.total,
                    });
                    break;
                case "finished":
                    setProgress(null);
                    void refreshInstalled();
                    break;
                case "cancelled":
                    setProgress(null);
                    break;
                case "failed":
                    setProgress(null);
                    setError(event.error);
                    break;
            }
        })
            .then((dispose) => {
                if (disposed) {
                    dispose();
                } else {
                    unlisten = dispose;
                }
            })
            .catch((caught: unknown) => {
                console.error(`Could not listen for install events: ${describeError(caught)}`);
            });

        return () => {
            disposed = true;
            unlisten?.();
        };
    }, [checkForUpdates, refreshInstalled]);

    const install = useCallback(
        async (version: string) => {
            setError(null);
            setProgress({ version, phase: "preparing", done: 0, total: 0 });
            try {
                await installEngine(version);
            } catch (caught) {
                const launcherError = toLauncherError(caught);
                // Cancelling is not a failure.
                if (launcherError.code !== "cancelled") {
                    setError(launcherError);
                }
            } finally {
                setProgress(null);
                await refreshInstalled();
            }
        },
        [refreshInstalled],
    );

    const cancelInstall = useCallback(async () => {
        try {
            await cancelInstallCommand();
        } catch (caught) {
            console.error(`Could not cancel the installation: ${describeError(caught)}`);
        }
    }, []);

    const uninstall = useCallback(
        async (version: string) => {
            setError(null);
            try {
                await uninstallEngine(version);
            } catch (caught) {
                setError(toLauncherError(caught));
            } finally {
                await refreshInstalled();
            }
        },
        [refreshInstalled],
    );

    const dismissError = useCallback(() => setError(null), []);

    const value = useMemo<EnginesContextValue>(
        () => ({
            installed,
            platform,
            latest,
            updateCheck,
            progress,
            error,
            dismissError,
            checkForUpdates,
            install,
            cancelInstall,
            uninstall,
        }),
        [
            installed,
            platform,
            latest,
            updateCheck,
            progress,
            error,
            dismissError,
            checkForUpdates,
            install,
            cancelInstall,
            uninstall,
        ],
    );

    return <EnginesContext.Provider value={value}>{children}</EnginesContext.Provider>;
}
