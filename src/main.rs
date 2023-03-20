use anyhow::Result;
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
    #[clap(about = "Uninstalls Sui version")]
    Uninstall { version: String },
    #[clap(about = "Installs Sui version")]
    Install {
        /// Force Sui to compile instead of downloading when possible
        #[arg(short, long)]
        compile: bool,
        /// Sui release tag, branch, or revision
        version: String,
    },
    #[clap(about = "Use Sui version and install it if missing")]
    Use {
        /// Force Sui to compile instead of downloading when possible
        #[arg(short, long)]
        compile: bool,
        /// Sui release tag, branch, or revision
        version: String,
    },
}

fn print_version(
    installed_versions: &Vec<String>,
    latest: &Option<String>,
    current: &Option<String>,
    version: &String,
) {
    let mut flags = vec![];
    if matches!(latest, Some(latest) if latest == version) {
        flags.push("latest");
    }
    if installed_versions.contains(version) {
        flags.push("installed");
    }
    if matches!(current, Some(current) if current == version) {
        flags.push("current");
    }

    if flags.is_empty() {
        println!("{version}");
    } else {
        println!("{version} ({})", flags.join(", "));
    }
}

fn print_versions() {
    let available_versions = match fetch_versions() {
        Ok(versions) => versions,
        Err(err) => return println!("Could not fetch versions: {err}"),
    };

    let current = suivm::current_version();
    let installed_versions = &suivm::fetch_installed_versions();
    let latest = available_versions.last().cloned();

    for version in available_versions {
        print_version(&installed_versions, &latest, &current, &version);
    }
}

fn print_latest_version() {
    let latest = match suivm::fetch_latest_version() {
        Ok(latest) => latest,
        Err(err) => return println!("Could not fetch latest version: {err}"),
    };

    let current = suivm::current_version();
    let installed_versions = &suivm::fetch_installed_versions();

    print_version(&installed_versions, &None, &current, &latest);
}

fn print_installed() {
    let latest = suivm::fetch_latest_version().ok();
    let current = suivm::current_version();

    for version in suivm::fetch_installed_versions() {
        print_version(&Vec::new(), &latest, &current, &version);
    }
}

fn print_current() {
    let latest = suivm::fetch_latest_version().ok();
    match suivm::current_version() {
        Some(current) => print_version(&Vec::new(), &latest, &None, &current),
        None => println!("Sui is not installed. Run `suivm use latest`"),
    }
}

fn handle_alias(alias: &str) -> Result<String> {
    if alias == "latest" {
        suivm::fetch_latest_version()
    } else {
        Ok(alias.to_string())
    }
}

fn main() -> Result<()> {
    match Suivm::parse() {
        Suivm::Latest => Ok(print_latest_version()),
        Suivm::List => Ok(print_versions()),
        Suivm::Installed => Ok(print_installed()),
        Suivm::Status => Ok(print_current()),
        Suivm::Uninstall { version } => suivm::uninstall_version(&version),
        Suivm::Install { compile, version } => {
            let version = handle_alias(&version)?;
            suivm::install_version(&version, compile)
        }
        Suivm::Use { compile, version } => {
            let version = handle_alias(&version)?;
            suivm::use_version(&version, compile)
        }
    }
}
