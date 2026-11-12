import { createContext } from "react";
import type { Release } from "@Coreverse-Game-Engine/db-client";

import type {
    InstalledEngine,
    InstallOptions,
    LauncherError,
    PlatformInfo,
    InstallStatus,
} from "@/lib/tauri";

/** `unconfigured`: no Coreverse DB connection; `offline`: the server could not be reached. */
export type UpdateCheck = "checking" | "ready" | "offline" | "error" | "unconfigured";

export interface EnginesContextValue {
    installed: InstalledEngine[];
    platform: PlatformInfo | null;
    /** The latest stable release, or `null` if none is published or it could not be read. */
    latest: Release | null;
    updateCheck: UpdateCheck;
    /** The running installation, if any. */
    progress: InstallStatus | null;
    /** The last installation or removal that failed. */
    error: LauncherError | null;
    dismissError: () => void;
    checkForUpdates: () => Promise<void>;
    install: (version: string, options?: InstallOptions) => Promise<void>;
    cancelInstall: () => Promise<void>;
    uninstall: (version: string) => Promise<void>;
}

export const EnginesContext = createContext<EnginesContextValue | null>(null);
