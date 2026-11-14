use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn version_prints_name_and_version() {
    Command::cargo_bin("termagent")
        .unwrap()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("termagent 2.0.0"));
}

#[test]
fn help_documents_web_onboard_and_port() {
    let out = Command::cargo_bin("termagent")
        .unwrap()
        .arg("--help")
        .assert()
        .success();
    let text = String::from_utf8_lossy(&out.get_output().stdout).to_string();
    for needle in [
        "--web",
        "--onboard",
        "--port",
        "6033",
        "--effort",
        "--list-models",
    ] {
        assert!(text.contains(needle), "help missing {needle}:\n{text}");
    }
}

#[test]
fn unknown_flag_is_a_usage_error() {
    Command::cargo_bin("termagent")
        .unwrap()
        .arg("--definitely-not-a-flag")
        .assert()
        .code(2);
}

#[test]
fn port_must_be_a_real_port() {
    Command::cargo_bin("termagent")
        .unwrap()
        .args(["--web", "--port", "80"])
        .assert()
        .code(2);
}

#[test]
fn one_shot_without_config_asks_for_onboarding() {
    let dir = tempfile::tempdir().unwrap();
    Command::cargo_bin("termagent")
        .unwrap()
        .env("TERMAGENT_CONFIG_DIR", dir.path())
        .env_remove("TERMAGENT_API_KEY")
        .env_remove("OPENAI_API_KEY")
        .env_remove("ANTHROPIC_API_KEY")
        .arg("say hi")
        .assert()
        .failure()
        .stderr(predicate::str::contains("onboard"));
}

#[test]
fn invalid_effort_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    Command::cargo_bin("termagent")
        .unwrap()
        .env("TERMAGENT_CONFIG_DIR", dir.path())
        .args(["--effort", "turbo", "--list-models"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("unknown effort"));
}
