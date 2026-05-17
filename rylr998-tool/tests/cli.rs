use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn help_runs() {
    Command::cargo_bin("rylr998")
        .unwrap()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("rylr998"));
}

#[test]
fn unknown_subcommand_fails() {
    Command::cargo_bin("rylr998")
        .unwrap()
        .arg("nope")
        .assert()
        .failure();
}

#[test]
fn missing_port_with_no_devices() {
    // Pass a clearly-invalid path; the tool should fail with non-zero exit
    // and a recognizable error message rather than panicking.
    Command::cargo_bin("rylr998")
        .unwrap()
        .args(["--port", "/dev/definitely-not-a-real-tty", "info"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("error:"));
}

#[test]
fn provision_help_documents_cpin_without_cpin_persist() {
    Command::cargo_bin("rylr998")
        .unwrap()
        .args(["provision", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--cpin"))
        .stdout(predicate::str::contains("--cpin-persist").not());
}
