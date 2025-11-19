// tests/path_symlink.rs

use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;

use tempfile::TempDir;

#[test]
fn test_path_symlink_created() {
    // 1. Setup isolated environment
    let temp_home = TempDir::new().expect("temp home");
    let source_dir = TempDir::new().expect("source dir");

    // 2. Create dummy executable
    let exe_path = source_dir.path().join("myapp");
    fs::write(&exe_path, "#!/bin/sh\necho ok").expect("write exe");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&exe_path).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&exe_path, perms).expect("set exec perms");
    }

    // 3. Create tar archive (outside source dir)
    let tar_dir = TempDir::new().expect("tar dir");
    let archive_path = tar_dir.path().join("myapp.tar");
    let status = std::process::Command::new("tar")
        .args(&[
            "-cf",
            archive_path.to_str().unwrap(),
            "-C",
            source_dir.path().to_str().unwrap(),
            ".",
        ])
        .status()
        .expect("tar");
    assert!(status.success(), "tar command failed");

    // 4. Run installer
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_tarsmith"));
    cmd.env("HOME", temp_home.path())
        .arg(&archive_path)
        .arg("--user");

    // 5. Assertions
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Created symlink"))
        .stdout(predicate::str::contains("You can now run these commands"));

    // Verify symlink exists in ~/.local/bin
    let bin_path = temp_home.path().join(".local/bin/myapp");
    assert!(bin_path.exists(), "symlink not created");
}
