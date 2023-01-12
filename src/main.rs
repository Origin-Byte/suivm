use anyhow::{anyhow, Error, Result};
use clap::Parser;
use suivm::fetch_versions;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Parser)]
#[command(name = "suivm")]
#[command(bin_name = "suivm")]
enum Suivm {
    #[clap(about = "List latest Sui version")]
    Latest,
    #[clap(about = "List all available Sui versions")]
    List,
    #[clap(about = "List all installed Sui versions")]
    Installed,
    #[clap(about = "Display current Sui version")]
    Status,
    #[clap(about = "Installs Sui version")]
    Install { version: String },
    #[clap(about = "Uninstalls Sui version")]
    Uninstall { version: String },
    #[clap(about = "Use Sui version and install it if missing")]
    Use { version: String },
}

// If `latest` is passed use the latest available version.
fn parse_version(version: String) -> Result<String, Error> {
    if version == "latest" {
        suivm::fetch_latest_version()
    } else {
        let available_versions = fetch_versions()?;
        if !available_versions.contains(&version) {
            return Err(anyhow!("`{version}` is not a valid version, check available versions using `suivm list`"));
        }
        Ok(version)
    }
}

fn main() -> Result<()> {
    match Suivm::parse() {
        Suivm::Latest => Ok(suivm::print_latest_version()),
        Suivm::List => Ok(suivm::print_versions()),
        Suivm::Installed => Ok(suivm::print_installed()),
        Suivm::Status => Ok(suivm::print_current()),
        Suivm::Install { version } => {
            suivm::install_version(&parse_version(version)?)
        }
        Suivm::Uninstall { version } => {
            suivm::uninstall_version(&parse_version(version)?)
        }
        Suivm::Use { version } => suivm::use_version(&parse_version(version)?),
    }
}
