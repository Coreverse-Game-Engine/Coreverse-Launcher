import { isTauri } from "@tauri-apps/api/core";
import { fetch as tauriFetch } from "@tauri-apps/plugin-http";
import { configureCoreverseClient } from "@Coreverse-Game-Engine/db-client";

import { getSupabase } from "@/lib/supabase";

/**
 * Coreverse DB requests go through Rust's HTTP client instead of the webview's `fetch`.
 * That is how the DB expects the Launcher to call it: no `Origin` header and therefore no
 * CORS (the DB only allowlists Website origins). The allowed URLs are listed in
 * `src-tauri/capabilities/default.json`.
 */
const rustFetch: typeof fetch = (input, init) => tauriFetch(input, init);

let configured = false;

/**
 * Points the db-client at the Coreverse DB API. Call once at startup. Returns `false` (and
 * leaves the client unconfigured) when `PUBLIC_SUPABASE_*` are missing.
 */
export function configureDb(): boolean {
    const url = import.meta.env.PUBLIC_SUPABASE_URL;
    const supabase = getSupabase();
    if (!url || !supabase) {
        configured = false;
        return false;
    }

    configureCoreverseClient({
        baseUrl: `${url.replace(/\/+$/, "")}/functions/v1`,
        // Signed in: the session's access token (supabase-js refreshes it when it is about to
        // expire). Signed out: no token at all. The DB answers public reads without one and
        // rejects a token that is not a user session, so the anon key must not be sent here.
        getAuthToken: async () => {
            const { data } = await supabase.auth.getSession();
            return data.session?.access_token ?? null;
        },
        fetch: isTauri() ? rustFetch : undefined,
    });
    configured = true;
    return true;
}

export function isDbConfigured(): boolean {
    return configured;
}
