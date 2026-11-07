import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import {
    CoreverseApiError,
    getLatestRelease,
    type Release,
} from "@Coreverse-Game-Engine/db-client";

import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { isDbConfigured } from "@/lib/db";
import { describeError } from "@/lib/tauri";

type State =
    | { kind: "loading" }
    | { kind: "unconfigured" }
    | { kind: "none" }
    | { kind: "error"; message: string }
    | { kind: "ready"; release: Release };

/** Latest stable Coreverse Engine release, read from Coreverse DB (works signed out). */
export function LatestReleaseCard() {
    const { t, i18n } = useTranslation();
    const [state, setState] = useState<State>({ kind: "loading" });

    useEffect(() => {
        let active = true;
        const update = (next: State) => {
            if (active) {
                setState(next);
            }
        };

        if (!isDbConfigured()) {
            update({ kind: "unconfigured" });
            return;
        }

        getLatestRelease()
            .then((response) => {
                update(
                    response.status === 200
                        ? { kind: "ready", release: response.data }
                        : { kind: "none" },
                );
            })
            .catch((error: unknown) => {
                if (error instanceof CoreverseApiError && error.status === 404) {
                    update({ kind: "none" });
                } else {
                    update({ kind: "error", message: describeError(error) });
                }
            });

        return () => {
            active = false;
        };
    }, []);

    let description = t("home.engine.loading");
    if (state.kind === "unconfigured") {
        description = t("home.engine.unconfigured");
    } else if (state.kind === "none") {
        description = t("home.engine.none");
    } else if (state.kind === "error") {
        description = t("home.engine.error");
    } else if (state.kind === "ready") {
        description = t("home.engine.latest", { status: state.release.status });
    }

    return (
        <Card>
            <CardHeader>
                <CardTitle>{t("home.engine.title")}</CardTitle>
                <CardDescription>{description}</CardDescription>
            </CardHeader>
            {state.kind === "ready" ? (
                <CardContent className="flex flex-col gap-1 text-sm">
                    <p className="text-lg font-medium">
                        {t("home.version", { version: state.release.version })}
                    </p>
                    <p className="text-muted-foreground">
                        {t("home.engine.releasedOn", {
                            date: new Date(state.release.release_date).toLocaleDateString(
                                i18n.language,
                            ),
                        })}
                    </p>
                    {state.release.release_notes_summary ? (
                        <p>{state.release.release_notes_summary}</p>
                    ) : null}
                </CardContent>
            ) : null}
            {state.kind === "error" ? (
                <CardContent>
                    <p className="text-sm text-muted-foreground">{state.message}</p>
                </CardContent>
            ) : null}
        </Card>
    );
}
