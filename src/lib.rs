use anyhow::Result;
use indicatif::ProgressBar;
use indicatif::ProgressStyle;
use serde::Deserialize;
use std::fs;
use std::fs::File;
use std::io::{Read, Write};
use std::path::PathBuf;

#[derive(Deserialize, Debug)]
struct GithubRelease {
    #[serde(rename = "tag_name")]
    version: String,
}

fn directory_suivm() -> PathBuf {
    let mut home = dirs::home_dir().expect("Could not find home directory");
    home.push(".suivm");
    fs::create_dir_all(&home).unwrap();
    home
}

fn directory_bin() -> PathBuf {
    let mut bin = directory_suivm();
    bin.push("bin");
    fs::create_dir_all(&bin).unwrap();
    bin
}

fn path_version() -> PathBuf {
    let mut path = directory_suivm();
    path.push(".version");
    path
}

pub fn path_bin(version: &str) -> PathBuf {
    let mut path = directory_bin();
    path.push(version);
    path
}

/// Read the current version from the version file
pub fn current_version() -> Option<String> {
    File::open(path_version()).ok().and_then(|mut file| {
        let mut v = String::new();
        file.read_to_string(&mut v).unwrap();

        (!v.is_empty()).then_some(v)
    })
}

/// Install and use Sui version
pub fn use_version(version: &String) -> Result<()> {
    // Make sure the requested version is installed
    let installed_versions = fetch_installed_versions();
    if !installed_versions.contains(version) {
        install_version(version)?
    }

    let mut current_version_file = File::create(path_version().as_path())?;
    current_version_file.write_all(version.as_bytes())?;

    println!("Using Sui `{}`", current_version().unwrap());
    Ok(())
}

/// Install Sui version
pub fn install_version(version: &str) -> Result<()> {
    let url = &format!(
        "https://github.com/MystenLabs/sui/releases/download/{version}/sui",
    );

    let path = path_bin(version);
    let mut file = File::create(&path)?;

    let pb = ProgressBar::new(1);
    pb.set_style(ProgressStyle::default_bar()
        .template("{msg}\n{spinner:.green} [{elapsed_precise}] [{wide_bar:.white/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})")
        .unwrap()
        .progress_chars("█  "));
    pb.set_message(format!("Downloading {version}"));

    let mut easy = curl::easy::Easy::new();
    easy.url(&url)?;
    easy.follow_location(true)?;
    easy.progress(true)?;

    {
        let mut transfer = easy.transfer();
        transfer.progress_function(|dtotal, dlnow, _, _| {
            // hi
            pb.set_length(dtotal as u64);
            pb.set_position(dlnow as u64);
            true
        })?;
        transfer.write_function(|buf| Ok(file.write(buf).unwrap()))?;
        transfer.perform()?;
    }

    pb.finish_with_message(format!("Downloaded {version}"));

    Ok(())
}

/// Uninstall Sui version
pub fn uninstall_version(version: &str) -> Result<()> {
    let current_version = current_version();
    if matches!(current_version, Some(current) if current == version) {
        let _ = fs::remove_file(path_version());
    }

    // Silence failures
    let _ = fs::remove_file(path_bin(version));

    println!("Uninstalled Sui `{version}`");
    Ok(())
}

/// Retrieve a list of installable versions of sui using the GitHub API and tags on the Sui
/// repository.
pub fn fetch_versions() -> Result<Vec<String>> {
    let mut dst = Vec::new();

    let mut easy = curl::easy::Easy::new();
    easy.url("https://api.github.com/repos/MystenLabs/sui/releases")?;
    easy.useragent("suivm")?;

    {
        let mut transfer = easy.transfer();
        transfer.write_function(|buf| {
            dst.extend_from_slice(buf);
            Ok(buf.len())
        })?;
        transfer.perform()?;
    }

    let versions: Vec<GithubRelease> = serde_json::from_slice(&dst)?;
    Ok(versions.into_iter().map(|r| r.version).rev().collect())
}

pub fn fetch_latest_version() -> Result<String> {
    let available_versions = fetch_versions()?;
    Ok(available_versions.last().unwrap().clone())
}

/// Read the installed sui-cli versions by reading the binaries in the SUIVM_HOME/bin directory.
pub fn fetch_installed_versions() -> Vec<String> {
    let home_dir = directory_bin();
    fs::read_dir(&home_dir)
        .unwrap()
        .filter_map(Result::ok)
        .filter_map(|item| {
            item.file_type()
                .unwrap()
                .is_file()
                .then_some(item.file_name())
        })
        .filter_map(|version| version.into_string().ok())
        .filter(|name| !name.starts_with("."))
        .collect()
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

/// Print available versions and flags indicating installed, current and latest
pub fn print_versions() {
    let available_versions = match fetch_versions() {
        Ok(versions) => versions,
        Err(err) => return println!("Could not fetch versions: {err}"),
    };

    let current = current_version();
    let installed_versions = &fetch_installed_versions();
    let latest = available_versions.last().cloned();

    for version in available_versions {
        print_version(&installed_versions, &latest, &current, &version);
    }
}

pub fn print_latest_version() {
    let latest = match fetch_latest_version() {
        Ok(latest) => latest,
        Err(err) => return println!("Could not fetch latest version: {err}"),
    };

    let current = current_version();
    let installed_versions = &fetch_installed_versions();

    print_version(&installed_versions, &None, &current, &latest);
}

pub fn print_installed() {
    let latest = fetch_latest_version().ok();
    let current = current_version();

    for version in fetch_installed_versions() {
        print_version(&Vec::new(), &latest, &current, &version);
    }
}

pub fn print_current() {
    let latest = fetch_latest_version().ok();
    match current_version() {
        Some(current) => print_version(&Vec::new(), &latest, &None, &current),
        None => println!("Sui is not installed"),
    }
}
