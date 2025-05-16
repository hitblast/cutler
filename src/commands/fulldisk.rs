use anyhow::Result;
use tokio::process::Command;
use crate::util::logging::{LogLevel, print_log};

pub async fn run() -> Result<()> {
    let mut command = Command::new("sqlite3");
    command.args(&["/Library/Application Support/com.apple.TCC/TCC.db", "SELECT client FROM access WHERE auth_value = \"2\" and service = \"kTCCServiceSystemPolicyAllFiles\""]);

    if !command.status().await?.success() {
        print_log(LogLevel::Error, "Grant \"Full Disk Access\" to your terminal in order to run this command.");
    }

    Ok(())
}
