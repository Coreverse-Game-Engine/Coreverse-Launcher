import { Compass, Library, Settings, UserRound, type LucideIcon } from "lucide-react";
import { useTranslation } from "react-i18next";
import { NavLink, Outlet } from "react-router";

import { Emblem } from "@/components/brand/emblem";
import { LanguageSwitcher } from "@/components/language-switcher";
import { ThemeToggle } from "@/components/theme-toggle";
import { UpdateBanner } from "@/components/update-banner";
import { Separator } from "@/components/ui/separator";
import { cn } from "@/lib/utils";

interface NavItem {
    to: string;
    labelKey: string;
    icon: LucideIcon;
    end?: boolean;
}

const NAV_ITEMS: readonly NavItem[] = [
    { to: "/", labelKey: "nav.home", icon: Compass, end: true },
    { to: "/library", labelKey: "nav.library", icon: Library },
    { to: "/account", labelKey: "nav.account", icon: UserRound },
    { to: "/settings", labelKey: "nav.settings", icon: Settings },
];

export function AppShell() {
    const { t } = useTranslation();

    return (
        <div className="flex h-screen bg-background text-foreground">
            <aside className="flex w-60 shrink-0 flex-col border-r bg-card">
                <div className="flex items-center gap-2 px-5 py-5">
                    <Emblem className="h-6 text-primary" />
                    <span className="text-lg font-semibold tracking-tight">{t("app.name")}</span>
                </div>
                <nav className="flex flex-1 flex-col gap-1 px-3">
                    {NAV_ITEMS.map(({ to, labelKey, icon: Icon, end }) => (
                        <NavLink
                            key={to}
                            to={to}
                            end={end}
                            className={({ isActive }) =>
                                cn(
                                    "flex items-center gap-3 rounded-md px-3 py-2 text-sm font-medium transition-colors",
                                    isActive
                                        ? "bg-accent text-accent-foreground"
                                        : "text-muted-foreground hover:bg-accent/60 hover:text-accent-foreground",
                                )
                            }
                        >
                            <Icon className="size-4" aria-hidden="true" />
                            {t(labelKey)}
                        </NavLink>
                    ))}
                </nav>
                <Separator />
                <div className="flex items-center justify-between px-3 py-3">
                    <LanguageSwitcher />
                    <ThemeToggle />
                </div>
            </aside>
            <main className="flex-1 overflow-y-auto p-8">
                <UpdateBanner />
                <Outlet />
            </main>
        </div>
    );
}
