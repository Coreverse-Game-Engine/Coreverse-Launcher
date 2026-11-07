import { createClient, type SupabaseClient } from "@supabase/supabase-js";

import { SecureSessionStorage } from "@/lib/secure-session-storage";

/** Where Supabase sends the browser back to after OAuth. Must be on the project's redirect allow-list. */
export const AUTH_REDIRECT_URL = "coreverse-launcher://auth/callback";

export const secureSessionStorage = new SecureSessionStorage();

let client: SupabaseClient | null | undefined;

/** The shared Supabase client, or `null` when `PUBLIC_SUPABASE_*` are not configured. */
export function getSupabase(): SupabaseClient | null {
    if (client !== undefined) {
        return client;
    }
    const url = import.meta.env.PUBLIC_SUPABASE_URL;
    const anonKey = import.meta.env.PUBLIC_SUPABASE_ANON_KEY;
    client =
        url && anonKey
            ? createClient(url, anonKey, {
                  auth: {
                      flowType: "pkce",
                      persistSession: true,
                      autoRefreshToken: true,
                      // The OAuth redirect arrives as a deep link, not as a page URL.
                      detectSessionInUrl: false,
                      storage: secureSessionStorage,
                  },
              })
            : null;
    return client;
}
