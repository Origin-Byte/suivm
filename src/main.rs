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
        #[clap(value_parser(parse_version))]
        version: Version,
    },
    #[clap(about = "Switch to specified version of suivm and install it if missing")]
    Switch {
        #[clap(value_parser(parse_version))]
        version: Version,
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
fn parse_version(version: &str) -> Result<Version, Error> {
    if version == "latest" {
        Ok(suivm::get_latest_version())
    } else {
        Version::parse(version).map_err(|e| anyhow::anyhow!(e))
    }
}

fn main() -> Result<()> {
    match Suivm::parse() {
        Suivm::List => suivm::list_versions(),
        Suivm::Uninstall { version } => suivm::uninstall_version(&version),
        Suivm::Switch { version, force } => suivm::switch_version(&version, force),
        Suivm::Latest => suivm::list_versions(),
        Suivm::ListLocal => suivm::list_versions(),
    }
}
