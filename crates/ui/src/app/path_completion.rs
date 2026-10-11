//! Path expansion and directory autocompletion helper supporting `~`, `./`, and `../`.

use std::fs;
use std::path::{Path, PathBuf};

/// Expands `~`, `./`, and `../` into an absolute PathBuf.
pub fn expand_path(input: &str) -> PathBuf {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    }

    let raw = if trimmed == "~" || trimmed.starts_with("~/") {
        if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
            if trimmed == "~" {
                home
            } else {
                home.join(&trimmed[2..])
            }
        } else {
            PathBuf::from(trimmed)
        }
    } else {
        let path = Path::new(trimmed);
        if path.is_absolute() {
            path.to_path_buf()
        } else if let Ok(cwd) = std::env::current_dir() {
            cwd.join(path)
        } else {
            path.to_path_buf()
        }
    };

    normalize_path(&raw)
}

/// Normalizes path by resolving `.` and `..` components.
pub fn normalize_path(path: &Path) -> PathBuf {
    use std::path::Component;
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            Component::Normal(c) => normalized.push(c),
            Component::RootDir => normalized.push(Component::RootDir),
            Component::Prefix(p) => normalized.push(Component::Prefix(p)),
        }
    }
    normalized
}

/// Returns matching subdirectories for the given path input prefix.
/// Preserves the user's notation style (`~`, `./`, `../`, or absolute).
pub fn complete_directory_path(input: &str) -> Vec<String> {
    let trimmed = input.trim();
    let (prefix_base, filter_name) = if trimmed.ends_with('/') {
        (trimmed, "")
    } else if let Some(last_slash) = trimmed.rfind('/') {
        (&trimmed[..=last_slash], &trimmed[last_slash + 1..])
    } else {
        ("", trimmed)
    };

    let search_dir = expand_path(if prefix_base.is_empty() { "." } else { prefix_base });

    let Ok(entries) = fs::read_dir(&search_dir) else {
        return Vec::new();
    };

    let mut suggestions = Vec::new();
    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        let is_dir = file_type.is_dir()
            || (file_type.is_symlink()
                && entry.path().canonicalize().is_ok_and(|p| p.is_dir()));
        if !is_dir {
            continue;
        }

        let name = entry.file_name();
        let name_str = name.to_string_lossy();

        // Skip hidden files unless filter starts with '.'
        if name_str.starts_with('.') && !filter_name.starts_with('.') {
            continue;
        }

        if name_str.starts_with(filter_name) {
            let formatted = format!("{prefix_base}{name_str}/");
            suggestions.push(formatted);
        }
    }

    suggestions.sort();
    suggestions.truncate(10);
    suggestions
}

/// Computes the longest common prefix or completes with the single candidate on Tab.
pub fn tab_complete(input: &str) -> Option<String> {
    let candidates = complete_directory_path(input);
    if candidates.is_empty() {
        return None;
    }
    if candidates.len() == 1 {
        return Some(candidates[0].clone());
    }

    // Find longest common prefix
    let first = &candidates[0];
    let mut common_len = first.len();
    for other in &candidates[1..] {
        common_len = first
            .chars()
            .zip(other.chars())
            .take_while(|(a, b)| a == b)
            .count()
            .min(common_len);
    }

    if common_len > input.len() {
        Some(first[..common_len].to_string())
    } else {
        Some(candidates[0].clone())
    }
}
