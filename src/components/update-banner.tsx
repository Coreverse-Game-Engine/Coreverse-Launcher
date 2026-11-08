import { useTranslation } from "react-i18next";
import { Link } from "react-router";

import { Button } from "@/components/ui/button";
import { useUpdates } from "@/hooks/use-updates";

/** A slim notice above the page when an update is waiting or has been installed. */
export function UpdateBanner() {
    const { t } = useTranslation();
    const { phase, update, restart } = useUpdates();

    if (phase === "ready") {
        return (
            <div className="mb-6 flex items-center justify-between gap-4 rounded-md border bg-card px-4 py-2 text-sm">
                <span>{t("updates.bannerReady")}</span>
                <Button size="sm" onClick={() => void restart()}>
                    {t("updates.restart")}
                </Button>
            </div>
        );
    }
    if (phase === "available" && update) {
        return (
            <div className="mb-6 flex items-center justify-between gap-4 rounded-md border bg-card px-4 py-2 text-sm">
                <span>{t("updates.bannerAvailable", { version: update.version })}</span>
                <Button size="sm" variant="outline" asChild>
                    <Link to="/settings">{t("updates.details")}</Link>
                </Button>
            </div>
        );
    }
    return null;
}
