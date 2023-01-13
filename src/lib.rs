use anyhow::Result;
use indicatif::ProgressBar;
use indicatif::ProgressStyle;
use serde::Deserialize;
use std::fs;
use std::fs::File;
use std::io::{Read, Write};
use std::path::PathBuf;

#[derive(Deserialize, Debug)]
struct Release {
    tag_name: String,
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
        install_version(version)?;
    }

    let mut current_version_file = File::create(path_version().as_path())?;
    current_version_file.write_all(version.as_bytes())?;

    println!("Using Sui `{}`", current_version().unwrap());
    Ok(())
}

pub fn install_version(version: &String) -> Result<()> {
    // TODO: Compile for non linux
    download_version(version)
}

pub fn download_version(version: &String) -> Result<()> {
    use std::os::unix::prelude::PermissionsExt;

    let mut file = File::create(path_bin(version))?;

    let pb = ProgressBar::new(1);
    pb.set_style(ProgressStyle::default_bar()
        .template("{msg}\n{spinner:.green} [{elapsed_precise}] [{wide_bar:.white/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})")
        .unwrap()
        .progress_chars("█  "));
    pb.set_message(format!("Downloading {version}"));

    let mut easy = curl::easy::Easy::new();
    easy.url(&format!(
        "https://github.com/MystenLabs/sui/releases/download/{version}/sui",
    ))?;
    easy.follow_location(true)?;
    easy.progress(true)?;

    {
        let mut transfer = easy.transfer();
        transfer.progress_function(|dtotal, dlnow, _, _| {
            pb.set_length(dtotal as u64);
            pb.set_position(dlnow as u64);
            true
        })?;
        transfer.write_function(|buf| Ok(file.write(buf).unwrap()))?;
        transfer.perform()?;
    }

    pb.finish_and_clear();

    println!("Downloaded `{version}`");

    // Set execution permission for the file
    let mut perms = file.metadata().unwrap().permissions();
    perms.set_mode(perms.mode() | 0b001000000);
    file.set_permissions(perms)?;

    Ok(())
}

/// Uninstall Sui version
pub fn uninstall_version(version: &String) -> Result<()> {
    let path = &path_bin(version);
    if path.as_path().exists() {
        fs::remove_file(path)?;
    }

    let current_version = &current_version();
    if matches!(current_version, Some(current) if current == version) {
        let path = &path_version();
        if path.as_path().exists() {
            fs::remove_file(path_version())?;
        }
    }

    println!("Uninstalled Sui `{version}`");
    Ok(())
}

/// Retrieve a list of installable versions of sui using the GitHub API and tags
/// on the Sui repository.
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

    let versions: Vec<Release> = serde_json::from_slice(&dst)?;
    Ok(versions.into_iter().map(|r| r.tag_name).rev().collect())
}

pub fn fetch_latest_version() -> Result<String> {
    let available_versions = fetch_versions()?;
    Ok(available_versions.last().unwrap().clone())
}

/// Read the installed sui-cli versions by reading files in bin directory
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
