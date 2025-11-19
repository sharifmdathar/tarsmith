// tests/cli.rs

use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn test_help_output() {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_tarsmith"));
    cmd.arg("--help");
    cmd.assert().success().stdout(predicate::str::contains(
        "A simple, interactive installer for tar archives",
    ));
}

#[test]
fn test_version_output() {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_tarsmith"));
    cmd.arg("--version");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains(env!("CARGO_PKG_VERSION")));
}
