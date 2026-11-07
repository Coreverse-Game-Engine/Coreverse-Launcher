import { describe, expect, it } from "vitest";

import { parseAuthCallback } from "@/lib/auth-callback";

const BASE = "coreverse-launcher://auth/callback";

describe("parseAuthCallback", () => {
    it("reads the authorization code", () => {
        expect(parseAuthCallback(`${BASE}?code=abc123`)).toEqual({ kind: "code", code: "abc123" });
    });

    it("reads an error from the query string, preferring the description", () => {
        expect(
            parseAuthCallback(`${BASE}?error=access_denied&error_description=User%20said%20no`),
        ).toEqual({ kind: "error", message: "User said no" });
        expect(parseAuthCallback(`${BASE}?error=access_denied`)).toEqual({
            kind: "error",
            message: "access_denied",
        });
    });

    it("reads an error from the fragment", () => {
        expect(parseAuthCallback(`${BASE}#error_description=Bad%20thing`)).toEqual({
            kind: "error",
            message: "Bad thing",
        });
    });

    it("ignores links that are not the launcher's auth callback", () => {
        expect(parseAuthCallback("coreverse-launcher://other/callback?code=x")).toBeNull();
        expect(parseAuthCallback("coreverse-launcher://auth/elsewhere?code=x")).toBeNull();
        expect(parseAuthCallback("https://auth/callback?code=x")).toBeNull();
        expect(parseAuthCallback("not a url")).toBeNull();
    });

    it("ignores the callback without a code or an error", () => {
        expect(parseAuthCallback(BASE)).toBeNull();
    });
});
