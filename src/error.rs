//! Structured error type for the crate.

use std::path::PathBuf;

/// Result alias used throughout the crate.
pub type Result<T> = std::result::Result<T, Error>;

/// Everything that can go wrong in `java-path`.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// A version string could not be parsed.
    #[error("invalid java version string: {0:?}")]
    InvalidVersion(String),

    /// The given path is not a usable Java home.
    #[error("not a java home: {path} ({reason})")]
    NotAJavaHome {
        /// The inspected path.
        path: PathBuf,
        /// Why it was rejected.
        reason: &'static str,
    },

    /// The `release` file could not be parsed.
    #[error("malformed release file at {path}: {reason}")]
    MalformedReleaseFile {
        /// Path of the release file.
        path: PathBuf,
        /// Why parsing failed.
        reason: String,
    },

    /// No installation matched a query.
    #[error("no java installation matched the query")]
    NoMatch,

    /// A subprocess failed.
    #[error("failed to run {program}: {reason}")]
    Command {
        /// Program that was invoked.
        program: String,
        /// Failure detail.
        reason: String,
    },

    /// Checksum mismatch on a downloaded artifact.
    #[error("checksum mismatch: expected {expected}, got {actual}")]
    ChecksumMismatch {
        /// Expected hex digest.
        expected: String,
        /// Computed hex digest.
        actual: String,
    },

    /// An archive entry tried to escape the extraction root.
    #[error("unsafe archive entry: {0}")]
    UnsafeArchiveEntry(String),

    /// An unsupported archive format was encountered.
    #[error("unsupported archive format: {0}")]
    UnsupportedArchive(String),

    /// No release/binary was available for the requested combination.
    #[error("no jdk release available: {0}")]
    NoRelease(String),

    /// A network or HTTP-level failure.
    #[error("network error: {0}")]
    Network(String),

    /// An I/O failure, with the path involved when known.
    #[error("io error at {path}: {source}")]
    Io {
        /// Path involved, or `<unknown>`.
        path: PathBuf,
        /// Underlying error.
        #[source]
        source: std::io::Error,
    },
}

impl Error {
    pub(crate) fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Error::Io {
            path: path.into(),
            source,
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(source: std::io::Error) -> Self {
        Error::Io {
            path: PathBuf::from("<unknown>"),
            source,
        }
    }
}
