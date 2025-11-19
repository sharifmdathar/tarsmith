// tests/cli_error.rs

use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn test_missing_archive_error() {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_tarsmith"));
    cmd.arg("nonexistent_file.tar");
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("ArchiveNotFound"));
}

#[test]
fn test_no_arguments_shows_error() {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_tarsmith"));
    // No arguments supplied; clap should error out.
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("error"));
}
