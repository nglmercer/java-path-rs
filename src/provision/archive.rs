//! Path-traversal-safe extraction of `.zip` and `.tar.gz` JDK archives.

use crate::error::{Error, Result};
use std::path::{Component, Path, PathBuf};

/// Extract a `.zip` or `.tar.gz` archive into `dest`.
///
/// Every entry path is validated before anything is written: absolute paths,
/// `..` components and symlinks pointing outside `dest` are rejected.
pub fn extract(archive: &Path, dest: &Path) -> Result<()> {
    std::fs::create_dir_all(dest).map_err(|e| Error::io(dest, e))?;
    let name = archive
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();

    if name.ends_with(".zip") {
        extract_zip(archive, dest)
    } else if name.ends_with(".tar.gz") || name.ends_with(".tgz") {
        extract_tar_gz(archive, dest)
    } else {
        Err(Error::UnsupportedArchive(name))
    }
}

/// Validate an archive entry path and join it onto `dest`.
pub fn safe_join(dest: &Path, entry: &Path) -> Result<PathBuf> {
    let mut out = dest.to_path_buf();
    for component in entry.components() {
        match component {
            Component::Normal(part) => out.push(part),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(Error::UnsafeArchiveEntry(entry.display().to_string()));
            }
        }
    }
    if !out.starts_with(dest) {
        return Err(Error::UnsafeArchiveEntry(entry.display().to_string()));
    }
    Ok(out)
}

fn extract_zip(archive: &Path, dest: &Path) -> Result<()> {
    let file = std::fs::File::open(archive).map_err(|e| Error::io(archive, e))?;
    let mut zip = zip::ZipArchive::new(file)
        .map_err(|e| Error::UnsupportedArchive(format!("{}: {e}", archive.display())))?;

    for i in 0..zip.len() {
        let mut entry = zip
            .by_index(i)
            .map_err(|e| Error::UnsupportedArchive(e.to_string()))?;
        let Some(name) = entry.enclosed_name() else {
            return Err(Error::UnsafeArchiveEntry(entry.name().to_string()));
        };
        let out = safe_join(dest, &name)?;

        if entry.is_dir() {
            std::fs::create_dir_all(&out).map_err(|e| Error::io(&out, e))?;
            continue;
        }
        if let Some(parent) = out.parent() {
            std::fs::create_dir_all(parent).map_err(|e| Error::io(parent, e))?;
        }
        let mut writer = std::fs::File::create(&out).map_err(|e| Error::io(&out, e))?;
        std::io::copy(&mut entry, &mut writer).map_err(|e| Error::io(&out, e))?;

        #[cfg(unix)]
        if let Some(mode) = entry.unix_mode() {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&out, std::fs::Permissions::from_mode(mode))
                .map_err(|e| Error::io(&out, e))?;
        }
    }
    Ok(())
}

fn extract_tar_gz(archive: &Path, dest: &Path) -> Result<()> {
    let file = std::fs::File::open(archive).map_err(|e| Error::io(archive, e))?;
    let decoder = flate2::read::GzDecoder::new(file);
    let mut tar = tar::Archive::new(decoder);
    tar.set_preserve_permissions(true);

    for entry in tar.entries().map_err(|e| Error::io(archive, e))? {
        let mut entry = entry.map_err(|e| Error::io(archive, e))?;
        let path = entry
            .path()
            .map_err(|e| Error::io(archive, e))?
            .into_owned();
        let out = safe_join(dest, &path)?;

        // Symlinks are validated against the destination root as well, so a
        // link cannot be used to escape the extraction directory later.
        if let Ok(Some(link)) = entry.link_name() {
            safe_join(dest, &link)
                .map_err(|_| Error::UnsafeArchiveEntry(link.display().to_string()))?;
        }

        if let Some(parent) = out.parent() {
            std::fs::create_dir_all(parent).map_err(|e| Error::io(parent, e))?;
        }
        entry.unpack(&out).map_err(|e| Error::io(&out, e))?;
    }
    Ok(())
}

/// Find the single Java home inside a freshly extracted archive.
///
/// JDK archives contain one top-level directory; on macOS that directory
/// holds a `Contents/Home` bundle.
pub fn find_extracted_home(dest: &Path) -> Result<PathBuf> {
    let mut children: Vec<PathBuf> = std::fs::read_dir(dest)
        .map_err(|e| Error::io(dest, e))?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    children.sort();

    for child in [dest.to_path_buf()].into_iter().chain(children) {
        if let Ok(layout) = crate::inspect::resolve_layout(&child) {
            return Ok(layout.home);
        }
    }
    Err(Error::NotAJavaHome {
        path: dest.to_path_buf(),
        reason: "no java home inside the extracted archive",
    })
}
