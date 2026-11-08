//! How this copy of the launcher was installed.
//!
//! The in-app updater replaces the running installation. That is only right where the launcher
//! owns the installation: a Windows MSI, a macOS app bundle or a Linux `AppImage`. Files that a
//! package manager owns (a `.deb` under `/usr`, a Scoop app folder) have to be updated by that
//! package manager, and a development build or a copy somebody unpacked by hand must not be
//! replaced at all.

use std::path::Path;

use serde::Serialize;

/// The way the running launcher was installed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum InstallChannel {
    /// A Windows installer (the MSI, which is also what winget installs) or a macOS app
    /// bundle (`.dmg`, `.pkg` or the Homebrew cask).
    Installer,
    /// A Linux `AppImage`.
    AppImage,
    /// A Linux package (`.deb`, `.rpm`, pacman) installed under `/usr`.
    SystemPackage,
    /// A portable copy managed by Scoop.
    Scoop,
    /// Anything else: a development build or a copy that was unpacked by hand.
    Unmanaged,
}

impl InstallChannel {
    /// Detects the channel of the running launcher.
    #[must_use]
    pub fn detect() -> Self {
        let appimage = std::env::var_os("APPIMAGE").is_some_and(|value| !value.is_empty());
        match std::env::current_exe() {
            Ok(exe) => Self::from_parts(std::env::consts::OS, &exe, appimage),
            Err(_) => Self::Unmanaged,
        }
    }

    /// Works out the channel from the operating system (as in `std::env::consts::OS`), the
    /// path of the running executable and whether the `APPIMAGE` variable is set.
    #[must_use]
    pub fn from_parts(os: &str, exe: &Path, appimage: bool) -> Self {
        let path = exe.to_string_lossy().replace('\\', "/").to_lowercase();
        match os {
            "windows" => {
                if path.contains("/scoop/apps/") {
                    Self::Scoop
                } else if is_build_output(&path) {
                    Self::Unmanaged
                } else {
                    Self::Installer
                }
            }
            "macos" => {
                if path.contains(".app/contents/macos/") {
                    Self::Installer
                } else {
                    Self::Unmanaged
                }
            }
            "linux" => {
                if appimage {
                    Self::AppImage
                } else if path.starts_with("/usr/") && !path.starts_with("/usr/local/") {
                    Self::SystemPackage
                } else {
                    Self::Unmanaged
                }
            }
            _ => Self::Unmanaged,
        }
    }

    /// Whether the in-app updater may replace this installation.
    #[must_use]
    pub fn can_self_update(self) -> bool {
        matches!(self, Self::Installer | Self::AppImage)
    }
}

/// `target/debug` and `target/release` hold the output of `cargo build`, not an installation.
fn is_build_output(path: &str) -> bool {
    path.contains("/target/debug/") || path.contains("/target/release/")
}

/// What the frontend needs to know about the installation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallInfo {
    pub channel: InstallChannel,
    /// Whether the in-app updater may replace this installation.
    pub self_update: bool,
}

impl InstallInfo {
    /// Detects how the running launcher was installed.
    #[must_use]
    pub fn detect() -> Self {
        Self::from_channel(InstallChannel::detect())
    }

    #[must_use]
    pub fn from_channel(channel: InstallChannel) -> Self {
        Self {
            channel,
            self_update: channel.can_self_update(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MSI: &str = r"C:\Program Files\Coreverse Launcher\coreverse-launcher.exe";
    const SCOOP: &str = r"D:\scoop\apps\coreverse-launcher\current\coreverse-launcher.exe";
    const WINDOWS_DEV: &str = r"D:\src\target\debug\coreverse-launcher.exe";
    const APP_BUNDLE: &str =
        "/Applications/Coreverse Launcher.app/Contents/MacOS/coreverse-launcher";
    const MACOS_DEV: &str = "/Users/ada/src/target/debug/coreverse-launcher";
    const APPIMAGE_MOUNT: &str = "/tmp/.mount_Coreve1/usr/bin/coreverse-launcher";
    const SYSTEM_BINARY: &str = "/usr/bin/coreverse-launcher";
    const LOCAL_BINARY: &str = "/usr/local/bin/coreverse-launcher";
    const LINUX_DEV: &str = "/home/ada/src/target/release/coreverse-launcher";

    fn channel(os: &str, exe: &str, appimage: bool) -> InstallChannel {
        InstallChannel::from_parts(os, Path::new(exe), appimage)
    }

    #[test]
    fn windows_installs_and_scoop_are_told_apart() {
        assert_eq!(channel("windows", MSI, false), InstallChannel::Installer);
        assert_eq!(channel("windows", SCOOP, false), InstallChannel::Scoop);
        assert_eq!(
            channel("windows", WINDOWS_DEV, false),
            InstallChannel::Unmanaged
        );
    }

    #[test]
    fn macos_needs_an_app_bundle() {
        assert_eq!(
            channel("macos", APP_BUNDLE, false),
            InstallChannel::Installer
        );
        assert_eq!(
            channel("macos", MACOS_DEV, false),
            InstallChannel::Unmanaged
        );
    }

    #[test]
    fn linux_distinguishes_appimage_system_package_and_the_rest() {
        assert_eq!(
            channel("linux", APPIMAGE_MOUNT, true),
            InstallChannel::AppImage
        );
        assert_eq!(
            channel("linux", SYSTEM_BINARY, false),
            InstallChannel::SystemPackage
        );
        assert_eq!(
            channel("linux", LOCAL_BINARY, false),
            InstallChannel::Unmanaged
        );
        assert_eq!(
            channel("linux", LINUX_DEV, false),
            InstallChannel::Unmanaged
        );
    }

    #[test]
    fn unknown_systems_are_never_updated_in_place() {
        assert_eq!(
            channel("freebsd", SYSTEM_BINARY, false),
            InstallChannel::Unmanaged
        );
    }

    #[test]
    fn only_installer_and_appimage_update_themselves() {
        assert!(InstallChannel::Installer.can_self_update());
        assert!(InstallChannel::AppImage.can_self_update());
        assert!(!InstallChannel::SystemPackage.can_self_update());
        assert!(!InstallChannel::Scoop.can_self_update());
        assert!(!InstallChannel::Unmanaged.can_self_update());
    }

    #[test]
    fn serializes_as_camel_case() {
        let info = InstallInfo::from_channel(InstallChannel::AppImage);
        let json = serde_json::to_string(&info).unwrap();
        assert_eq!(json, r#"{"channel":"appImage","selfUpdate":true}"#);

        let info = InstallInfo::from_channel(InstallChannel::Scoop);
        let json = serde_json::to_string(&info).unwrap();
        assert_eq!(json, r#"{"channel":"scoop","selfUpdate":false}"#);
    }
}
