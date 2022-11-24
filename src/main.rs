mod cmd;
use anyhow::{Error, Result};
use clap::Parser;
use semver::Version;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Parser)]
#[command(name = "suivm")]
#[command(bin_name = "suivm")]
enum Suivm {
    Latest,
    List,
    #[clap(about = "Uninstall a version of suivm")]
    Uninstall {
        version: String,
    },
    #[clap(
        about = "Switch to specified version of suivm and install it if missing"
    )]
    Switch {
        version: String,
        #[clap(long)]
        /// Flag to force installation even if the version
        /// is already installed
        force: bool,
    },
    ListLocal,
}
//
// /// Remove from locally installed versions
// #[derive(clap::Args)]
// struct Remove {
//     #[clap(parse(try_from_str = parse_version))]
//     version: String,
// }

// /// Use given version, install if not yet
// #[derive(clap::Args)]
// struct Switch {
//     #[clap(parse(try_from_str = parse_version))]
//     version: String,
// }

// If `latest` is passed use the latest available version.
async fn parse_version(version: &str) -> Result<Version, Error> {
    if version == "latest" {
        suivm::get_latest_version().await
    } else {
        Version::parse(version).map_err(|e| anyhow::anyhow!(e))
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    suivm::ensure_paths();
    match Suivm::parse() {
        Suivm::List => suivm::list_versions().await,
        Suivm::Uninstall { version } => {
            suivm::uninstall_version(&parse_version(&version).await?)
        }
        Suivm::Switch { version, force } => {
            suivm::switch_version(&parse_version(&version).await?, force).await
        }
        Suivm::Latest => suivm::list_versions().await,
        Suivm::ListLocal => suivm::list_versions().await,
    }
}
