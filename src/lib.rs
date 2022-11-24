use anyhow::Context;
use anyhow::{anyhow, Result};
use futures_util::StreamExt;
use indicatif::{ProgressBar, ProgressStyle};
use once_cell::sync::Lazy;
use reqwest::header::USER_AGENT;
use semver::Version;
use serde::{de, Deserialize};
use std::cmp;
use std::fs;
use std::fs::File;
use std::io::{Seek, Write};
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

#[derive(Deserialize, Debug)]
struct GithubRelease {
    #[serde(rename = "tag_name", deserialize_with = "version_deserializer")]
    version: semver::Version,
    assets_url: String,
}

fn version_deserializer<'de, D>(
    deserializer: D,
) -> Result<semver::Version, D::Error>
where
    D: de::Deserializer<'de>,
{
    let s: &str = de::Deserialize::deserialize(deserializer)?;
    // println!("{} / {} / {}", s.trim_start_matches("refs/tags/private-testnet-"), s.trim_start_matches("refs/tags/devnet-").trim_start_matches("_v"), s.trim_start_matches("refs/tags/private-testnet-").trim_start_matches("refs/tags/devnet-").trim_start_matches("refs/tags/sui_v").trim_end_matches("_ci").replace("2022-08-15","0.8.15"));
    let val = Version::parse(s.trim_start_matches("devnet-"))
        .unwrap_or(semver::Version::new(0, 0, 0));
    Ok(val)
}

/// Storage directory for SUIVM, ~/.suivm
pub static SUIVM_HOME: Lazy<PathBuf> = Lazy::new(|| {
    cfg_if::cfg_if! {
        if #[cfg(test)] {
            let dir = tempfile::tempdir().expect("Could not create temporary directory");
            dir.path().join(".suivm")
        } else {
            let mut user_home = dirs::home_dir().expect("Could not find home directory");
            user_home.push(".suivm");
            user_home
        }
    }
});

/// Path to the current version file ~/.suivm/.version
pub fn current_version_file_path() -> PathBuf {
    let mut current_version_file_path = SUIVM_HOME.to_path_buf();
    current_version_file_path.push(".version");
    current_version_file_path
}

/// Read the current version from the version file
pub fn current_version() -> Result<Version> {
    let v = fs::read_to_string(current_version_file_path().as_path())
        .map_err(|e| anyhow!("Could not read version file: {}", e))?;
    Version::parse(v.trim_end_matches('\n').to_string().as_str())
        .map_err(|e| anyhow!("Could not parse version file: {}", e))
}

/// Path to the binary for the given version
pub fn version_binary_path(version: &Version) -> PathBuf {
    let mut version_path = SUIVM_HOME.join("bin");
    version_path.push(format!("anchor-{}", version));
    version_path
}

/// Update the current version to a new version
pub fn use_version(version: &Version) -> Result<()> {
    let installed_versions = read_installed_versions();
    // Make sure the requested version is installed
    if !installed_versions.contains(version) {
        if let Ok(current) = current_version() {
            println!(
                "Version {} is not installed, staying on version {}.",
                version, current
            );
        } else {
            println!(
                "Version {} is not installed, no current version.",
                version
            );
        }

        return Err(anyhow!(
            "You need to run 'suivm install {}' to install it before using it.",
            version
        ));
    }

    let mut current_version_file =
        fs::File::create(current_version_file_path().as_path())?;
    current_version_file.write_all(version.to_string().as_bytes())?;
    println!("Now using sui version {}.", current_version()?);
    let path = current_version()?;
    let setSui = format!("export PATH=$PATH:{path}");
    Command::new(setSui);
    Ok(())
}

/// Update to the latest version
pub async fn update() -> Result<()> {
    // Find last stable version
    let version = &get_latest_version().await?;

    switch_version(version, false).await
}

pub async fn download_file(
    client: &reqwest::Client,
    url: &str,
    path: &str,
) -> Result<()> {
    let res = client
        .get(url)
        .send()
        .await
        .with_context(|| format!("Failed to GET from '{}'", &url))?;
    let total_size = res.content_length().with_context(|| {
        format!("Failed to get content length from '{}'", &url)
    })?;

    let pb = ProgressBar::new(total_size);
    pb.set_style(ProgressStyle::default_bar()
        .template("{msg}\n{spinner:.green} [{elapsed_precise}] [{wide_bar:.white/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})")
        .unwrap()
        .progress_chars("█  "));
    pb.set_message(format!("Downloading {}", url));

    let mut file;
    let mut downloaded: u64 = 0;
    let mut stream = res.bytes_stream();

    println!("Seeking in file.");
    if std::path::Path::new(path).exists() {
        println!("File exists. Resuming.");
        file = std::fs::OpenOptions::new()
            .read(true)
            .append(true)
            .open(path)
            .unwrap();

        let file_size = std::fs::metadata(path).unwrap().len();
        file.seek(std::io::SeekFrom::Start(file_size)).unwrap();
        downloaded = file_size;
    } else {
        println!("Fresh file..");
        file = File::create(path)
            .with_context(|| format!("Failed to create file '{}'", path))?;
    }

    println!("Commencing transfer");
    while let Some(item) = stream.next().await {
        let chunk =
            item.with_context(|| format!("Error while downloading file"))?;
        // BUG: the chuck might not be written all at once
        file.write(&chunk)?;
        let new = cmp::min(downloaded + (chunk.len() as u64), total_size);
        downloaded = new;
        pb.set_position(new);
    }

    pb.finish_with_message(format!("Downloaded {} to {}", url, path));

    Ok(())
}

/// Install a version of sui
pub async fn switch_version(version: &Version, force: bool) -> Result<()> {
    // If version is already installed we ignore the request.
    let installed_versions = read_installed_versions();
    if !(installed_versions.contains(version) && !force) {
        let client = reqwest::Client::new();
        download_file(
            &client,
            &format!(
                "https://github.com/MystenLabs/sui/releases/download/devnet-{}/sui",
                &version
            ),
            SUIVM_HOME
                .join("bin")
                .join(format!("sui-{}", version))
                .as_os_str()
                .to_str()
                .unwrap(),
        )
            .await
            .unwrap();
    }
    println!("Version {} is already installed", version);
    //return Ok(());

    // if !exit.status.success() {
    //     return Err(anyhow!(
    //         "Failed to install {}, is it a valid version?",
    //         version
    //     ));
    // }
    fs::rename(
        &SUIVM_HOME.join("bin").join("sui"),
        &SUIVM_HOME.join("bin").join(format!("sui-{}", version)),
    )?;
    // If .version file is empty or not parseable, write the newly installed version to it
    if current_version().is_err() {
        let mut current_version_file =
            fs::File::create(current_version_file_path().as_path())?;
        current_version_file.write_all(version.to_string().as_bytes())?;
    }

    use_version(version)
}

/// Remove an installed version of sui
pub fn uninstall_version(version: &Version) -> Result<()> {
    let version_path = SUIVM_HOME.join("bin").join(format!("sui-{}", version));
    if !version_path.exists() {
        return Err(anyhow!("sui {} is not installed", version));
    }
    if version == &current_version().unwrap() {
        return Err(anyhow!("sui {} is currently in use", version));
    }
    fs::remove_file(version_path.as_path())?;
    Ok(())
}

/// Ensure the users home directory is setup with the paths required by AVM.
pub fn ensure_paths() {
    let home_dir = SUIVM_HOME.to_path_buf();
    if !home_dir.as_path().exists() {
        fs::create_dir_all(home_dir.clone())
            .expect("Could not create .suivm directory");
    }
    let bin_dir = home_dir.join("bin");
    if !bin_dir.as_path().exists() {
        fs::create_dir_all(bin_dir)
            .expect("Could not create .suivm/bin directory");
    }
    if !current_version_file_path().exists() {
        fs::File::create(current_version_file_path())
            .expect("Could not create .version file");
    }
}

/// Retrieve a list of installable versions of sui using the GitHub API and tags on the Sui
/// repository.
pub async fn fetch_versions() -> Result<Vec<semver::Version>> {
    let client = reqwest::Client::new();
    let versions: Vec<GithubRelease> = client
        .get("https://api.github.com/repos/MystenLabs/sui/releases")
        .header(USER_AGENT, "suivm https://github.com/MystenLabs/sui")
        .send()
        .await?
        .json()
        .await?;
    Ok(versions
        .into_iter()
        .filter(|r| {
            r.version.to_string() != semver::Version::new(0, 0, 0).to_string()
        })
        .rev()
        .map(|r| r.version)
        .collect())
}

/// Print available versions and flags indicating installed, current and latest
pub async fn list_versions() -> Result<()> {
    let installed_versions = read_installed_versions();

    let available_versions = fetch_versions().await?;

    available_versions.iter().enumerate().for_each(|(i, v)| {
        print!("{}", v);
        let mut flags = vec![];
        if i == available_versions.len() - 1 {
            flags.push("latest");
        }
        if installed_versions.contains(v) {
            flags.push("installed");
        }
        if current_version().is_ok() && current_version().unwrap() == v.clone()
        {
            flags.push("current");
        }
        if flags.is_empty() {
            println!();
        } else {
            println!("\t({})", flags.join(", "));
        }
    });

    Ok(())
}

pub async fn get_latest_version() -> Result<semver::Version> {
    let available_versions = fetch_versions().await?;
    Ok(available_versions.first().unwrap().clone())
}

/// Read the installed anchor-cli versions by reading the binaries in the SUIVM_HOME/bin directory.
pub fn read_installed_versions() -> Vec<semver::Version> {
    let home_dir = SUIVM_HOME.to_path_buf();
    println!("{}", home_dir.display());
    let mut versions = vec![];
    let home_exists: bool = Path::new(&home_dir).is_dir();
    if !home_exists {
        return versions;
    }
    for file in fs::read_dir(&home_dir.join("bin")).unwrap() {
        let file_name = file.unwrap().file_name();
        // Match only things that look like sui-*
        if file_name.to_str().unwrap().starts_with("sui-") {
            let version = file_name
                .to_str()
                .unwrap()
                .trim_start_matches("sui-")
                .parse::<semver::Version>()
                .unwrap();
            versions.push(version);
        }
    }

    versions
}

#[cfg(test)]
mod tests {
    use crate::*;
    use semver::Version;
    use std::fs;
    use std::io::Write;

    #[test]
    fn test_ensure_paths() {
        ensure_paths();
        assert!(SUIVM_HOME.exists());
        let bin_dir = SUIVM_HOME.join("bin");
        assert!(bin_dir.exists());
        let current_version_file = SUIVM_HOME.join(".version");
        assert!(current_version_file.exists());
    }

    #[test]
    fn test_current_version_file_path() {
        ensure_paths();
        assert!(current_version_file_path().exists());
    }

    #[test]
    fn test_version_binary_path() {
        assert!(
            version_binary_path(&Version::parse("0.14.0").unwrap())
                == SUIVM_HOME.join("bin/sui-0.14.0")
        );
    }

    #[test]
    fn test_current_version() {
        ensure_paths();
        let mut current_version_file =
            fs::File::create(current_version_file_path().as_path()).unwrap();
        current_version_file.write_all("0.14.0".as_bytes()).unwrap();
        // Sync the file to disk before the read in current_version() to
        // mitigate the read not seeing the written version bytes.
        current_version_file.sync_all().unwrap();
        assert!(
            current_version().unwrap() == Version::parse("0.14.0").unwrap()
        );
    }

    #[test]
    #[should_panic(expected = "sui 0.14.0 is not installed")]
    fn test_uninstall_non_installed_version() {
        uninstall_version(&Version::parse("0.14.0").unwrap()).unwrap();
    }

    #[test]
    #[should_panic(expected = "sui 0.14.0 is currently in use")]
    fn test_uninstalled_in_use_version() {
        ensure_paths();
        let version = Version::parse("0.14.0").unwrap();
        let mut current_version_file =
            fs::File::create(current_version_file_path().as_path()).unwrap();
        current_version_file.write_all("0.14.0".as_bytes()).unwrap();
        // Sync the file to disk before the read in current_version() to
        // mitigate the read not seeing the written version bytes.
        current_version_file.sync_all().unwrap();
        // Create a fake binary for sui-0.14.0 in the bin directory
        fs::File::create(version_binary_path(&version)).unwrap();
        uninstall_version(&version).unwrap();
    }

    #[test]
    fn test_read_installed_versions() {
        ensure_paths();
        let version = Version::parse("0.14.0").unwrap();
        // Create a fake binary for sui-0.14.0 in the bin directory
        fs::File::create(version_binary_path(&version)).unwrap();
        let expected = vec![version];
        assert!(read_installed_versions() == expected);
        // Should ignore this file because its not sui- prefixed
        fs::File::create(SUIVM_HOME.join("bin").join("garbage").as_path())
            .unwrap();
        assert!(read_installed_versions() == expected);
    }
}
