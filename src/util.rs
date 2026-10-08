use anyhow::{Context, Result, bail, ensure};
use serde::Serialize;
use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Component, Path, PathBuf},
};

pub fn id(prefix: &str) -> String {
    format!("{prefix}_{}", uuid::Uuid::new_v4().simple())
}
pub fn now() -> String {
    chrono::Utc::now().to_rfc3339()
}
pub fn digest(bytes: impl AsRef<[u8]>) -> String {
    format!("blake3:{}", blake3::hash(bytes.as_ref()).to_hex())
}
pub fn json_digest(value: &impl Serialize) -> Result<String> {
    Ok(digest(serde_json::to_vec(value)?))
}

pub fn private_dir(path: &Path) -> Result<()> {
    reject_symlinks(path)?;
    fs::create_dir_all(path).with_context(|| format!("create directory {}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

/// Refuse symlink traversal at a filesystem boundary. Call on normalized paths.
pub fn reject_symlinks(path: &Path) -> Result<()> {
    let mut current = PathBuf::new();
    for component in path.components() {
        current.push(component);
        // A Windows drive/UNC prefix alone is not a filesystem entry. Inspect
        // the complete root on the next component, then every descendant.
        if matches!(component, Component::Prefix(_)) {
            continue;
        }
        match fs::symlink_metadata(&current) {
            Ok(metadata) => ensure!(
                !metadata.file_type().is_symlink(),
                "symlink not allowed: {}",
                current.display()
            ),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e).with_context(|| format!("inspect {}", current.display())),
        }
    }
    Ok(())
}

pub fn absolute(base: &Path, path: &Path) -> Result<PathBuf> {
    let joined = if path.is_absolute() {
        path.to_owned()
    } else {
        base.join(path)
    };
    let mut normalized = PathBuf::new();
    for c in joined.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => {
                ensure!(normalized.pop(), "path escapes filesystem root");
            }
            _ => normalized.push(c),
        }
    }
    ensure!(normalized.is_absolute(), "expected an absolute path");
    reject_symlinks(&normalized)?;
    Ok(normalized)
}

/// Replace a small metadata file without an interval in which it is truncated.
/// The old file is retained until the new file is synced; publication recovery
/// uses content-addressed stages, not this helper, for its multi-file switch.
pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    reject_symlinks(path)?;
    let parent = path.parent().context("file has no parent")?;
    fs::create_dir_all(parent)?;
    let temporary = parent.join(format!(".lore-write-{}", uuid::Uuid::new_v4()));
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let result = (|| -> Result<()> {
        let mut file = options.open(&temporary)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        // Windows rename does not replace an existing destination. Metadata that
        // requires a replacement uses a backup and is recovered by its caller.
        #[cfg(windows)]
        if path.exists() {
            fs::remove_file(path)?;
        }
        fs::rename(&temporary, path)?;
        sync_directory(parent)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

pub fn sync_directory(path: &Path) -> Result<()> {
    #[cfg(unix)]
    File::open(path)?.sync_all()?;
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

pub fn safe_slug(value: &str) -> Result<()> {
    ensure!(
        !value.is_empty()
            && value.len() <= 80
            && !value.starts_with('-')
            && !value.ends_with('-')
            && value
                .bytes()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-'),
        "invalid topic slug (expected lowercase words separated by hyphens)"
    );
    ensure!(value != "index", "index is reserved");
    Ok(())
}

pub fn markdown_text(text: &str) -> String {
    // Source/model strings are prose, not trusted Markdown or HTML.
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('\\', "\\\\")
        .replace('[', "\\[")
        .replace(']', "\\]")
        .replace('*', "\\*")
        .replace('_', "\\_")
        .replace('`', "\\`")
        .replace('#', "\\#")
}

pub fn read_limited(path: &Path, max: usize) -> Result<String> {
    use std::io::Read;
    reject_symlinks(path)?;
    let file = File::open(path).with_context(|| format!("read {}", path.display()))?;
    ensure!(
        file.metadata()?.len() <= max as u64,
        "file exceeds configured limit: {}",
        path.display()
    );
    let mut bytes = Vec::new();
    file.take(max as u64 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > max {
        bail!("file grew beyond configured limit: {}", path.display());
    }
    String::from_utf8(bytes).context("Markdown/configuration must be valid UTF-8")
}
