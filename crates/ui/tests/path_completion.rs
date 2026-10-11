use std::fs;
use truff_ui::app::path_completion::{complete_directory_path, expand_path, tab_complete};

#[test]
fn test_expand_path_tilde() {
    let expanded = expand_path("~/some/path");
    let home = std::env::var_os("HOME")
        .map(std::path::PathBuf::from)
        .unwrap();
    assert_eq!(expanded, home.join("some/path"));
}

#[test]
fn test_expand_path_dot_slash() {
    let expanded = expand_path("./some/sub");
    let cwd = std::env::current_dir().unwrap();
    assert_eq!(expanded, cwd.join("some/sub"));
}

#[test]
fn test_expand_path_dot_dot_slash() {
    let expanded = expand_path("../sibling");
    let cwd = std::env::current_dir().unwrap();
    let expected = cwd.parent().unwrap_or(&cwd).join("sibling");
    assert_eq!(expanded, expected);
}

#[test]
fn test_complete_directories_in_temp() {
    let tmp = std::env::temp_dir().join(format!("truff-test-complete-{}", std::process::id()));
    let _ = fs::remove_dir_all(&tmp);
    let alpha = tmp.join("alpha_dir");
    let beta = tmp.join("beta_dir");
    let file = tmp.join("file.txt");
    fs::create_dir_all(&alpha).unwrap();
    fs::create_dir_all(&beta).unwrap();
    fs::write(&file, "not a dir").unwrap();

    let prefix = format!("{}/a", tmp.display());
    let suggestions = complete_directory_path(&prefix);
    assert_eq!(suggestions.len(), 1);
    assert_eq!(suggestions[0], format!("{}/alpha_dir/", tmp.display()));

    // Tab complete should expand prefix to full alpha_dir/
    let completed = tab_complete(&prefix);
    assert_eq!(completed, Some(format!("{}/alpha_dir/", tmp.display())));

    let _ = fs::remove_dir_all(&tmp);
}
