#![forbid(unsafe_code)]

use std::path::PathBuf;

use symphony_cli::build_startup_config_from_args;
use symphony_testkit::test_cwd;

#[test]
fn cli_uses_default_runtime_config_path_relative_to_cwd() {
    let cwd = test_cwd("/srv/symphony");
    let startup =
        build_startup_config_from_args(["symphony"], &cwd).expect("default args should parse");

    assert_eq!(startup.config_path, cwd.join("symphony.runtime.json"));
}

#[test]
fn cli_resolves_relative_config_path_against_runtime_cwd() {
    let cwd = test_cwd("/srv/symphony");
    let startup =
        build_startup_config_from_args(["symphony", "--config", "config/runtime.json"], &cwd)
            .expect("cli args should parse");

    assert_eq!(startup.config_path, cwd.join("config/runtime.json"));
}

#[test]
fn cli_keeps_absolute_config_path_unchanged() {
    let cwd = test_cwd("/srv/symphony");
    let absolute = PathBuf::from("/etc/symphony/runtime.json");
    let startup = build_startup_config_from_args(
        ["symphony", "--config", "/etc/symphony/runtime.json"],
        &cwd,
    )
    .expect("cli args should parse");

    assert_eq!(startup.config_path, absolute);
}
