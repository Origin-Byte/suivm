use std::{env, fs, process::Command};

fn main() -> anyhow::Result<()> {
    let args = env::args().skip(1).collect::<Vec<String>>();

    let version = suivm::current_version()
        .map_err(|_e| anyhow::anyhow!("suivm version not set. Please run `suivm use latest`."))?;

    let binary_path = avm::version_binary_path(&version);
    if fs::metadata(&binary_path).is_err() {
        anyhow::bail!(
            "sui-cli {} not installed. Please run `suivm use {}`.",
            version,
            version
        );
    }
    Command::new(binary_path)
        .args(args)
        .spawn()?
        .wait_with_output()
        .expect("Failed to run sui-cli");

    Ok(())
}
