import { AUTH_REDIRECT_URL } from "@/lib/supabase";

export type AuthCallback = { kind: "code"; code: string } | { kind: "error"; message: string };

const EXPECTED = new URL(AUTH_REDIRECT_URL);

/**
 * Reads a deep link opened by the OS. Returns `null` for anything that is not the
 * launcher's own auth callback, so unrelated or malicious links are ignored.
 */
export function parseAuthCallback(raw: string): AuthCallback | null {
    let url: URL;
    try {
        url = new URL(raw);
    } catch {
        return null;
    }
    if (
        url.protocol !== EXPECTED.protocol ||
        url.host !== EXPECTED.host ||
        url.pathname !== EXPECTED.pathname
    ) {
        return null;
    }

    // Supabase puts errors in the query string (PKCE) or, historically, in the fragment.
    const fragment = new URLSearchParams(url.hash.replace(/^#/, ""));
    const read = (name: string) => url.searchParams.get(name) ?? fragment.get(name);

    const message = read("error_description") ?? read("error");
    if (message) {
        return { kind: "error", message };
    }
    const code = read("code");
    return code ? { kind: "code", code } : null;
}
