import { openUrl } from "@tauri-apps/plugin-opener";
import { useTranslation } from "react-i18next";

import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Progress } from "@/components/ui/progress";
import { useUpdates } from "@/hooks/use-updates";
import { formatBytes, percent } from "@/lib/format";
import { describeError } from "@/lib/tauri";
import { RELEASES_URL } from "@/lib/updates";

function openReleases() {
    openUrl(RELEASES_URL).catch((error: unknown) => {
        console.error(`Could not open the releases page: ${describeError(error)}`);
    });
}

/** Settings card: the installed version and, where the launcher may do it, updating. */
export function UpdateCard() {
    const { t } = useTranslation();
    const { appVersion, installInfo, phase, update, progress, error, check, install, restart } =
        useUpdates();

    const body = () => {
        if (!installInfo) {
            return <p className="text-sm text-muted-foreground">{t("updates.browserOnly")}</p>;
        }
        if (!installInfo.selfUpdate) {
            return (
                <div className="flex flex-col items-start gap-3">
                    <p className="text-sm">{t(`updates.manual.${installInfo.channel}`)}</p>
                    {installInfo.channel !== "scoop" ? (
                        <Button variant="outline" onClick={openReleases}>
                            {t("updates.releasesPage")}
                        </Button>
                    ) : null}
                </div>
            );
        }

        const value = progress ? percent(progress.downloaded, progress.total ?? 0) : null;
        return (
            <div className="flex flex-col items-start gap-3">
                {phase === "checking" ? (
                    <p className="text-sm text-muted-foreground">{t("updates.checking")}</p>
                ) : null}
                {phase === "upToDate" ? (
                    <p className="text-sm text-muted-foreground">{t("updates.upToDate")}</p>
                ) : null}
                {phase === "available" && update ? (
                    <>
                        <p className="text-sm font-medium">
                            {t("updates.available", { version: update.version })}
                        </p>
                        {update.notes ? (
                            <div className="w-full">
                                <h3 className="text-sm font-medium">{t("updates.notesTitle")}</h3>
                                <p className="max-h-40 overflow-y-auto text-sm whitespace-pre-wrap text-muted-foreground">
                                    {update.notes}
                                </p>
                            </div>
                        ) : null}
                        <Button onClick={() => void install()}>{t("updates.install")}</Button>
                    </>
                ) : null}
                {phase === "downloading" ? (
                    <div className="flex w-full flex-col gap-2">
                        <Progress value={value} />
                        <p className="text-sm text-muted-foreground">
                            {t("updates.downloading")}
                            {progress && progress.total
                                ? ` · ${formatBytes(progress.downloaded)} / ${formatBytes(progress.total)}`
                                : ""}
                        </p>
                    </div>
                ) : null}
                {phase === "installing" ? (
                    <p className="text-sm text-muted-foreground">{t("updates.installing")}</p>
                ) : null}
                {phase === "ready" ? (
                    <>
                        <p className="text-sm">{t("updates.ready")}</p>
                        <Button onClick={() => void restart()}>{t("updates.restart")}</Button>
                    </>
                ) : null}
                {error ? (
                    <p role="alert" className="text-sm text-destructive">
                        {t(`updates.errors.${error.code}`, { defaultValue: error.message })}
                    </p>
                ) : null}
                {phase === "idle" ||
                phase === "upToDate" ||
                phase === "error" ||
                phase === "checking" ? (
                    <Button
                        variant="outline"
                        disabled={phase === "checking"}
                        onClick={() => void check()}
                    >
                        {t("updates.check")}
                    </Button>
                ) : null}
            </div>
        );
    };

    return (
        <Card>
            <CardHeader>
                <CardTitle>{t("updates.title")}</CardTitle>
                {appVersion ? (
                    <CardDescription>
                        {t("updates.currentVersion", { version: appVersion })}
                    </CardDescription>
                ) : null}
            </CardHeader>
            <CardContent>{body()}</CardContent>
        </Card>
    );
}
