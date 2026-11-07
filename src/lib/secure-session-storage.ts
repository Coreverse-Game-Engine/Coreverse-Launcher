import { isTauri } from "@tauri-apps/api/core";
import type { SupportedStorage } from "@supabase/supabase-js";

import { clearRefreshToken, describeError, saveRefreshToken } from "@/lib/tauri";

/** supabase-js stores the session under `sb-<project>-auth-token`. */
const SESSION_KEY_SUFFIX = "-auth-token";

function extractRefreshToken(sessionJson: string): string | null {
    try {
        const parsed: unknown = JSON.parse(sessionJson);
        if (
            typeof parsed === "object" &&
            parsed !== null &&
            "refresh_token" in parsed &&
            typeof parsed.refresh_token === "string"
        ) {
            return parsed.refresh_token;
        }
    } catch {
        // Not JSON: nothing to persist.
    }
    return null;
}

/**
 * Session storage for supabase-js.
 *
 * The whole session (including the access token) only lives in memory. The refresh token,
 * the one value that can mint new sessions, is mirrored into the system keyring so the user
 * stays signed in across restarts. If the keyring is unavailable the session still works
 * but is lost on exit; `available` reports that so the UI can say so.
 */
export class SecureSessionStorage implements SupportedStorage {
    private readonly memory = new Map<string, string>();
    private readonly listeners = new Set<(available: boolean) => void>();
    private isAvailable = isTauri();

    get available(): boolean {
        return this.isAvailable;
    }

    subscribe(listener: (available: boolean) => void): () => void {
        this.listeners.add(listener);
        return () => {
            this.listeners.delete(listener);
        };
    }

    markUnavailable(): void {
        if (!this.isAvailable) {
            return;
        }
        this.isAvailable = false;
        this.listeners.forEach((listener) => listener(false));
    }

    getItem(key: string): string | null {
        return this.memory.get(key) ?? null;
    }

    setItem(key: string, value: string): void {
        this.memory.set(key, value);
        if (!key.endsWith(SESSION_KEY_SUFFIX)) {
            return;
        }
        const token = extractRefreshToken(value);
        if (token) {
            saveRefreshToken(token).catch((error: unknown) => {
                console.warn(`Could not save the session: ${describeError(error)}`);
                this.markUnavailable();
            });
        }
    }

    removeItem(key: string): void {
        this.memory.delete(key);
        if (key.endsWith(SESSION_KEY_SUFFIX)) {
            clearRefreshToken().catch((error: unknown) => {
                console.warn(`Could not clear the saved session: ${describeError(error)}`);
            });
        }
    }
}
