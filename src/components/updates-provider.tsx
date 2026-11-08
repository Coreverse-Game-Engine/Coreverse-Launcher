import { useCallback, useEffect, useMemo, useState, type ReactNode } from "react";

import {
    checkForUpdate,
    describeError,
    getAppInfo,
    getInstallInfo,
    installUpdate,
    isLauncherError,
    onUpdateEvent,
    restartApp,
    type InstallInfo,
    type LauncherError,
    type UpdateInfo,
} from "@/lib/tauri";
import {
    UpdatesContext,
    type UpdatePhase,
    type UpdateProgress,
    type UpdatesContextValue,
} from "@/lib/updates";

function toLauncherError(error: unknown): LauncherError {
    return isLauncherError(error) ? error : { code: "unknown", message: describeError(error) };
}

/**
 * Looks for a new launcher version once at startup and keeps the result in one place, so it
 * survives navigating between pages. Downloading and installing happen in Rust.
 */
export function UpdatesProvider({ children }: { children: ReactNode }) {
    const [appVersion, setAppVersion] = useState<string | null>(null);
    const [installInfo, setInstallInfo] = useState<InstallInfo | null>(null);
    const [phase, setPhase] = useState<UpdatePhase>("idle");
    const [update, setUpdate] = useState<UpdateInfo | null>(null);
    const [progress, setProgress] = useState<UpdateProgress | null>(null);
    const [error, setError] = useState<LauncherError | null>(null);

    const check = useCallback(async (options?: { silent?: boolean }) => {
        const silent = options?.silent === true;
        setError(null);
        setPhase("checking");
        try {
            const found = await checkForUpdate();
            setUpdate(found);
            setPhase(found ? "available" : "upToDate");
        } catch (caught) {
            if (silent) {
                console.warn(`Could not check for updates: ${describeError(caught)}`);
                setPhase("idle");
            } else {
                setError(toLauncherError(caught));
                setPhase("error");
            }
        }
    }, []);

    const install = useCallback(async () => {
        setError(null);
        setProgress({ downloaded: 0, total: null });
        setPhase("downloading");
        try {
            await installUpdate();
            // On Windows the installer closes the launcher before this point is reached.
            setPhase("ready");
        } catch (caught) {
            setError(toLauncherError(caught));
            setPhase("error");
        } finally {
            setProgress(null);
        }
    }, []);

    const restart = useCallback(async () => {
        try {
            await restartApp();
        } catch (caught) {
            setError(toLauncherError(caught));
            setPhase("error");
        }
    }, []);

    useEffect(() => {
        let disposed = false;
        let unlisten: (() => void) | undefined;

        getAppInfo()
            .then((info) => {
                if (!disposed) {
                    setAppVersion(info?.version ?? null);
                }
            })
            .catch(() => undefined);

        getInstallInfo()
            .then((info) => {
                if (disposed) {
                    return;
                }
                setInstallInfo(info);
                if (info?.selfUpdate) {
                    void check({ silent: true });
                }
            })
            .catch((caught: unknown) => {
                console.error(`Could not read the installation type: ${describeError(caught)}`);
            });

        onUpdateEvent((event) => {
            if (event.type === "progress") {
                setProgress({ downloaded: event.downloaded, total: event.total });
            } else {
                setPhase("installing");
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
                console.error(`Could not listen for update events: ${describeError(caught)}`);
            });

        return () => {
            disposed = true;
            unlisten?.();
        };
    }, [check]);

    const value = useMemo<UpdatesContextValue>(
        () => ({
            appVersion,
            installInfo,
            phase,
            update,
            progress,
            error,
            check,
            install,
            restart,
        }),
        [appVersion, installInfo, phase, update, progress, error, check, install, restart],
    );

    return <UpdatesContext.Provider value={value}>{children}</UpdatesContext.Provider>;
}
