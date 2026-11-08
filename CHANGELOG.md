# Changelog

All notable changes to Coreverse Launcher are written down here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and versions are plain `X.Y.Z`.

## [0.1.0]

### Added

- Desktop launcher built with Tauri 2, React and TypeScript, with a light and a dark theme and
  English and Turkish translations.
- Sign-in with email and password, GitHub or Google through Coreverse DB; the session is kept
  in the operating system's credential store.
- Library page that installs Coreverse Engine versions from Coreverse DB: resumable download,
  SHA-256 verification against the DB's value and a safe unpack.
- Packaging for Windows (MSI), macOS (DMG and PKG) and Linux (AppImage, .deb, .rpm and pacman
  package) on x86_64 and arm64, built and signed by the release workflow.
- In-app updates with minisign-signed update files, offered where the launcher owns its
  installation (MSI, macOS app bundle, AppImage). Package-managed installs show how to update.
- Installation through winget, Scoop, Homebrew, the AUR and an install script for Linux.
