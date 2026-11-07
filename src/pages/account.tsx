import { useState, type FormEvent } from "react";
import { isAuthError, isAuthRetryableFetchError } from "@supabase/supabase-js";
import type { TFunction } from "i18next";
import { useTranslation } from "react-i18next";

import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Separator } from "@/components/ui/separator";
import { useAuth } from "@/hooks/use-auth";
import type { OAuthProvider } from "@/lib/auth";

function authErrorMessage(t: TFunction, error: unknown): string {
    if (isAuthRetryableFetchError(error)) {
        return t("account.errors.network");
    }
    if (isAuthError(error)) {
        if (error.code === "invalid_credentials") {
            return t("account.errors.invalidCredentials");
        }
        if (error.code === "email_not_confirmed") {
            return t("account.errors.emailNotConfirmed");
        }
    }
    return t("account.errors.generic");
}

function ErrorNotice({ children }: { children: string }) {
    return (
        <p
            role="alert"
            className="rounded-md border border-destructive/50 bg-destructive/10 px-3 py-2 text-sm text-destructive"
        >
            {children}
        </p>
    );
}

function SignInCard() {
    const { t } = useTranslation();
    const { signInWithPassword, signInWithOAuth, callbackError, dismissCallbackError } = useAuth();
    const [email, setEmail] = useState("");
    const [password, setPassword] = useState("");
    const [busy, setBusy] = useState(false);
    const [waitingForBrowser, setWaitingForBrowser] = useState(false);
    const [error, setError] = useState<string | null>(null);

    const run = async (action: () => Promise<void>) => {
        setBusy(true);
        setError(null);
        dismissCallbackError();
        try {
            await action();
        } catch (caught) {
            console.error(caught);
            setError(authErrorMessage(t, caught));
        } finally {
            setBusy(false);
        }
    };

    const onSubmit = (event: FormEvent<HTMLFormElement>) => {
        event.preventDefault();
        void run(() => signInWithPassword(email.trim(), password));
    };

    const onProvider = (provider: OAuthProvider) => {
        setWaitingForBrowser(false);
        void run(async () => {
            await signInWithOAuth(provider);
            setWaitingForBrowser(true);
        });
    };

    return (
        <Card>
            <CardHeader>
                <CardTitle>{t("account.signInTitle")}</CardTitle>
                <CardDescription>{t("account.signInDescription")}</CardDescription>
            </CardHeader>
            <CardContent className="flex flex-col gap-4">
                {error ? <ErrorNotice>{error}</ErrorNotice> : null}
                {callbackError ? (
                    <ErrorNotice>
                        {t("account.errors.callback", { message: callbackError })}
                    </ErrorNotice>
                ) : null}
                <form className="flex flex-col gap-4" onSubmit={onSubmit}>
                    <div className="flex flex-col gap-2">
                        <Label htmlFor="account-email">{t("account.email")}</Label>
                        <Input
                            id="account-email"
                            type="email"
                            autoComplete="email"
                            required
                            value={email}
                            onChange={(event) => setEmail(event.target.value)}
                        />
                    </div>
                    <div className="flex flex-col gap-2">
                        <Label htmlFor="account-password">{t("account.password")}</Label>
                        <Input
                            id="account-password"
                            type="password"
                            autoComplete="current-password"
                            required
                            value={password}
                            onChange={(event) => setPassword(event.target.value)}
                        />
                    </div>
                    <Button type="submit" disabled={busy}>
                        {t("account.signIn")}
                    </Button>
                </form>
                <div className="flex items-center gap-3">
                    <Separator className="flex-1" />
                    <span className="text-xs text-muted-foreground">
                        {t("account.orContinueWith")}
                    </span>
                    <Separator className="flex-1" />
                </div>
                <div className="flex flex-wrap gap-2">
                    <Button variant="outline" disabled={busy} onClick={() => onProvider("github")}>
                        GitHub
                    </Button>
                    <Button variant="outline" disabled={busy} onClick={() => onProvider("google")}>
                        Google
                    </Button>
                </div>
                {waitingForBrowser ? (
                    <p className="text-sm text-muted-foreground">{t("account.waitingForBrowser")}</p>
                ) : null}
                <p className="text-sm text-muted-foreground">{t("account.createAccountHint")}</p>
            </CardContent>
        </Card>
    );
}

function SignedInCard() {
    const { t } = useTranslation();
    const { session, signOut, secureStorage } = useAuth();
    const [busy, setBusy] = useState(false);
    const [error, setError] = useState<string | null>(null);

    const onSignOut = async () => {
        setBusy(true);
        setError(null);
        try {
            await signOut();
        } catch (caught) {
            console.error(caught);
            setError(authErrorMessage(t, caught));
            setBusy(false);
        }
    };

    return (
        <Card>
            <CardHeader>
                <CardTitle>{t("account.signedInTitle")}</CardTitle>
                <CardDescription>
                    {t("account.signedInAs", { email: session?.user.email ?? "" })}
                </CardDescription>
            </CardHeader>
            <CardContent className="flex flex-col gap-4">
                {error ? <ErrorNotice>{error}</ErrorNotice> : null}
                {secureStorage ? null : (
                    <p className="text-sm text-muted-foreground">{t("account.keyringWarning")}</p>
                )}
                <div>
                    <Button variant="outline" disabled={busy} onClick={() => void onSignOut()}>
                        {t("account.signOut")}
                    </Button>
                </div>
            </CardContent>
        </Card>
    );
}

export function AccountPage() {
    const { t } = useTranslation();
    const { status } = useAuth();

    return (
        <div className="mx-auto flex max-w-3xl flex-col gap-6">
            <h1 className="text-3xl font-semibold tracking-tight">{t("account.title")}</h1>
            {status === "loading" ? (
                <p className="text-muted-foreground">{t("account.loading")}</p>
            ) : null}
            {status === "unconfigured" ? (
                <Card>
                    <CardHeader>
                        <CardTitle>{t("account.notConfiguredTitle")}</CardTitle>
                        <CardDescription>{t("account.notConfiguredDescription")}</CardDescription>
                    </CardHeader>
                </Card>
            ) : null}
            {status === "signed-out" ? <SignInCard /> : null}
            {status === "signed-in" ? <SignedInCard /> : null}
        </div>
    );
}
