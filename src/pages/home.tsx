import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { LatestReleaseCard } from "@/components/latest-release-card";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { getAppInfo, type AppInfo } from "@/lib/tauri";

export function HomePage() {
    const { t } = useTranslation();
    const [info, setInfo] = useState<AppInfo | null>(null);

    useEffect(() => {
        let active = true;
        getAppInfo()
            .then((value) => {
                if (active) {
                    setInfo(value);
                }
            })
            .catch(() => {
                if (active) {
                    setInfo(null);
                }
            });
        return () => {
            active = false;
        };
    }, []);

    return (
        <div className="mx-auto flex max-w-3xl flex-col gap-6">
            <header className="flex flex-col gap-1">
                <h1 className="text-3xl font-semibold tracking-tight">{t("home.title")}</h1>
                <p className="text-muted-foreground">{t("home.subtitle")}</p>
            </header>
            <Card>
                <CardHeader>
                    <CardTitle>{t("home.statusTitle")}</CardTitle>
                    <CardDescription>
                        {info ? t("home.statusDesktop") : t("home.statusBrowser")}
                    </CardDescription>
                </CardHeader>
                {info ? (
                    <CardContent>
                        <p className="text-sm">
                            {info.name} · {t("home.version", { version: info.version })}
                        </p>
                    </CardContent>
                ) : null}
            </Card>
            <LatestReleaseCard />
        </div>
    );
}
