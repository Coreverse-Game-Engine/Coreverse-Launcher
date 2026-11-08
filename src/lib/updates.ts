import { createContext } from "react";

import type { InstallInfo, LauncherError, UpdateInfo } from "@/lib/tauri";

/** Where users find the latest builds when the launcher cannot update itself. */
export const RELEASES_URL =
    "https://github.com/Coreverse-Game-Engine/Coreverse-Launcher/releases/latest";

/**
 * `idle`: nothing checked yet (or a silent check failed); `ready`: the update is installed and
 * only a restart is missing.
 */
export type UpdatePhase =
    | "idle"
    | "checking"
    | "upToDate"
    | "available"
    | "downloading"
    | "installing"
    | "ready"
    | "error";

export interface UpdateProgress {
    downloaded: number;
    /** `null` while the server has not told the size. */
    total: number | null;
}

export interface UpdatesContextValue {
    /** Version of the running launcher, `null` in a browser preview. */
    appVersion: string | null;
    /** How the launcher was installed, `null` in a browser preview. */
    installInfo: InstallInfo | null;
    phase: UpdatePhase;
    update: UpdateInfo | null;
    progress: UpdateProgress | null;
    error: LauncherError | null;
    /** `silent` keeps a failed check out of the UI; the automatic check at startup uses it. */
    check: (options?: { silent?: boolean }) => Promise<void>;
    install: () => Promise<void>;
    restart: () => Promise<void>;
}

export const UpdatesContext = createContext<UpdatesContextValue | null>(null);
