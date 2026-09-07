//! Discover, inspect, select, download and provision JVM/JDK installations.
//!
//! ```no_run
//! use java_path::{discover, SelectExt};
//!
//! let installs = discover()?;
//! let java = installs.select().major(21).jdk().best()?;
//! println!("{}", java.home.display());
//! # Ok::<(), java_path::Error>(())
//! ```
//!
//! # Features
//!
//! Discovery and inspection are always available and pull in no dependencies
//! beyond `thiserror`. Everything else is opt-in:
//!
//! * `serde` — `Serialize`/`Deserialize` for the model types.
//! * `network` — the Adoptium release API.
//! * `install` — download, verify and extract JDKs (implies `network`).

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod discovery;
pub mod error;
pub mod inspect;
pub mod model;
pub mod process;
pub mod provision;
pub mod selector;
pub mod version;

pub use discovery::{discover, Discovery};
pub use error::{Error, Result};
pub use inspect::{inspect_java_home, inspect_java_home_with, InspectOptions};
pub use model::{
    Architecture, DiscoverySource, JavaInstallation, JavaKind, JavaMetadata, Platform,
};
pub use selector::{JavaQuery, SelectExt, Selector};
pub use version::JavaVersion;

#[cfg(feature = "network")]
pub use provision::adoptium::AdoptiumProvider;
#[cfg(feature = "install")]
pub use provision::{target_dir_name, InstallEvent, JavaInstaller};
pub use provision::{JdkProvider, JdkRelease, ReleaseRequest, ReleaseType, Vendor, VersionSpec};

/// Entry point combining discovery and selection.
pub struct Java;

impl Java {
    /// Discover every installation on this machine.
    pub fn discover() -> Result<Vec<JavaInstallation>> {
        discovery::discover()
    }

    /// The installation a plain `java` invocation would actually use.
    ///
    /// This resolves `PATH` the way the shell does: the *first* `java` on
    /// `PATH` wins, regardless of version. `JAVA_HOME` is not consulted,
    /// because it does not decide what `java` runs — only `PATH` does; use
    /// [`Java::preferred`] for the `JAVA_HOME`-first answer.
    pub fn current() -> Result<JavaInstallation> {
        discovery::path::first_on_path().ok_or(Error::NoMatch)
    }

    /// The installation a caller should prefer.
    ///
    /// `JAVA_HOME` wins when it points at a usable Java home; otherwise this
    /// is the newest installation reachable through `PATH`. This is the
    /// convention most build tools follow, and it is deliberately *not* the
    /// same question as [`Java::current`].
    pub fn preferred() -> Result<JavaInstallation> {
        let installs = Discovery::new()
            .system_dirs(false)
            .tool_stores(false)
            .search()?;
        installs
            .iter()
            .find(|i| i.source == DiscoverySource::JavaHome)
            .or_else(|| installs.first())
            .cloned()
            .ok_or(Error::NoMatch)
    }
}
