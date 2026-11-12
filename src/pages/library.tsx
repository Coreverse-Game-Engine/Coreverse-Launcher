import { useState } from "react";
import { Library, RefreshCw } from "lucide-react";
import { useTranslation } from "react-i18next";

import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Checkbox } from "@/components/ui/checkbox";
import { Label } from "@/components/ui/label";
import { Progress } from "@/components/ui/progress";
import { useEngines } from "@/hooks/use-engines";
import { formatBytes, percent } from "@/lib/format";
import { previousVersion } from "@/lib/versions";

function dateLabel(value: string, language: string): string {
    const date = new Date(value);
    return Number.isNaN(date.getTime()) ? value : date.toLocaleDateString(language);
}

function InstallProgress() {
    const { t } = useTranslation();
    const { progress, cancelInstall } = useEngines();
    const [cancelling, setCancelling] = useState(false);

    if (!progress) {
        return null;
    }
    const value = percent(progress.done, progress.total);
    const showBytes =
        progress.phase === "downloading" ||
        progress.phase === "copying" ||
        progress.phase === "verifying";

    return (
        <div className="flex flex-col gap-2">
            <Progress value={value} />
            <div className="flex items-center justify-between gap-4 text-sm">
                <span className="text-muted-foreground">
                    {t(`library.phase.${progress.phase}`)}
                    {showBytes && progress.total > 0
                        ? ` · ${formatBytes(progress.done)} / ${formatBytes(progress.total)}`
                        : value === null
                          ? ""
                          : ` · ${value}%`}
                </span>
                <Button
                    variant="outline"
                    size="sm"
                    disabled={cancelling}
                    onClick={() => {
                        setCancelling(true);
                        void cancelInstall().finally(() => setCancelling(false));
                    }}
                >
                    {t("library.cancel")}
                </Button>
            </div>
        </div>
    );
}

function ErrorNotice() {
    const { t } = useTranslation();
    const { error, dismissError } = useEngines();

    if (!error) {
        return null;
    }
    return (
        <div
            role="alert"
            className="flex items-start justify-between gap-4 rounded-md border border-destructive/50 bg-destructive/10 px-3 py-2 text-sm text-destructive"
        >
            <span>{t(`library.errors.${error.code}`, { defaultValue: error.message })}</span>
            <button type="button" className="shrink-0 underline" onClick={dismissError}>
                {t("library.dismiss")}
            </button>
        </div>
    );
}

function LatestReleaseCard() {
    const { t, i18n } = useTranslation();
    const { latest, installed, platform, progress, updateCheck, install } = useEngines();
    // Off by default, and not remembered: removing a version should be a choice made each time.
    const [deletePrevious, setDeletePrevious] = useState(false);

    const notice =
        updateCheck === "checking"
            ? t("library.checking")
            : updateCheck === "offline"
              ? t("library.offline")
              : updateCheck === "error"
                ? t("library.checkFailed")
                : updateCheck === "unconfigured"
                  ? t("library.unconfigured")
                  : latest === null
                    ? t("library.noRelease")
                    : null;

    if (!latest) {
        return notice ? <p className="text-sm text-muted-foreground">{notice}</p> : null;
    }

    const artifact = platform
        ? latest.artifacts.find(
              (item) => item.os === platform.os && item.architecture === platform.architecture,
          )
        : undefined;
    const isInstalled = installed.some((engine) => engine.version === latest.version);
    const busy = progress !== null;
    const previous = previousVersion(installed, latest.version, platform);

    return (
        <Card>
            <CardHeader>
                <CardTitle>{t("library.latestTitle", { version: latest.version })}</CardTitle>
                <CardDescription>
                    {t("library.releasedOn", {
                        date: dateLabel(latest.release_date, i18n.language),
                    })}
                    {artifact ? ` · ${formatBytes(artifact.size_bytes)}` : ""}
                </CardDescription>
            </CardHeader>
            <CardContent className="flex flex-col gap-4">
                {latest.release_notes_summary ? (
                    <p className="text-sm">{latest.release_notes_summary}</p>
                ) : null}
                {notice ? <p className="text-sm text-muted-foreground">{notice}</p> : null}
                <InstallProgress />
                {!busy && !isInstalled && artifact && previous ? (
                    <div className="flex items-center gap-2">
                        <Checkbox
                            id="delete-previous"
                            checked={deletePrevious}
                            onChange={(event) => setDeletePrevious(event.target.checked)}
                        />
                        <Label htmlFor="delete-previous" className="font-normal">
                            {t("library.deletePrevious", { version: previous })}
                        </Label>
                    </div>
                ) : null}
                {!busy ? (
                    <div className="flex items-center gap-3">
                        {isInstalled ? (
                            <span className="text-sm text-muted-foreground">
                                {t("library.upToDate")}
                            </span>
                        ) : (
                            <Button
                                disabled={!artifact}
                                onClick={() =>
                                    void install(latest.version, {
                                        deletePrevious: previous !== null && deletePrevious,
                                    })
                                }
                            >
                                {t("library.install", { version: latest.version })}
                            </Button>
                        )}
                        {!isInstalled && platform && !artifact ? (
                            <span className="text-sm text-muted-foreground">
                                {t("library.noBuildForPlatform", {
                                    os: platform.os,
                                    architecture: platform.architecture,
                                })}
                            </span>
                        ) : null}
                    </div>
                ) : null}
            </CardContent>
        </Card>
    );
}

function InstalledList() {
    const { t, i18n } = useTranslation();
    const { installed, progress, uninstall } = useEngines();
    const [confirming, setConfirming] = useState<string | null>(null);

    if (installed.length === 0) {
        return (
            <div className="flex flex-col items-center gap-3 rounded-xl border border-dashed py-12 text-center">
                <Library className="size-10 text-muted-foreground" aria-hidden="true" />
                <h2 className="text-lg font-medium">{t("library.emptyTitle")}</h2>
                <p className="max-w-sm text-sm text-muted-foreground">
                    {t("library.emptyDescription")}
                </p>
            </div>
        );
    }

    return (
        <section className="flex flex-col gap-3">
            <h2 className="text-lg font-medium">{t("library.installedTitle")}</h2>
            <ul className="flex flex-col gap-2">
                {installed.map((engine) => (
                    <li
                        key={engine.version}
                        className="flex items-center justify-between gap-4 rounded-lg border bg-card px-4 py-3"
                    >
                        <div className="flex flex-col">
                            <span className="font-medium">
                                {t("home.version", { version: engine.version })}
                                <span className="ml-2 rounded-full bg-secondary px-2 py-0.5 text-xs text-secondary-foreground">
                                    {engine.status}
                                </span>
                            </span>
                            <span className="text-sm text-muted-foreground">
                                {t("library.installedOn", {
                                    date: dateLabel(engine.installedAt, i18n.language),
                                })}
                                {` · ${formatBytes(engine.sizeBytes)}`}
                            </span>
                        </div>
                        {confirming === engine.version ? (
                            <div className="flex items-center gap-2">
                                <Button
                                    variant="destructive"
                                    size="sm"
                                    onClick={() => {
                                        setConfirming(null);
                                        void uninstall(engine.version);
                                    }}
                                >
                                    {t("library.confirmUninstall")}
                                </Button>
                                <Button
                                    variant="ghost"
                                    size="sm"
                                    onClick={() => setConfirming(null)}
                                >
                                    {t("library.keep")}
                                </Button>
                            </div>
                        ) : (
                            <Button
                                variant="outline"
                                size="sm"
                                disabled={progress?.version === engine.version}
                                onClick={() => setConfirming(engine.version)}
                            >
                                {t("library.uninstall")}
                            </Button>
                        )}
                    </li>
                ))}
            </ul>
        </section>
    );
}

export function LibraryPage() {
    const { t } = useTranslation();
    const { updateCheck, progress, checkForUpdates } = useEngines();

    return (
        <div className="mx-auto flex max-w-3xl flex-col gap-6">
            <div className="flex items-center justify-between gap-4">
                <h1 className="text-3xl font-semibold tracking-tight">{t("library.title")}</h1>
                <Button
                    variant="outline"
                    size="sm"
                    disabled={updateCheck === "checking" || progress !== null}
                    onClick={() => void checkForUpdates()}
                >
                    <RefreshCw aria-hidden="true" />
                    {t("library.checkForUpdates")}
                </Button>
            </div>
            <ErrorNotice />
            <LatestReleaseCard />
            <InstalledList />
        </div>
    );
}
