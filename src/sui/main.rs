use std::{env, fs, process};

fn main() -> anyhow::Result<()> {
    let args = env::args().skip(1).collect::<Vec<String>>();

    let version = suivm::current_version().ok_or_else(|| {
        anyhow::Error::msg("Sui is not installed. Run `suivm use latest`")
    })?;

    let binary_path = suivm::path_bin(&version);
    fs::metadata(&binary_path).map_err(|_| {
        anyhow::Error::msg(
            "Sui installation corrupted. Run `suivm install {version}`",
        )
    })?;

    process::Command::new(binary_path)
        .args(args)
        .spawn()?
        .wait_with_output()
        .expect("Failed to run Sui");

    Ok(())
}
