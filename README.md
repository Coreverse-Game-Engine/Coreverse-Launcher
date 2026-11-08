# Coreverse Launcher

The official launcher of the Coreverse ecosystem.

## Stack

| Area      | Tools                                                                 |
| --------- | --------------------------------------------------------------------- |
| Desktop   | Tauri 2, Rust (edition 2024), Cargo workspace                         |
| Frontend  | React, TypeScript, Vite, TailwindCSS, shadcn/ui, Radix UI             |
| Data      | SQLite (local storage), `@Coreverse-Game-Engine/db-client` (Supabase) |
| Quality   | ESLint, Prettier, rustfmt, Clippy                                     |
| Packaging | pnpm                                                                  |

## Requirements

- Node.js 22.18 or newer and pnpm 12 (the version is pinned in `packageManager`)
- A GitHub personal access token with `read:packages`, saved in your user `~/.npmrc` as
  `//npm.pkg.github.com/:_authToken=<token>`, to install `@Coreverse-Game-Engine/db-client`
- Rust (stable, installed through `rustup`; `rust-toolchain.toml` picks the components)
- The Tauri system prerequisites for your OS: <https://tauri.app/start/prerequisites/>
- Linux only, in addition: the libdbus development files and `pkg-config` for the keyring
  (Debian: `libdbus-1-dev pkg-config`, Fedora: `dbus-devel pkgconf-pkg-config`,
  Arch: `dbus pkgconf`, openSUSE: `dbus-1-devel pkgconf-pkg-config`), and a Secret Service provider such
  as GNOME Keyring or KWallet at runtime

## Getting started

```sh
pnpm install
pnpm tauri dev
```

`pnpm dev` starts only the web UI in a browser (native features are disabled there).

## Scripts

| Script                                | Purpose                                                  |
| ------------------------------------- | -------------------------------------------------------- |
| `pnpm tauri dev` / `pnpm tauri build` | Run or package the desktop app                           |
| `pnpm lint` / `pnpm format`           | ESLint / Prettier                                        |
| `pnpm typecheck`                      | Type check with the native TypeScript 7 compiler (`tsc`) |
| `pnpm typecheck:compat`               | Same check with the TypeScript 6 compatibility build     |
| `pnpm rust:fmt` / `pnpm rust:clippy`  | rustfmt / Clippy (`-D warnings`)                         |
| `pnpm test` / `pnpm test:watch`       | Frontend tests (Vitest, Testing Library, jsdom)          |
| `pnpm rust:test`                      | Rust tests of the whole workspace                        |
| `pnpm release:bump <x>`               | Raise the version everywhere (`patch`, `minor`, `X.Y.Z`) |
| `pnpm release:check` / `:test`        | Version sync check / tests of the release scripts        |
| `pnpm release:pubkey <file>`          | Store the updater's public key in `tauri.conf.json`      |
| `pnpm verify`                         | Everything above, in CI order                            |

## Quality checks and CI

`pnpm verify` runs everything CI runs, in the same order: Prettier, ESLint, type checks
(native TypeScript 7 and the TypeScript 6 compatibility build), Vitest, the production build,
`cargo fmt --check`, Clippy with `-D warnings` and `cargo test`.

`.github/workflows/ci.yml` runs on every push to `main` and every pull request:

- **Frontend** on Ubuntu: format, lint, both type checks, tests and the build.
- **Rust** on Linux, Windows, macOS (arm64) and macOS (x64): `fmt`, Clippy and tests.
- **Linux distributions** in containers (Debian, Fedora, Arch, openSUSE): the workspace has to
  compile and pass its tests with each distribution's own system libraries.

Installing `@Coreverse-Game-Engine/db-client` in CI uses the repository secret `GH_PACKAGES_TOKEN`
(a token with `read:packages`) when it exists and the workflow's own `GITHUB_TOKEN` otherwise. The
workflow token only works after the package grants this repository access: organization ->
Packages -> db-client -> Package settings -> _Manage Actions access_ -> add this repository with
the Read role. The "Check access to the db-client package" step prints the HTTP status the
registry answers with, which tells the two failure modes apart (403: no access, 404: wrong name).
Dependabot needs the same secret under _Settings -> Secrets -> Dependabot_.

`.github/workflows/audit.yml` runs `cargo audit` and `pnpm audit` weekly and when a lockfile
changes. It is a report, not a merge gate. Commit `pnpm-lock.yaml` and `Cargo.lock`; CI installs
with `--frozen-lockfile`.

## Layout

```text
crates/launcher-core/   Tauri-independent Rust logic (testable with plain cargo test)
src-tauri/              Tauri shell: window, IPC commands, bundling
src/                    React UI (components, pages, i18n, theme)
```

## Accounts and sign-in

Sign-in goes through Supabase Auth (email and password, GitHub, Google). Copy `.env.example`
to `.env.local` and fill in the project URL and anon key (release builds read the same public values from `.env.production`); without them the Account page says
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

## Installing engines

The Library page installs Coreverse Engine versions published in Coreverse DB:

1. The Rust side reads the release from `GET /releases/{version}` itself, so the download URL and
   the SHA-256 never pass through the webview, and picks the build for this OS and architecture.
2. It downloads the archive over HTTPS. A partial download (after a cancel, a crash or a lost
   connection) is resumed with a range request.
3. It verifies the file against the **SHA-256 from Coreverse DB**, not from the download source,
   and discards the file on a mismatch.
4. It unpacks into a staging folder and only then moves the result to `engines/<version>`;
   a failed or cancelled install leaves nothing half-installed behind.

Archives must be `.zip`, `.tar.gz` or `.tgz` (decided from the artifact's `download_url`). They
are unpacked defensively: no paths outside the install folder, no hard links or special files,
symbolic links only when they point inside the folder (not supported on Windows), and a size cap.
If the archive holds a single top-level folder, that folder becomes the engine's root.

Without a connection the Library still lists and removes installed engines; only the update
check and installing need the network.

## Packaging and releases

A `vX.Y.Z` tag runs `.github/workflows/release.yml`, which builds, signs and packages the
launcher for Windows (MSI), macOS (DMG, PKG) and Linux (AppImage, `.deb`, `.rpm`, pacman
package), each on x86_64 and arm64, and prepares a draft release. Publishing it feeds the
in-app updater and the winget, Scoop, Homebrew and AUR packages.

- [docs/releasing.md](docs/releasing.md): one-time setup (signing keys, secrets), cutting a
  release, what is built and how to verify it.
- [docs/distribution.md](docs/distribution.md): every way to install and update, the file
  naming and the `downloads.json` the Website's download panel reads.

Updates are signed with a minisign key (`plugins.updater` in `src-tauri/tauri.conf.json`) and
only offered where the launcher owns its installation; package-managed installs show how to
update them instead.

## Data and logs

Settings and the list of installed engines live in a SQLite database (`launcher.db`, schema
migrations in `crates/launcher-core/migrations/`), the engines themselves in `engines/<version>`
next to it, and logs in a daily rolling `launcher.log`. Both sit in
the OS-specific app data and app log folders of the identifier `com.coreverse.launcher`
(on Windows: `%APPDATA%` and `%LOCALAPPDATA%`). Set `RUST_LOG=debug` for more detail.

## License

GPL-3.0-only. See [LICENSE](LICENSE).
