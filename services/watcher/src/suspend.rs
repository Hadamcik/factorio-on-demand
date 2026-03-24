use tokio::process::Command;

use crate::logging::log;

pub async fn suspend_machine() -> Result<(), String> {
    log("Suspending machine");

    let output = Command::new("/usr/bin/sudo")
        .arg("-n")
        .arg("/usr/bin/systemctl")
        .arg("suspend")
        .output()
        .await
        .map_err(|e| format!("failed to execute suspend: {e}"))?;

    if !output.status.success() {
        return Err(format!(
            "sudo systemctl suspend failed: status={} stdout={} stderr={}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        ));
    }

    Ok(())
}
