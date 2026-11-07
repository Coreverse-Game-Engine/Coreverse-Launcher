import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
    saveRefreshToken: vi.fn(),
    clearRefreshToken: vi.fn(),
}));

vi.mock("@tauri-apps/api/core", () => ({ isTauri: () => true }));
vi.mock("@/lib/tauri", () => ({
    saveRefreshToken: mocks.saveRefreshToken,
    clearRefreshToken: mocks.clearRefreshToken,
    describeError: (error: unknown) => String(error),
}));

import { SecureSessionStorage } from "@/lib/secure-session-storage";

const SESSION_KEY = "sb-project-auth-token";

describe("SecureSessionStorage", () => {
    beforeEach(() => {
        mocks.saveRefreshToken.mockReset().mockResolvedValue(undefined);
        mocks.clearRefreshToken.mockReset().mockResolvedValue(undefined);
    });

    it("keeps the session in memory and mirrors only the refresh token to the keyring", () => {
        const storage = new SecureSessionStorage();
        const session = JSON.stringify({ access_token: "access", refresh_token: "refresh-1" });

        storage.setItem(SESSION_KEY, session);

        expect(storage.getItem(SESSION_KEY)).toBe(session);
        expect(mocks.saveRefreshToken).toHaveBeenCalledExactlyOnceWith("refresh-1");
    });

    it("does not persist other keys such as the PKCE code verifier", () => {
        const storage = new SecureSessionStorage();

        storage.setItem(`${SESSION_KEY}-code-verifier`, "verifier");

        expect(storage.getItem(`${SESSION_KEY}-code-verifier`)).toBe("verifier");
        expect(mocks.saveRefreshToken).not.toHaveBeenCalled();
    });

    it("ignores session values that are not JSON", () => {
        const storage = new SecureSessionStorage();

        storage.setItem(SESSION_KEY, "not json");

        expect(mocks.saveRefreshToken).not.toHaveBeenCalled();
    });

    it("clears the keyring when the session is removed", () => {
        const storage = new SecureSessionStorage();
        storage.setItem(SESSION_KEY, JSON.stringify({ refresh_token: "r" }));

        storage.removeItem(SESSION_KEY);

        expect(storage.getItem(SESSION_KEY)).toBeNull();
        expect(mocks.clearRefreshToken).toHaveBeenCalledOnce();
    });

    it("reports when the keyring cannot be used", async () => {
        mocks.saveRefreshToken.mockRejectedValueOnce(new Error("no keyring"));
        const storage = new SecureSessionStorage();
        const listener = vi.fn();
        storage.subscribe(listener);
        vi.spyOn(console, "warn").mockImplementation(() => undefined);

        storage.setItem(SESSION_KEY, JSON.stringify({ refresh_token: "r" }));

        await vi.waitFor(() => expect(listener).toHaveBeenCalledWith(false));
        expect(storage.available).toBe(false);
    });
});
