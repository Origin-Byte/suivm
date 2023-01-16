use anyhow::Result;
use indicatif::ProgressBar;
use indicatif::ProgressStyle;
use serde::Deserialize;
use std::fs;
use std::fs::File;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process;

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
pub fn use_version(version: &String, compile: bool) -> Result<()> {
    // Make sure the requested version is installed
    let installed_versions = fetch_installed_versions();
    if !installed_versions.contains(version) {
        install_version(version, compile)?;
    }

    let mut current_version_file = File::create(path_version().as_path())?;
    current_version_file.write_all(version.as_bytes())?;

    println!("Using Sui `{}`", current_version().unwrap());
    Ok(())
}

pub fn install_version(version: &String, compile: bool) -> Result<()> {
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    {
        if compile {
            compile_version(version)?;
        } else {
            download_version(version)?;
        }
    }
    #[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
    compile_version(version)?;

    Ok(())
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
pub fn download_version(version: &String) -> Result<()> {
    use std::os::unix::prelude::PermissionsExt;

    let mut file = File::create(path_bin(version))?;

    let res = ureq::get(&format!(
        "https://github.com/MystenLabs/sui/releases/download/{version}/sui",
    ))
    .call()?;
    let len: u64 = res.header("Content-Length").unwrap().parse()?;
    let mut rdr = res.into_reader();

    let pb = ProgressBar::new(len);
    pb.set_style(ProgressStyle::default_bar()
        .template("{msg}\n{spinner:.green} [{elapsed_precise}] [{wide_bar:.white/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})")
        .unwrap()
        .progress_chars("█  "));
    pb.set_message(format!("Downloading {version}"));

    let mut buf = [0; 8192];
    while let Ok(len) = rdr.read(&mut buf) {
        if len == 0 {
            break;
        }

        pb.set_position(pb.position() + len as u64);
        file.write(&buf[..len]).unwrap();
    }

    pb.finish_and_clear();

    println!("Downloaded `{version}`");

    // Set execution permission for the file
    let mut perms = file.metadata().unwrap().permissions();
    perms.set_mode(perms.mode() | 0b001000000);
    file.set_permissions(perms)?;

    Ok(())
}

pub fn compile_version(version: &String) -> Result<()> {
    let directory = directory_suivm();
    let exit = std::process::Command::new("cargo")
        .args([
            "install",
            "--locked",
            "--git",
            "https://github.com/MystenLabs/sui.git",
            "--rev",
            version,
            "sui",
            "--bin",
            "sui",
            "--root",
            directory.as_os_str().to_str().unwrap(),
        ])
        .stdout(process::Stdio::inherit())
        .stderr(process::Stdio::inherit())
        .output()
        .map_err(|err| {
            anyhow::Error::msg(format!(
                "Cargo install for {version} failed: {err}"
            ))
        })?;

    if !exit.status.success() {
        return Err(anyhow::Error::msg("Failed to compile Sui"));
    }

    // fs::rename(&directory.join("sui"), &directory.join(version))?;

    println!("Compiled `{version}`");

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
    let file =
        ureq::get("https://api.github.com/repos/MystenLabs/sui/releases")
            .call()?
            .into_reader();

    let versions: Vec<Release> = serde_json::from_reader(file)?;
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
