// tests/install_success.rs

use assert_cmd::Command;
use predicates::prelude::*;
use std::fs::{self, File};
use std::io::Write;

use tempfile::TempDir;

#[test]
fn test_successful_installation() {
    // 1. Setup isolated environment
    let temp_home = TempDir::new().expect("failed to create temp home");
    let source_dir = TempDir::new().expect("failed to create source dir");

    // 2. Create dummy content
    let file_path = source_dir.path().join("hello.txt");
    let mut file = File::create(&file_path).expect("failed to create file");
    writeln!(file, "Hello, tarsmith!").expect("failed to write file");

    // Create a dummy executable so the installer finds something
    let exe_path = source_dir.path().join("run.sh");
    fs::write(&exe_path, "#!/bin/sh\necho running").expect("failed to write exe");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&exe_path).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&exe_path, perms).expect("set exec perms");
    }

    // 3. Create tar archive (outside source dir)
    let tar_dir = TempDir::new().expect("failed to create tar dir");
    let archive_path = tar_dir.path().join("test_archive.tar");
    let status = std::process::Command::new("tar")
        .args(&[
            "-cf",
            archive_path.to_str().unwrap(),
            "-C",
            source_dir.path().to_str().unwrap(),
            ".",
        ])
        .status()
        .expect("failed to create tar archive");
    assert!(status.success(), "tar command failed");

    // 4. Run installer
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_tarsmith"));
    cmd.env("HOME", temp_home.path())
        .arg(&archive_path)
        .arg("--no-desktop")
        .arg("--no-path")
        .arg("--user"); // force user-level install to avoid sudo

    // 5. Assertions
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Installation complete!"))
        .stdout(predicate::str::contains("Skipped desktop entry creation"))
        .stdout(predicate::str::contains("Skipped adding to PATH"));
}
