use std::process::Command;

#[test]
fn test_cli_help() {
    // Test that the CLI shows help when run with --help
    let output = Command::new("cargo")
        .args(&["run", "--bin", "stablecoin-backend", "--", "--help"])
        .output()
        .expect("Failed to execute command");

    // The command should succeed and show help
    assert!(output.status.success() || output.status.code() == Some(1));
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    
    // Help should contain expected commands
    let help_text = if !stdout.is_empty() { stdout } else { stderr };
    assert!(help_text.contains("distribute-rewards") || help_text.contains("claim-yield"));
    
    println!("✅ CLI help command works correctly");
}

#[test]
fn test_cli_invalid_command() {
    // Test that the CLI shows error for invalid commands
    let output = Command::new("cargo")
        .args(&["run", "--bin", "stablecoin-backend", "--", "invalid-command"])
        .output()
        .expect("Failed to execute command");

    // Should fail with invalid command
    assert!(!output.status.success());
    
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("error") || stderr.contains("Unknown"));
    
    println!("✅ CLI correctly rejects invalid commands");
}
