use anyhow::{anyhow, Context, Result};
use reqwest::Client;
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::process::Command;

pub const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const GITHUB_REPO: &str = "soltros/supernova-tui";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallKind {
    Nix,
    Deb,
    Rpm,
    Aur,
    Binary,
}

impl InstallKind {
    pub fn label(&self) -> &'static str {
        match self {
            InstallKind::Nix => "Nix store",
            InstallKind::Deb => "Debian/Ubuntu (.deb)",
            InstallKind::Rpm => "RPM (.rpm)",
            InstallKind::Aur => "Arch/AUR (pacman)",
            InstallKind::Binary => "Standalone binary",
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ReleaseAsset {
    pub name: String,
    pub browser_download_url: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GitHubRelease {
    pub tag_name: String,
    #[serde(default)]
    pub assets: Vec<ReleaseAsset>,
    #[serde(default)]
    pub html_url: String,
}

pub fn detect_install_kind(exe_path: &Path) -> InstallKind {
    let exe_str = exe_path.to_string_lossy();
    if exe_str.starts_with("/nix/store") {
        return InstallKind::Nix;
    }
    if command_ok("dpkg", &["-S", &exe_str]) || command_ok("dpkg", &["-s", "supernova-tui"]) {
        return InstallKind::Deb;
    }
    if command_ok("rpm", &["-qf", &exe_str]) || command_ok("rpm", &["-q", "supernova-tui"]) {
        return InstallKind::Rpm;
    }
    if command_ok("pacman", &["-Qo", &exe_str]) || command_ok("pacman", &["-Qi", "supernova-tui"]) {
        return InstallKind::Aur;
    }
    InstallKind::Binary
}

fn command_ok(bin: &str, args: &[&str]) -> bool {
    Command::new(bin)
        .args(args)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

pub fn parse_semver(v: &str) -> Option<(u64, u64, u64)> {
    let clean = v.trim().trim_start_matches('v');
    let mut parts = clean.split('.').map(|p| {
        p.split('-')
            .next()
            .unwrap_or("0")
            .parse::<u64>()
            .unwrap_or(0)
    });
    Some((parts.next()?, parts.next().unwrap_or(0), parts.next().unwrap_or(0)))
}

pub fn is_newer(latest: &str, current: &str) -> bool {
    match (parse_semver(latest), parse_semver(current)) {
        (Some(l), Some(c)) => l > c,
        _ => latest.trim_start_matches('v') != current.trim_start_matches('v'),
    }
}

fn arch_aliases(arch: &str) -> &'static [&'static str] {
    match arch {
        "x86_64" => &["x86_64", "amd64"],
        "aarch64" => &["aarch64", "arm64"],
        _ => &[],
    }
}

pub fn select_asset<'a>(
    assets: &'a [ReleaseAsset],
    os: &str,
    arch: &str,
    kind: InstallKind,
) -> Option<&'a ReleaseAsset> {
    let aliases = arch_aliases(arch);
    let matches_arch = |name: &str| {
        let lower = name.to_lowercase();
        aliases.iter().any(|a| lower.contains(a))
    };

    match kind {
        InstallKind::Deb => assets
            .iter()
            .find(|a| a.name.ends_with(".deb") && matches_arch(&a.name)),
        InstallKind::Rpm => assets
            .iter()
            .find(|a| a.name.ends_with(".rpm") && matches_arch(&a.name)),
        InstallKind::Binary | InstallKind::Nix | InstallKind::Aur => assets
            .iter()
            .find(|a| {
                let lower = a.name.to_lowercase();
                lower.ends_with(".tar.gz") && lower.contains(os) && matches_arch(&lower)
            })
            .or_else(|| {
                assets.iter().find(|a| {
                    let lower = a.name.to_lowercase();
                    !lower.ends_with(".deb")
                        && !lower.ends_with(".rpm")
                        && !lower.ends_with(".sha256")
                        && lower.contains(os)
                        && matches_arch(&lower)
                })
            }),
    }
}

pub async fn run_self_update(check_only: bool) -> Result<()> {
    let exe_path = std::env::current_exe().context("resolve current executable path")?;
    let kind = detect_install_kind(&exe_path);
    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;

    println!(
        "supernova-tui v{CURRENT_VERSION} ({os}/{arch}, install: {})",
        kind.label()
    );

    let http = Client::builder()
        .user_agent(format!("supernova-tui/{CURRENT_VERSION}"))
        .build()?;

    let api_url = format!("https://api.github.com/repos/{GITHUB_REPO}/releases/latest");
    let resp = http.get(&api_url).send().await?;
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(anyhow!("GitHub release check failed ({status}): {body}"));
    }
    let release: GitHubRelease = resp.json().await?;
    let latest_ver = release.tag_name.trim_start_matches('v');

    if !is_newer(latest_ver, CURRENT_VERSION) {
        println!("Already up to date (latest: v{latest_ver}).");
        return Ok(());
    }

    println!("New version available: v{latest_ver} ({})", release.html_url);

    match kind {
        InstallKind::Nix => {
            println!("Installed via Nix store. Run: nix profile upgrade supernova-tui");
            return Ok(());
        }
        InstallKind::Aur => {
            println!("Installed via pacman/AUR. Update with your AUR helper (e.g. yay -Syu supernova-tui).");
            return Ok(());
        }
        _ => {}
    }

    let asset = select_asset(&release.assets, os, arch, kind).ok_or_else(|| {
        anyhow!(
            "No matching release asset found for {os}/{arch} ({})",
            kind.label()
        )
    })?;

    println!("Selected asset: {}", asset.name);
    if check_only {
        println!("Download URL: {}", asset.browser_download_url);
        return Ok(());
    }

    println!("Downloading {}...", asset.browser_download_url);
    let bytes = http
        .get(&asset.browser_download_url)
        .send()
        .await?
        .error_for_status()?
        .bytes()
        .await?;

    match kind {
        InstallKind::Deb => {
            let tmp = std::env::temp_dir().join(&asset.name);
            std::fs::write(&tmp, &bytes)?;
            println!("Installing {} via sudo dpkg -i...", tmp.display());
            let status = Command::new("sudo").args(["dpkg", "-i"]).arg(&tmp).status()?;
            let _ = std::fs::remove_file(&tmp);
            if !status.success() {
                return Err(anyhow!("dpkg -i failed with status {status}"));
            }
        }
        InstallKind::Rpm => {
            let tmp = std::env::temp_dir().join(&asset.name);
            std::fs::write(&tmp, &bytes)?;
            println!("Installing {} via sudo rpm -Uvh...", tmp.display());
            let status = Command::new("sudo").args(["rpm", "-Uvh"]).arg(&tmp).status()?;
            let _ = std::fs::remove_file(&tmp);
            if !status.success() {
                return Err(anyhow!("rpm -Uvh failed with status {status}"));
            }
        }
        InstallKind::Binary => {
            replace_binary(&exe_path, &asset.name, &bytes)?;
        }
        InstallKind::Nix | InstallKind::Aur => unreachable!(),
    }

    println!("Successfully updated supernova-tui to v{latest_ver}!");
    Ok(())
}

fn replace_binary(exe_path: &Path, asset_name: &str, bytes: &[u8]) -> Result<()> {
    let tmp_dir = std::env::temp_dir().join(format!("supernova-tui-upd-{}", std::process::id()));
    std::fs::create_dir_all(&tmp_dir)?;

    let new_bin: PathBuf = if asset_name.ends_with(".tar.gz") {
        let tgz = tmp_dir.join(asset_name);
        std::fs::write(&tgz, bytes)?;
        let status = Command::new("tar")
            .args(["-xzf"])
            .arg(&tgz)
            .arg("-C")
            .arg(&tmp_dir)
            .status()?;
        if !status.success() {
            return Err(anyhow!("Failed to extract {}", asset_name));
        }
        tmp_dir.join("supernova-tui")
    } else {
        let p = tmp_dir.join("supernova-tui");
        std::fs::write(&p, bytes)?;
        p
    };

    if !new_bin.exists() {
        return Err(anyhow!("Extracted archive did not contain 'supernova-tui' binary"));
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&new_bin, std::fs::Permissions::from_mode(0o755))?;
    }

    let parent = exe_path.parent().unwrap_or_else(|| Path::new("/tmp"));
    let staged = parent.join(format!(".supernova-tui.new.{}", std::process::id()));
    if std::fs::copy(&new_bin, &staged).is_ok() {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&staged, std::fs::Permissions::from_mode(0o755));
        }
        std::fs::rename(&staged, exe_path).context("replace running binary")?;
    } else {
        println!("Permission denied writing to {}; trying sudo mv...", exe_path.display());
        let status = Command::new("sudo")
            .arg("mv")
            .arg(&new_bin)
            .arg(exe_path)
            .status()?;
        if !status.success() {
            return Err(anyhow!("Failed to replace {}", exe_path.display()));
        }
    }

    let _ = std::fs::remove_dir_all(&tmp_dir);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mock_assets() -> Vec<ReleaseAsset> {
        vec![
            ReleaseAsset {
                name: "supernova-tui-linux-x86_64.tar.gz".into(),
                browser_download_url: "https://example.com/x86_64.tar.gz".into(),
            },
            ReleaseAsset {
                name: "supernova-tui-linux-aarch64.tar.gz".into(),
                browser_download_url: "https://example.com/aarch64.tar.gz".into(),
            },
            ReleaseAsset {
                name: "supernova-tui_0.2.0_amd64.deb".into(),
                browser_download_url: "https://example.com/amd64.deb".into(),
            },
            ReleaseAsset {
                name: "supernova-tui-0.2.0-1.x86_64.rpm".into(),
                browser_download_url: "https://example.com/x86_64.rpm".into(),
            },
        ]
    }

    #[test]
    fn test_version_compare() {
        assert!(is_newer("0.2.0", "0.1.0"));
        assert!(is_newer("v1.0.0", "0.9.9"));
        assert!(!is_newer("0.1.0", "0.1.0"));
        assert!(!is_newer("0.1.0", "0.2.0"));
    }

    #[test]
    fn test_asset_selection() {
        let assets = mock_assets();
        assert_eq!(
            select_asset(&assets, "linux", "x86_64", InstallKind::Deb).unwrap().name,
            "supernova-tui_0.2.0_amd64.deb"
        );
        assert_eq!(
            select_asset(&assets, "linux", "x86_64", InstallKind::Rpm).unwrap().name,
            "supernova-tui-0.2.0-1.x86_64.rpm"
        );
        assert_eq!(
            select_asset(&assets, "linux", "aarch64", InstallKind::Binary).unwrap().name,
            "supernova-tui-linux-aarch64.tar.gz"
        );
    }
}
