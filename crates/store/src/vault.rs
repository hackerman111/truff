use std::fmt;
use std::fs::{self, File};
use std::io::{self, Read, Seek, Write};
use std::path::{Component, Path, PathBuf};

/// Errors relating to vault operations and paths.
#[derive(Debug)]
pub enum VaultError {
    Io(io::Error),
    PathOutsideVault(PathBuf),
    LockBusy { pid: u32 },
}

impl fmt::Display for VaultError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(err) => write!(f, "vault I/O error: {err}"),
            Self::PathOutsideVault(p) => {
                write!(f, "path {} escapes outside the vault root", p.display())
            }
            Self::LockBusy { pid } => {
                write!(f, "vault is already locked by another process (PID {pid})")
            }
        }
    }
}

impl std::error::Error for VaultError {}

impl From<io::Error> for VaultError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}

/// Paths associated with a vault directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VaultPaths {
    root: PathBuf,
    truff_dir: PathBuf,
    recovery_dir: PathBuf,
    trash_dir: PathBuf,
}

impl VaultPaths {
    /// Creates a new `VaultPaths` manager for the given vault root.
    pub fn new(root: PathBuf) -> Self {
        let root = root.canonicalize().unwrap_or_else(|_| {
            if root.is_absolute() {
                root
            } else {
                std::env::current_dir().unwrap_or_default().join(root)
            }
        });
        let truff_dir = root.join(".truff");
        let recovery_dir = truff_dir.join("recovery");
        let trash_dir = truff_dir.join("trash");
        Self {
            root,
            truff_dir,
            recovery_dir,
            trash_dir,
        }
    }

    /// Root directory of the vault.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Internal `.truff` configuration and metadata directory.
    pub fn truff_dir(&self) -> &Path {
        &self.truff_dir
    }

    /// Recovery snapshots directory (`.truff/recovery`).
    pub fn recovery_dir(&self) -> &Path {
        &self.recovery_dir
    }

    /// Trash directory (`.truff/trash`).
    pub fn trash_dir(&self) -> &Path {
        &self.trash_dir
    }

    /// Resolves a path relative to the vault root, preventing path traversal attacks.
    pub fn resolve_path(&self, rel: &Path) -> Result<PathBuf, VaultError> {
        let mut normalized = PathBuf::new();
        for comp in rel.components() {
            match comp {
                Component::Normal(c) => normalized.push(c),
                Component::CurDir => {}
                Component::ParentDir => {
                    return Err(VaultError::PathOutsideVault(rel.to_path_buf()));
                }
                Component::RootDir | Component::Prefix(_) => {
                    // Absolute path: check if inside root
                    if let Ok(stripped) = rel.strip_prefix(&self.root) {
                        return self.resolve_path(stripped);
                    }
                    return Err(VaultError::PathOutsideVault(rel.to_path_buf()));
                }
            }
        }
        if normalized.as_os_str().is_empty()
            || normalized.components().next()
                == Some(Component::Normal(std::ffi::OsStr::new(".truff")))
        {
            return Err(VaultError::PathOutsideVault(rel.to_path_buf()));
        }
        let mut checked = self.root.clone();
        for component in normalized.components() {
            checked.push(component);
            match fs::symlink_metadata(&checked) {
                Ok(meta) if meta.file_type().is_symlink() => {
                    return Err(VaultError::PathOutsideVault(rel.to_path_buf()));
                }
                Ok(_) => {}
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
            }
        }
        Ok(checked)
    }

    /// Strips the vault root prefix from an absolute path.
    pub fn relative_path<'a>(&self, path: &'a Path) -> Result<&'a Path, VaultError> {
        path.strip_prefix(&self.root)
            .map_err(|_| VaultError::PathOutsideVault(path.to_path_buf()))
    }
}

/// Advisory lock file preventing multiple Truff instances from conflicting on the same vault.
pub struct VaultLock {
    // The same inode must survive unlock; unlinking would let a third owner bypass a waiter.
    _file: File,
}

impl VaultLock {
    pub fn acquire(vault_root: &Path) -> Result<Self, VaultError> {
        let truff_dir = vault_root.join(".truff");
        if fs::symlink_metadata(&truff_dir).is_ok_and(|m| m.file_type().is_symlink()) {
            return Err(VaultError::PathOutsideVault(truff_dir));
        }
        fs::create_dir_all(&truff_dir)?;
        let lock_path = truff_dir.join("vault.lock");
        if fs::symlink_metadata(&lock_path).is_ok_and(|m| m.file_type().is_symlink()) {
            return Err(VaultError::PathOutsideVault(lock_path));
        }
        let mut file = File::options()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(lock_path)?;
        if let Err(error) = file.try_lock() {
            if matches!(error, std::fs::TryLockError::WouldBlock) {
                let mut contents = String::new();
                Read::by_ref(&mut file)
                    .take(32)
                    .read_to_string(&mut contents)?;
                return Err(VaultError::LockBusy {
                    pid: contents.trim().parse().unwrap_or(0),
                });
            }
            return Err(VaultError::Io(io::Error::other(error)));
        }
        file.set_len(0)?;
        file.rewind()?;
        writeln!(file, "{}", std::process::id())?;
        file.sync_all()?;
        Ok(Self { _file: file })
    }
}
