import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export interface AppInfo {
    name: string;
    version: string;
}

export type ThemeSetting = "light" | "dark" | "system";
export type LanguageSetting = "en" | "tr";

/** Mirrors `launcher_core::Settings`. `language` is `null` until the user picks one. */
export interface Settings {
    theme: ThemeSetting;
    language: LanguageSetting | null;
}

/** Mirrors `launcher_core::SettingsPatch`: only the given fields are changed. */
export interface SettingsPatch {
    theme?: ThemeSetting;
    language?: LanguageSetting;
}

/** Mirrors the serialized `launcher_core::Error`. */
export interface LauncherError {
    code: string;
    message: string;
}

/** Mirrors `events::SETTINGS_CHANGED` in `src-tauri/src/events.rs`. */
export const SETTINGS_CHANGED_EVENT = "settings-changed";

export function isLauncherError(value: unknown): value is LauncherError {
    return (
        typeof value === "object" &&
        value !== null &&
        "code" in value &&
        "message" in value &&
        typeof value.code === "string" &&
        typeof value.message === "string"
    );
}

/** Turns anything thrown by a command into a readable message. */
export function describeError(error: unknown): string {
    if (isLauncherError(error)) {
        return `${error.message} (${error.code})`;
    }
    return error instanceof Error ? error.message : String(error);
}

/**
 * Asks the Rust side for application info.
 * Returns `null` when the UI runs in a plain browser (e.g. `pnpm dev` without Tauri).
 */
export async function getAppInfo(): Promise<AppInfo | null> {
    if (!isTauri()) {
        return null;
    }
    return invoke<AppInfo>("app_info");
}

/** Reads the settings stored in SQLite. `null` in a plain browser. */
export async function getSettings(): Promise<Settings | null> {
    if (!isTauri()) {
        return null;
    }
    return invoke<Settings>("get_settings");
}

/** Saves a partial settings update. `null` in a plain browser. */
export async function updateSettings(patch: SettingsPatch): Promise<Settings | null> {
    if (!isTauri()) {
        return null;
    }
    return invoke<Settings>("update_settings", { patch });
}

/** Subscribes to settings changes. Resolves to the function that unsubscribes again. */
export async function onSettingsChanged(
    handler: (settings: Settings) => void,
): Promise<() => void> {
    if (!isTauri()) {
        return () => undefined;
    }
    return listen<Settings>(SETTINGS_CHANGED_EVENT, (event) => handler(event.payload));
}

/** Reads the refresh token from the system keyring. `null` when none is saved or in a browser. */
export async function loadRefreshToken(): Promise<string | null> {
    if (!isTauri()) {
        return null;
    }
    return invoke<string | null>("load_refresh_token");
}

/** Saves the refresh token to the system keyring. Does nothing in a plain browser. */
export async function saveRefreshToken(token: string): Promise<void> {
    if (!isTauri()) {
        return;
    }
    await invoke("save_refresh_token", { token });
}

/** Removes the refresh token from the system keyring. Does nothing in a plain browser. */
export async function clearRefreshToken(): Promise<void> {
    if (!isTauri()) {
        return;
    }
    await invoke("clear_refresh_token");
}

/** Mirrors `launcher_core::Platform`: the names Coreverse DB uses for this machine. */
export interface PlatformInfo {
    os: string;
    architecture: string;
}

/** Mirrors `launcher_core::InstalledEngine`. */
export interface InstalledEngine {
    version: string;
    status: string;
    os: string;
    architecture: string;
    sha256: string;
    sizeBytes: number;
    installedAt: string;
}

export type InstallPhase = "preparing" | "downloading" | "verifying" | "extracting" | "finalizing";

/** Mirrors `launcher_core::InstallStatus`. */
export interface InstallStatus {
    version: string;
    phase: InstallPhase;
    done: number;
    total: number;
}

/** Mirrors `launcher_core::InstallEvent`. */
export type InstallEvent =
    | ({ type: "progress" } & InstallStatus)
    | { type: "finished"; version: string }
    | { type: "cancelled"; version: string }
    | { type: "failed"; version: string; error: LauncherError };

/** Mirrors `events::ENGINE_INSTALL` in `src-tauri/src/events.rs`. */
export const ENGINE_INSTALL_EVENT = "engine-install-event";

/** Tells the Rust side where Coreverse DB's API lives. Needed once before installing. */
export async function configureApi(baseUrl: string): Promise<void> {
    if (!isTauri()) {
        return;
    }
    await invoke("configure_api", { baseUrl });
}

export async function getPlatform(): Promise<PlatformInfo | null> {
    if (!isTauri()) {
        return null;
    }
    return invoke<PlatformInfo | null>("current_platform");
}

/** Installed engine versions, newest first. Empty in a plain browser. */
export async function listInstalledEngines(): Promise<InstalledEngine[]> {
    if (!isTauri()) {
        return [];
    }
    return invoke<InstalledEngine[]>("list_installed_engines");
}

export async function getInstallStatus(): Promise<InstallStatus | null> {
    if (!isTauri()) {
        return null;
    }
    return invoke<InstallStatus | null>("get_install_status");
}

/** Runs until the installation is over; progress arrives through `onInstallEvent`. */
export async function installEngine(version: string): Promise<InstalledEngine> {
    if (!isTauri()) {
        throw new Error("Installing engines needs the desktop app.");
    }
    return invoke<InstalledEngine>("install_engine", { version });
}

export async function cancelInstall(): Promise<boolean> {
    if (!isTauri()) {
        return false;
    }
    return invoke<boolean>("cancel_install");
}

export async function uninstallEngine(version: string): Promise<void> {
    if (!isTauri()) {
        return;
    }
    await invoke("uninstall_engine", { version });
}

/** Subscribes to installation events. Resolves to the function that unsubscribes again. */
export async function onInstallEvent(handler: (event: InstallEvent) => void): Promise<() => void> {
    if (!isTauri()) {
        return () => undefined;
    }
    return listen<InstallEvent>(ENGINE_INSTALL_EVENT, (event) => handler(event.payload));
}
