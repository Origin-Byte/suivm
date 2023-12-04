use anyhow::{anyhow, Result};
use indicatif::ProgressBar;
use indicatif::ProgressStyle;
use serde::Deserialize;
use std::fs;
use std::fs::File;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process;

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
pub fn use_version(alias: &String, compile: bool) -> Result<()> {
    let version = fetch_version(alias)?;

    // Make sure the requested version is installed
    let installed_versions = fetch_installed_versions();
    if !installed_versions.contains(&version) {
        install_version(alias, compile)?;
    }

    let mut current_version_file = File::create(path_version().as_path())?;
    current_version_file.write_all(version.as_bytes())?;

    println!("Using Sui `{}`", current_version().unwrap());
    Ok(())
}

pub fn install_version(alias: &String, _compile: bool) -> Result<()> {
    let version = fetch_version(alias)?;

    println!("Installing Sui `{alias} ({version})`");

    if !_compile {
        let available_versions = fetch_versions()?;
        if available_versions.contains(alias) {
            download_version(&version)?;
            println!("Downloaded Sui `{alias} ({version})`");
            return Ok(());
        }
    }

    compile_version(&version)?;
    println!("Compiled Sui `{alias} ({version})`");

    Ok(())
}

fn download_version(version: &String) -> Result<()> {
    use std::io::Cursor;

    use flate2::read::GzDecoder;
    use tar::Archive;

    let mut temp_path = directory_bin();
    temp_path.push(format!(".{version}"));

    let mut tar_gz_buffer: Vec<u8> = vec![];

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    let os_postfix = "macos-arm64";
    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    let os_postfix = "macos-x86_64";
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    let os_postfix = "ubuntu-x86_64";
    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    let os_postfix = "windows-x86_64";

    let res = ureq::get(&format!(
        "https://github.com/MystenLabs/sui/releases/download/{version}/sui-{version}-{os_postfix}.tgz",
    ))
    .call()?;
    let len: u64 = res.header("Content-Length").unwrap().parse()?;
    let mut rdr = res.into_reader();

    let pb = ProgressBar::new(len);
    pb.set_style(ProgressStyle::default_bar()
        .template("{msg}\n{spinner:.green} [{elapsed_precise}] [{wide_bar:.white/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})")
        .unwrap()
        .progress_chars("█  "));
    pb.set_message(format!("Downloading Sui `{version}`"));

    let mut buf = [0; 8192];
    while let Ok(len) = rdr.read(&mut buf) {
        if len == 0 {
            break;
        }

        pb.set_position(pb.position() + len as u64);
        tar_gz_buffer.write_all(&buf[..len])?;
    }

    pb.finish_and_clear();

    let cursor = Cursor::new(tar_gz_buffer);
    let mut archive = Archive::new(GzDecoder::new(cursor));

    #[cfg(not(windows))]
    let target_path = format!("./target/release/sui-{os_postfix}");
    #[cfg(windows)]
    let target_path = format!(".\\target\\release\\sui-{os_postfix}");

    let unpacked = archive
        .entries()?
        .filter_map(|entry| entry.ok())
        .find(|entry| entry.path_bytes() == target_path.as_bytes())
        .ok_or(anyhow!(
            "{target_path:?} was not present in downloaded archive"
        ))?
        .unpack(path_bin(version))?;

    // Set execution permission for the file
    let file = match unpacked {
        tar::Unpacked::File(file) => file,
        _ => {
            return Err(anyhow!(
            "Unpacked file was a directory, hardlink, symlink, or other node"
        ))
        }
    };

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let mut perms = file.metadata().unwrap().permissions();
        perms.set_mode(perms.mode() | 0b001000000);
        file.set_permissions(perms)?;
    }

    Ok(())
}

fn compile_version(version: &String) -> Result<()> {
    let directory = directory_suivm();
    let exit = std::process::Command::new("cargo")
        .args([
            "install",
            "--locked",
            "--force",
            "--git",
            "https://github.com/MystenLabs/sui.git",
            "--rev",
            version,
            "sui",
            "--root",
            &directory.to_string_lossy(),
        ])
        .stdout(process::Stdio::inherit())
        .stderr(process::Stdio::inherit())
        .output()
        .map_err(|err| anyhow!("Install for Sui `{version}` failed: {err}"))?;

    if !exit.status.success() {
        return Err(anyhow!("Failed to compile Sui `{version}`"));
    }

    fs::rename(path_bin("sui"), path_bin(version))?;

    Ok(())
}

/// Uninstall Sui version
pub fn uninstall_version(alias: &String) -> Result<()> {
    let version = fetch_version(alias)?;

    let path = &path_bin(&version);
    if path.as_path().exists() {
        fs::remove_file(path)?;
    }

    let current_version = &current_version();
    if matches!(current_version, Some(current) if current == &version) {
        let path = &path_version();
        if path.as_path().exists() {
            fs::remove_file(path_version())?;
        }
    }

    println!("Uninstalled Sui `{alias} ({version})`");
    Ok(())
}

/// Resolves aliases to their commit hash
fn fetch_version(alias: &String) -> Result<String> {
    match fetch_versions() {
        Ok(available_versions) => {
            if available_versions.contains(alias) {
                return Ok(alias.clone());
            }
        }
        Err(err) => {
            eprintln!("Could not fetch available versions, falling back to commit version check: {err}");
        }
    };

    // Will treat branch names and commit hashes as valid commits
    if let Ok(version) = fetch_latest_commit(alias) {
        return Ok(version);
    }

    Err(anyhow!("`{alias}` is neither a valid version, branch, or commit, check available versions using `suivm list`"))
}

/// Retrieve a list of installable versions of sui using the GitHub API and tags
/// on the Sui repository.
pub fn fetch_versions() -> Result<Vec<String>> {
    #[derive(Deserialize, Debug)]
    struct Release {
        tag_name: String,
    }

    let file =
        ureq::get("https://api.github.com/repos/MystenLabs/sui/releases")
            .call()?
            .into_reader();

    let versions: Vec<Release> = serde_json::from_reader(file)?;
    Ok(versions.into_iter().map(|r| r.tag_name).rev().collect())
}

pub fn fetch_latest_version() -> Result<String> {
    let available_versions = fetch_versions()?;
    available_versions
        .last()
        .cloned()
        .ok_or_else(|| anyhow::Error::msg("No versions found"))
}

pub fn fetch_latest_commit(branch: &str) -> Result<String> {
    #[derive(Deserialize, Debug)]
    struct Commit {
        sha: String,
    }

    let file = ureq::get(&format!(
        "https://api.github.com/repos/MystenLabs/sui/commits/{branch}"
    ))
    .call()?
    .into_reader();

    let commit: Commit = serde_json::from_reader(file)?;
    Ok(commit.sha)
}

/// Read the installed sui-cli versions by reading files in bin directory
pub fn fetch_installed_versions() -> Vec<String> {
    let home_dir = directory_bin();
    fs::read_dir(home_dir)
        .unwrap()
        .filter_map(Result::ok)
        .filter_map(|item| {
            item.file_type()
                .unwrap()
                .is_file()
                .then_some(item.file_name())
        })
        .filter_map(|version| version.into_string().ok())
        .filter(|name| !name.starts_with('.'))
        .collect()
}
