import { fileURLToPath, URL } from "node:url";

import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

// https://vite.dev/config/
export default defineConfig({
    plugins: [react(), tailwindcss()],
    resolve: {
        alias: { "@": fileURLToPath(new URL("./src", import.meta.url)) },
    },
    // Expose `PUBLIC_*` too, so `.env.local` uses the same variable names as Coreverse-DB.
    envPrefix: ["VITE_", "PUBLIC_"],
    // Keep Rust compiler messages visible in the terminal.
    clearScreen: false,
    server: {
        port: 1420,
        strictPort: true,
        watch: { ignored: ["**/src-tauri/**", "**/crates/**", "**/target/**"] },
    },
    build: {
        target: "es2022",
    },
});
