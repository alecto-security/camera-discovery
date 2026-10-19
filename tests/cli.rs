use std::process::Command;

#[test]
fn help_lists_the_three_contract_flags() {
    let out = Command::new(env!("CARGO_BIN_EXE_camera-discovery"))
        .arg("--help")
        .output()
        .expect("run camera-discovery --help");
    assert!(out.status.success());
    let help = String::from_utf8_lossy(&out.stdout);
    assert!(help.contains("--network"));
    assert!(help.contains("--timeout"));
    assert!(help.contains("--json"));
}

#[test]
fn invalid_network_exits_one_with_a_config_message() {
    let out = Command::new(env!("CARGO_BIN_EXE_camera-discovery"))
        .args(["--network", "definitely-not-a-cidr", "--timeout", "0"])
        .output()
        .expect("run camera-discovery");
    assert_eq!(out.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("invalid network definitely-not-a-cidr"));
}

#[test]
fn version_is_reported() {
    let out = Command::new(env!("CARGO_BIN_EXE_camera-discovery"))
        .arg("--version")
        .output()
        .expect("run camera-discovery --version");
    assert!(out.status.success());
    assert!(String::from_utf8_lossy(&out.stdout).contains(env!("CARGO_PKG_VERSION")));
}
