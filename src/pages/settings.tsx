import { useTranslation } from "react-i18next";

import { UpdateCard } from "@/components/update-card";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Separator } from "@/components/ui/separator";
import { useTheme } from "@/hooks/use-theme";
import { SUPPORTED_LANGUAGES, setLanguage } from "@/i18n";
import { THEMES } from "@/lib/theme";

const LANGUAGE_NAMES: Record<(typeof SUPPORTED_LANGUAGES)[number], string> = {
    en: "English",
    tr: "Türkçe",
};

export function SettingsPage() {
    const { t, i18n } = useTranslation();
    const { theme, setTheme } = useTheme();

    return (
        <div className="mx-auto flex max-w-3xl flex-col gap-6">
            <h1 className="text-3xl font-semibold tracking-tight">{t("settings.title")}</h1>
            <Card>
                <CardHeader>
                    <CardTitle>{t("settings.appearanceTitle")}</CardTitle>
                </CardHeader>
                <CardContent className="flex flex-col gap-6">
                    <section className="flex flex-col gap-3">
                        <div>
                            <h2 className="text-sm font-medium">{t("settings.themeLabel")}</h2>
                            <CardDescription>{t("settings.themeDescription")}</CardDescription>
                        </div>
                        <div className="flex flex-wrap gap-2">
                            {THEMES.map((item) => (
                                <Button
                                    key={item}
                                    variant={theme === item ? "default" : "outline"}
                                    aria-pressed={theme === item}
                                    onClick={() => setTheme(item)}
                                >
                                    {t(`theme.${item}`)}
                                </Button>
                            ))}
                        </div>
                    </section>
                    <Separator />
                    <section className="flex flex-col gap-3">
                        <div>
                            <h2 className="text-sm font-medium">{t("settings.languageLabel")}</h2>
                            <CardDescription>{t("settings.languageDescription")}</CardDescription>
                        </div>
                        <div className="flex flex-wrap gap-2">
                            {SUPPORTED_LANGUAGES.map((language) => (
                                <Button
                                    key={language}
                                    variant={
                                        i18n.resolvedLanguage === language ? "default" : "outline"
                                    }
                                    aria-pressed={i18n.resolvedLanguage === language}
                                    onClick={() => void setLanguage(language)}
                                >
                                    {LANGUAGE_NAMES[language]}
                                </Button>
                            ))}
                        </div>
                    </section>
                </CardContent>
            </Card>
            <UpdateCard />
        </div>
    );
}
