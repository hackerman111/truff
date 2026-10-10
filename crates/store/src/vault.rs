use std::fmt;
use std::fs::{self, File};
use std::io::{self, Write};
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
                    if !normalized.pop() {
                        return Err(VaultError::PathOutsideVault(rel.to_path_buf()));
                    }
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
        Ok(self.root.join(normalized))
    }

    /// Strips the vault root prefix from an absolute path.
    pub fn relative_path<'a>(&self, path: &'a Path) -> Result<&'a Path, VaultError> {
        path.strip_prefix(&self.root)
            .map_err(|_| VaultError::PathOutsideVault(path.to_path_buf()))
    }
}

/// Advisory lock file preventing multiple Truff instances from conflicting on the same vault.
pub struct VaultLock {
    lock_file: PathBuf,
}

impl VaultLock {
    /// Acquires an advisory lock for the vault at `.truff/vault.lock`.
    pub fn acquire(vault_root: &Path) -> Result<Self, VaultError> {
        let truff_dir = vault_root.join(".truff");
        fs::create_dir_all(&truff_dir)?;
        let lock_file = truff_dir.join("vault.lock");

        if lock_file.exists()
            && let Ok(contents) = fs::read_to_string(&lock_file)
            && let Ok(pid) = contents.trim().parse::<u32>()
        {
            // Check if process with PID is alive on Unix
            #[cfg(unix)]
            {
                // kill(pid, 0) checks if process is alive
                let is_alive = unsafe { libc_kill_check(pid) };
                if is_alive {
                    return Err(VaultError::LockBusy { pid });
                }
            }
            #[cfg(not(unix))]
            {
                return Err(VaultError::LockBusy { pid });
            }
        }

        let pid = std::process::id();
        let mut f = File::create(&lock_file)?;
        writeln!(f, "{pid}")?;
        f.sync_all()?;

        Ok(Self { lock_file })
    }
}

impl Drop for VaultLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.lock_file);
    }
}

#[cfg(unix)]
unsafe fn libc_kill_check(pid: u32) -> bool {
    // libc::kill(pid as i32, 0)
    // In safe std Rust without libc crate, /proc/<pid> check is available on Linux:
    Path::new(&format!("/proc/{pid}")).exists()
}
