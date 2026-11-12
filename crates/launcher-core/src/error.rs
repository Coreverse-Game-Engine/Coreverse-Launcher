use serde::{
    Serialize,
    ser::{SerializeStruct, Serializer},
};

/// Result type used across the core crate.
pub type Result<T> = std::result::Result<T, Error>;

/// Every error the core can produce.
///
/// The type implements [`Serialize`] so it can be returned straight from a Tauri command;
/// the frontend receives `{ code, message }`.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),

    #[error(
        "the database was created by a newer launcher (schema version {found}, this launcher supports up to {supported})"
    )]
    DatabaseTooNew { found: u32, supported: u32 },

    #[error("logging could not be initialised: {0}")]
    Logging(String),

    #[error("the system keyring is not available: {0}")]
    SecretStore(String),

    #[error("network error: {0}")]
    Network(String),

    #[error("the server answered with HTTP status {status}")]
    Http { status: u16 },

    #[error("the operation was cancelled")]
    Cancelled,

    #[error("another installation is already running")]
    InstallInProgress,

    #[error("the launcher is not connected to Coreverse DB yet")]
    NotConfigured,

    #[error("this platform is not supported: {0}")]
    UnsupportedPlatform(String),

    #[error("{0:?} is not a valid engine version")]
    InvalidVersion(String),

    #[error("engine {0} is already installed")]
    AlreadyInstalled(String),

    #[error("engine {0} is not installed")]
    NotInstalled(String),

    #[error("engine release {0} was not found")]
    ReleaseNotFound(String),

    #[error("release {version} has no build for {os} {architecture}")]
    NoArtifact {
        version: String,
        os: String,
        architecture: String,
    },

    #[error("the download address is not allowed: {0}")]
    InvalidUrl(String),

    #[error("the download metadata is invalid: {0}")]
    InvalidMetadata(String),

    #[error("the archive format is not supported: {0}")]
    UnsupportedArchive(String),

    #[error("the archive is damaged or contains an unsafe entry: {0}")]
    UnsafeArchive(String),

    #[error("the downloaded file does not match its checksum")]
    ChecksumMismatch,

    #[error("the download has the wrong size (expected {expected} bytes, got {actual})")]
    SizeMismatch { expected: u64, actual: u64 },

    #[error("this installation cannot update itself")]
    UpdateNotSupported,

    #[error("the incremental update could not be applied: {0}")]
    Delta(String),

    #[error("the update failed: {0}")]
    Update(String),

    #[error("there is no update to install; check for updates first")]
    NoPendingUpdate,
}

impl Error {
    /// Stable machine-readable code for the frontend.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::Database(_) => "database",
            Self::Io(_) => "io",
            Self::DatabaseTooNew { .. } => "database_too_new",
            Self::Logging(_) => "logging",
            Self::SecretStore(_) => "secret_store",
            Self::Network(_) => "network",
            Self::Http { .. } => "http",
            Self::Cancelled => "cancelled",
            Self::InstallInProgress => "install_in_progress",
            Self::NotConfigured => "not_configured",
            Self::UnsupportedPlatform(_) => "unsupported_platform",
            Self::InvalidVersion(_) => "invalid_version",
            Self::AlreadyInstalled(_) => "already_installed",
            Self::NotInstalled(_) => "not_installed",
            Self::ReleaseNotFound(_) => "release_not_found",
            Self::NoArtifact { .. } => "no_artifact",
            Self::InvalidUrl(_) => "invalid_url",
            Self::InvalidMetadata(_) => "invalid_metadata",
            Self::UnsupportedArchive(_) => "unsupported_archive",
            Self::UnsafeArchive(_) => "unsafe_archive",
            Self::ChecksumMismatch => "checksum_mismatch",
            Self::SizeMismatch { .. } => "size_mismatch",
            Self::UpdateNotSupported => "update_not_supported",
            Self::Delta(_) => "delta_failed",
            Self::Update(_) => "update_failed",
            Self::NoPendingUpdate => "no_pending_update",
        }
    }
}

impl From<reqwest::Error> for Error {
    fn from(error: reqwest::Error) -> Self {
        // The URL can carry query parameters; the message does not need them.
        Self::Network(error.without_url().to_string())
    }
}

impl Serialize for Error {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        let mut state = serializer.serialize_struct("Error", 2)?;
        state.serialize_field("code", self.code())?;
        state.serialize_field("message", &self.to_string())?;
        state.end()
    }
}

/// A cloneable `{ code, message }` snapshot of an [`Error`], used in events.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ErrorInfo {
    pub code: &'static str,
    pub message: String,
}

impl From<&Error> for ErrorInfo {
    fn from(error: &Error) -> Self {
        Self {
            code: error.code(),
            message: error.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_code_and_message() {
        let error = Error::DatabaseTooNew {
            found: 9,
            supported: 1,
        };
        let value = serde_json::to_value(&error).unwrap();
        assert_eq!(value["code"], "database_too_new");
        assert!(
            value["message"]
                .as_str()
                .unwrap()
                .contains("schema version 9")
        );
    }

    #[test]
    fn error_info_copies_code_and_message() {
        let info = ErrorInfo::from(&Error::ChecksumMismatch);
        assert_eq!(info.code, "checksum_mismatch");
        assert!(info.message.contains("checksum"));
    }
}
