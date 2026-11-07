import { Route, Routes } from "react-router";

import { AppShell } from "@/components/layout/app-shell";
import { AccountPage } from "@/pages/account";
import { HomePage } from "@/pages/home";
import { LibraryPage } from "@/pages/library";
import { NotFoundPage } from "@/pages/not-found";
import { SettingsPage } from "@/pages/settings";

export function App() {
    return (
        <Routes>
            <Route element={<AppShell />}>
                <Route index element={<HomePage />} />
                <Route path="library" element={<LibraryPage />} />
                <Route path="account" element={<AccountPage />} />
                <Route path="settings" element={<SettingsPage />} />
                <Route path="*" element={<NotFoundPage />} />
            </Route>
        </Routes>
    );
}
