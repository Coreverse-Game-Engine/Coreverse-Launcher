import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { HashRouter } from "react-router";

import "./i18n";
import "./index.css";

import { App } from "@/App";
import { AuthProvider } from "@/components/auth-provider";
import { EnginesProvider } from "@/components/engines-provider";
import { SettingsBridge } from "@/components/settings-bridge";
import { ThemeProvider } from "@/components/theme-provider";
import { UpdatesProvider } from "@/components/updates-provider";
import { configureDb } from "@/lib/db";
import { applyTheme, readStoredTheme } from "@/lib/theme";

// Apply the saved theme before the first render so there is no flash of the wrong theme.
applyTheme(readStoredTheme());

configureDb();

const container = document.getElementById("root");
if (!container) {
    throw new Error("Root element #root was not found.");
}

createRoot(container).render(
    <StrictMode>
        <ThemeProvider>
            <SettingsBridge />
            <AuthProvider>
                <EnginesProvider>
                    <UpdatesProvider>
                        <HashRouter>
                            <App />
                        </HashRouter>
                    </UpdatesProvider>
                </EnginesProvider>
            </AuthProvider>
        </ThemeProvider>
    </StrictMode>,
);
