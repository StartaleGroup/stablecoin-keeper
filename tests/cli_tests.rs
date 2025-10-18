use std::process::Command;
use anyhow::Result;

#[test]
fn test_cli_help() -> Result<()> {
    // Test that CLI help works
    let output = Command::new("cargo")
        .args(&["run", "--", "--help"])
        .output()?;
    
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout)?;
    assert!(stdout.contains("stablecoin-backend"));
    assert!(stdout.contains("Automated USDSC yield distribution keeper"));
    
    println!("✅ CLI help test passed");
    Ok(())
}

#[test]
fn test_cli_claim_yield_help() -> Result<()> {
    // Test that claim-yield subcommand help works
    let output = Command::new("cargo")
        .args(&["run", "--", "claim-yield", "--help"])
        .output()?;
    
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout)?;
    assert!(stdout.contains("chain-id"));
    assert!(stdout.contains("config"));
    assert!(stdout.contains("private-key"));
    assert!(stdout.contains("dry-run"));
    
    println!("✅ CLI claim-yield help test passed");
    Ok(())
}

#[test]
fn test_cli_distribute_rewards_help() -> Result<()> {
    // Test that distribute-rewards subcommand help works
    let output = Command::new("cargo")
        .args(&["run", "--", "distribute-rewards", "--help"])
        .output()?;
    
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout)?;
    assert!(stdout.contains("chain-id"));
    assert!(stdout.contains("config"));
    assert!(stdout.contains("private-key"));
    assert!(stdout.contains("dry-run"));
    
    println!("✅ CLI distribute-rewards help test passed");
    Ok(())
}

#[test]
fn test_cli_missing_required_args() -> Result<()> {
    // Test that missing required arguments cause appropriate errors
    let output = Command::new("cargo")
        .args(&["run", "--", "claim-yield"])
        .output()?;
    
    // Should fail due to missing required arguments
    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr)?;
    assert!(stderr.contains("required"));
    
    println!("✅ CLI missing required args test passed");
    Ok(())
}

#[test]
fn test_cli_invalid_chain_id() -> Result<()> {
    // Test that invalid chain ID causes appropriate error
    let output = Command::new("cargo")
        .args(&["run", "--", "claim-yield", "--chain-id=invalid", "--config=test.toml", "--dry-run"])
        .output()?;
    
    // Should fail due to invalid chain ID
    assert!(!output.status.success());
    
    println!("✅ CLI invalid chain ID test passed");
    Ok(())
}

#[test]
fn test_cli_dry_run_flag() -> Result<()> {
    // Test that dry-run flag is properly recognized
    let output = Command::new("cargo")
        .args(&["run", "--", "claim-yield", "--chain-id=1", "--config=test.toml", "--dry-run"])
        .output()?;
    
    // Should fail due to missing config file, but dry-run flag should be recognized
    let stderr = String::from_utf8(output.stderr)?;
    // The error should be about missing config file, not about unknown dry-run flag
    assert!(!stderr.contains("unknown"));
    
    println!("✅ CLI dry-run flag test passed");
    Ok(())
}

#[test]
fn test_cli_private_key_override() -> Result<()> {
    // Test that private-key override is properly recognized
    let output = Command::new("cargo")
        .args(&["run", "--", "claim-yield", "--chain-id=1", "--config=test.toml", "--private-key=0x123", "--dry-run"])
        .output()?;
    
    // Should fail due to missing config file, but private-key flag should be recognized
    let stderr = String::from_utf8(output.stderr)?;
    // The error should be about missing config file, not about unknown private-key flag
    assert!(!stderr.contains("unknown"));
    
    println!("✅ CLI private-key override test passed");
    Ok(())
}
