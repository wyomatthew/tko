//! Smoke tests for the CLI skeleton.

use assert_cmd::Command;
use predicates::prelude::*;

fn tko() -> Command {
    Command::cargo_bin("tko").unwrap()
}

#[test]
fn help_lists_core_commands() {
    tko()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("create"))
        .stdout(predicate::str::contains("show"))
        .stdout(predicate::str::contains("dep"));
}

#[test]
fn version_flag_reports_version() {
    tko()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn list_alias_ls_parses() {
    tko()
        .arg("ls")
        .assert()
        .failure()
        .stderr(predicate::str::contains("not yet implemented"));
}

#[test]
fn unknown_command_errors() {
    tko().arg("frobnicate").assert().failure();
}

#[test]
fn link_requires_two_ids() {
    tko().args(["link", "1"]).assert().failure();
}
