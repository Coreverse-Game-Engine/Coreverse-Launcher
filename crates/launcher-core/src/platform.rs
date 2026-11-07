/// The operating system and CPU architecture the launcher runs on, spelled the way Coreverse DB
/// spells them in `engine_artifacts` (`windows|linux|macos`, `x86_64|arm64`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct Platform {
    pub os: &'static str,
    pub architecture: &'static str,
}

impl Platform {
    /// The platform this binary was built for, or `None` if the DB has no name for it.
    #[must_use]
    pub fn current() -> Option<Self> {
        Self::from_rust_consts(std::env::consts::OS, std::env::consts::ARCH)
    }

    /// Maps Rust's `std::env::consts` names to the DB's names.
    #[must_use]
    pub fn from_rust_consts(os: &str, arch: &str) -> Option<Self> {
        let os = match os {
            "windows" => "windows",
            "linux" => "linux",
            "macos" => "macos",
            _ => return None,
        };
        let architecture = match arch {
            "x86_64" => "x86_64",
            "aarch64" => "arm64",
            _ => return None,
        };
        Some(Self { os, architecture })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_rust_names_to_db_names() {
        let platform = Platform::from_rust_consts("macos", "aarch64").unwrap();
        assert_eq!(platform.os, "macos");
        assert_eq!(platform.architecture, "arm64");
        assert_eq!(
            Platform::from_rust_consts("linux", "x86_64"),
            Some(Platform {
                os: "linux",
                architecture: "x86_64"
            })
        );
    }

    #[test]
    fn unknown_platforms_are_rejected() {
        assert_eq!(Platform::from_rust_consts("freebsd", "x86_64"), None);
        assert_eq!(Platform::from_rust_consts("linux", "riscv64"), None);
    }
}
