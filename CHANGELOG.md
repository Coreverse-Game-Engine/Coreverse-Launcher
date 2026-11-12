# Changelog

All notable changes to Coreverse Launcher are written down here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and versions are plain `X.Y.Z`.

## [Unreleased]

## [0.2.0] - 2026-11-12

### Added

- Incremental engine updates. When an older engine version is installed, the launcher asks
  Coreverse DB (0.6.0 or later) for an update plan, downloads only a file manifest and small
  delta packages, copies the installed version and patches the copy, then verifies every file
  against the manifest. Anything that goes wrong (no plan, a modified or damaged base, a failed
  check) falls back to the normal full download. New install phases: planning, copying and
  patching.
- "Delete the old version" check box next to the install button (off by default, not
  remembered). After a successful install it removes the newest older version of the same
  platform, whichever way the new one was obtained.
- GitHub and Google logos on the social sign-in buttons (the GitHub mark follows the theme).

## [0.1.1]

### Fixed

- Installer images
- DB configuration in release builds.

[Unreleased]: https://github.com/Coreverse-Game-Engine/Coreverse-Launcher/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/Coreverse-Game-Engine/Coreverse-Launcher/compare/v0.1.1...v0.2.0
