import { useCallback, useEffect, useMemo, useState, type ReactNode } from "react";
import { isTauri } from "@tauri-apps/api/core";
import { getCurrent, onOpenUrl } from "@tauri-apps/plugin-deep-link";
import { openUrl } from "@tauri-apps/plugin-opener";
import {
    isAuthRetryableFetchError,
    type Session,
    type SupabaseClient,
} from "@supabase/supabase-js";

import {
    AuthContext,
    type AuthContextValue,
    type AuthStatus,
    type OAuthProvider,
} from "@/lib/auth";
import { parseAuthCallback } from "@/lib/auth-callback";
import { AUTH_REDIRECT_URL, getSupabase, secureSessionStorage } from "@/lib/supabase";
import { clearRefreshToken, describeError, loadRefreshToken } from "@/lib/tauri";

// React StrictMode runs effects twice; restoring must only happen once because refresh
// tokens rotate.
let restoring: Promise<Session | null> | null = null;
// A deep link can be delivered more than once (for example `getCurrent` after a reload).
const handledCallbacks = new Set<string>();

async function restoreSession(client: SupabaseClient): Promise<Session | null> {
    let token: string | null;
    try {
        token = await loadRefreshToken();
    } catch (error) {
        console.warn(`Could not read the saved session: ${describeError(error)}`);
        secureSessionStorage.markUnavailable();
        return null;
    }
    if (!token) {
        return null;
    }

    const { data, error } = await client.auth.refreshSession({ refresh_token: token });
    if (error) {
        // Offline start: keep the token and try again next time. Anything else means the
        // token is no longer valid.
        if (!isAuthRetryableFetchError(error)) {
            await clearRefreshToken().catch(() => undefined);
        }
        return null;
    }
    return data.session;
}

async function finishOAuth(client: SupabaseClient, raw: string): Promise<string | null> {
    const callback = parseAuthCallback(raw);
    if (!callback || handledCallbacks.has(raw)) {
        return null;
    }
    handledCallbacks.add(raw);
    if (callback.kind === "error") {
        return callback.message;
    }
    const { error } = await client.auth.exchangeCodeForSession(callback.code);
    return error ? error.message : null;
}

export function AuthProvider({ children }: { children: ReactNode }) {
    const [status, setStatus] = useState<AuthStatus>("loading");
    const [session, setSession] = useState<Session | null>(null);
    const [secureStorage, setSecureStorage] = useState(secureSessionStorage.available);
    const [callbackError, setCallbackError] = useState<string | null>(null);

    useEffect(() => secureSessionStorage.subscribe(setSecureStorage), []);

    // Restore the saved session, then follow auth changes.
    useEffect(() => {
        const client = getSupabase();
        if (!client) {
            setStatus("unconfigured");
            return;
        }

        let disposed = false;
        let unsubscribe: (() => void) | undefined;

        restoring ??= restoreSession(client);
        void restoring.then((restored) => {
            if (disposed) {
                return;
            }
            setSession(restored);
            setStatus(restored ? "signed-in" : "signed-out");
            const { data } = client.auth.onAuthStateChange((_event, next) => {
                setSession(next);
                setStatus(next ? "signed-in" : "signed-out");
            });
            unsubscribe = () => data.subscription.unsubscribe();
        });

        return () => {
            disposed = true;
            unsubscribe?.();
        };
    }, []);

    // Finish OAuth sign-ins that come back through the `coreverse-launcher://` deep link.
    useEffect(() => {
        const client = getSupabase();
        if (!client || !isTauri()) {
            return;
        }

        let disposed = false;
        let unlisten: (() => void) | undefined;

        const handle = (urls: string[]) => {
            for (const raw of urls) {
                finishOAuth(client, raw)
                    .then((message) => {
                        if (message) {
                            setCallbackError(message);
                        }
                    })
                    .catch((error: unknown) => setCallbackError(describeError(error)));
            }
        };

        getCurrent()
            .then((urls) => {
                if (!disposed && urls) {
                    handle(urls);
                }
            })
            .catch((error: unknown) => {
                console.warn(`Could not read the launch link: ${describeError(error)}`);
            });
        onOpenUrl(handle)
            .then((dispose) => {
                if (disposed) {
                    dispose();
                } else {
                    unlisten = dispose;
                }
            })
            .catch((error: unknown) => {
                console.warn(`Could not listen for deep links: ${describeError(error)}`);
            });

        return () => {
            disposed = true;
            unlisten?.();
        };
    }, []);

    const signInWithPassword = useCallback(async (email: string, password: string) => {
        const client = getSupabase();
        if (!client) {
            throw new Error("Supabase is not configured.");
        }
        const { error } = await client.auth.signInWithPassword({ email, password });
        if (error) {
            throw error;
        }
    }, []);

    const signInWithOAuth = useCallback(async (provider: OAuthProvider) => {
        const client = getSupabase();
        if (!client) {
            throw new Error("Supabase is not configured.");
        }
        if (!isTauri()) {
            throw new Error("OAuth sign-in needs the desktop app.");
        }
        setCallbackError(null);
        const { data, error } = await client.auth.signInWithOAuth({
            provider,
            options: { redirectTo: AUTH_REDIRECT_URL, skipBrowserRedirect: true },
        });
        if (error) {
            throw error;
        }
        await openUrl(data.url);
    }, []);

    const signOut = useCallback(async () => {
        const client = getSupabase();
        if (!client) {
            return;
        }
        // Local scope: end this device's session only, not the user's other sessions.
        const { error } = await client.auth.signOut({ scope: "local" });
        if (error) {
            throw error;
        }
    }, []);

    const dismissCallbackError = useCallback(() => setCallbackError(null), []);

    const value = useMemo<AuthContextValue>(
        () => ({
            status,
            session,
            secureStorage,
            callbackError,
            dismissCallbackError,
            signInWithPassword,
            signInWithOAuth,
            signOut,
        }),
        [
            status,
            session,
            secureStorage,
            callbackError,
            dismissCallbackError,
            signInWithPassword,
            signInWithOAuth,
            signOut,
        ],
    );

    return <AuthContext.Provider value={value}>{children}</AuthContext.Provider>;
}
