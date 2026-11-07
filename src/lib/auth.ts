import { createContext } from "react";
import type { Session } from "@supabase/supabase-js";

export type AuthStatus = "loading" | "unconfigured" | "signed-out" | "signed-in";
export type OAuthProvider = "github" | "google";

export interface AuthContextValue {
    status: AuthStatus;
    session: Session | null;
    /** `false` when the refresh token cannot be kept in the system keyring. */
    secureStorage: boolean;
    /** Set when finishing an OAuth sign-in in the browser failed. */
    callbackError: string | null;
    dismissCallbackError: () => void;
    signInWithPassword: (email: string, password: string) => Promise<void>;
    /** Opens the provider's login page in the system browser. */
    signInWithOAuth: (provider: OAuthProvider) => Promise<void>;
    signOut: () => Promise<void>;
}

export const AuthContext = createContext<AuthContextValue | null>(null);
