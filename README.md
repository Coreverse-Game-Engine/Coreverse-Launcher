# Coreverse Launcher

The official launcher of the Coreverse ecosystem.

## Stack

| Area      | Tools                                                                  |
| --------- | ---------------------------------------------------------------------- |
| Desktop   | Tauri 2, Rust (edition 2024), Cargo workspace                          |
| Frontend  | React, TypeScript, Vite, TailwindCSS, shadcn/ui, Radix UI              |
| Data      | SQLite (local storage), `@Coreverse-Game-Engine/db-client` (Supabase)  |
| Quality   | ESLint, Prettier, rustfmt, Clippy                                      |
| Packaging | pnpm                                                                   |

## Requirements

- Node.js 22.18 or newer and pnpm 12 (the version is pinned in `packageManager`)
- A GitHub personal access token with `read:packages`, saved in your user `~/.npmrc` as
  `//npm.pkg.github.com/:_authToken=<token>`, to install `@Coreverse-Game-Engine/db-client`
- Rust (stable, installed through `rustup`; `rust-toolchain.toml` picks the components)
- The Tauri system prerequisites for your OS: <https://tauri.app/start/prerequisites/>
- Linux only, in addition: the libdbus development files and `pkg-config` for the keyring
  (Debian: `libdbus-1-dev pkg-config`, Fedora: `dbus-devel pkgconf-pkg-config`,
  Arch: `dbus pkgconf`, openSUSE: `dbus-1-devel pkg-config`), and a Secret Service provider such
  as GNOME Keyring or KWallet at runtime

## Getting started

```sh
pnpm install
pnpm tauri dev
```

`pnpm dev` starts only the web UI in a browser (native features are disabled there).

## Scripts

| Script                                       | Purpose                                                     |
| -------------------------------------------- | ----------------------------------------------------------- |
| `pnpm tauri dev` / `pnpm tauri build`        | Run or package the desktop app                              |
| `pnpm lint` / `pnpm format`                  | ESLint / Prettier                                           |
| `pnpm typecheck`                             | Type check with the native TypeScript 7 compiler (`tsc`)    |
| `pnpm typecheck:compat`                      | Same check with the TypeScript 6 compatibility build        |
| `pnpm rust:fmt` / `pnpm rust:clippy`         | rustfmt / Clippy (`-D warnings`)                            |
| `pnpm rust:test`                             | Rust tests of the whole workspace                           |
| `pnpm verify`                                | Everything above, in CI order                               |

## Layout

```text
crates/launcher-core/   Tauri-independent Rust logic (testable with plain cargo test)
src-tauri/              Tauri shell: window, IPC commands, bundling
src/                    React UI (components, pages, i18n, theme)
```

## Accounts and sign-in

Sign-in goes through Supabase Auth (email and password, GitHub, Google). Copy `.env.example`
to `.env.local` and fill in the project URL and anon key; without them the Account page says
sign-in is not configured.

- The variable names (`PUBLIC_SUPABASE_URL`, `PUBLIC_SUPABASE_ANON_KEY`) are the same as in
  Coreverse-DB.
- OAuth opens the system browser and comes back through the `coreverse-launcher://auth/callback`
  deep link. Add that URL to the Supabase project's **redirect URL allow-list**
  (`additional_redirect_urls` in the DB repo's `supabase/config.toml`).
- Only the refresh token is saved, in the OS credential store (Windows Credential Manager,
  macOS Keychain, Secret Service on Linux). If no keyring is available the user simply signs
  in again on the next start.
- Deep links need a bundled app on macOS, so test OAuth there with `pnpm tauri build`. Windows
  and Linux register the scheme at runtime, so `pnpm tauri dev` works.

## Coreverse DB

The launcher talks to the DB only through `@Coreverse-Game-Engine/db-client`, configured once at
startup in `src/lib/db.ts`:

- Requests go through Rust's HTTP client (`tauri-plugin-http`), not the webview's `fetch`, so
  they carry no `Origin` header and need no CORS entry on the DB. The allowed URL patterns are in
  `src-tauri/capabilities/default.json`; add your own host there if you use a custom domain.
- Signed in, every request carries the session's access token. Signed out, no token is sent:
  public reads (releases, news, ...) work without an account, which the launcher relies on to
  check for updates before sign-in.

## Data and logs

Settings live in a SQLite database (`launcher.db`, schema migrations in
`crates/launcher-core/migrations/`), logs in a daily rolling `launcher.log`. Both sit in
the OS-specific app data and app log folders of the identifier `com.coreverse.launcher`
(on Windows: `%APPDATA%` and `%LOCALAPPDATA%`). Set `RUST_LOG=debug` for more detail.

## License

GPL-3.0-only. See [LICENSE](LICENSE).
