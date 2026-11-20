use reqwest::{Client, StatusCode, Url};
use semver::{BuildMetadata, Version};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;
use std::time::{Duration, Instant};
use tauri::async_runtime::Mutex;
use tauri::ipc::Channel;
use tauri::{AppHandle, State};

const RELEASE_API: &str = "https://api.github.com/repos/Leo-Lee-j/JustToDo/releases/latest";
const DOWNLOAD_PREFIX: &str = "/Leo-Lee-j/JustToDo/releases/download/";
const MAX_INSTALLER_SIZE: u64 = 250 * 1024 * 1024;

#[derive(Default)]
pub struct UpdateState(Mutex<Option<PendingUpdate>>);

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    version: String,
    notes: String,
    size: u64,
}

struct PendingUpdate {
    info: UpdateInfo,
    url: Url,
    sha256: String,
}

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    body: Option<String>,
    draft: bool,
    prerelease: bool,
    assets: Vec<Asset>,
}

#[derive(Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
    size: u64,
    digest: Option<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProgress {
    phase: &'static str,
    downloaded: u64,
    total: u64,
}

fn client() -> Result<Client, String> {
    Client::builder()
        .user_agent(concat!("JustToDo/", env!("CARGO_PKG_VERSION")))
        .https_only(true)
        .connect_timeout(Duration::from_secs(15))
        .read_timeout(Duration::from_secs(30))
        .timeout(Duration::from_secs(600))
        .build()
        .map_err(|err| format!("无法初始化更新服务：{err}"))
}

fn newer_stable_version(tag: &str, current: &str) -> Result<Option<Version>, String> {
    let mut latest = Version::parse(tag.strip_prefix('v').unwrap_or(tag))
        .map_err(|_| "发布版本号无效，请前往 GitHub Releases 查看".to_string())?;
    let mut current = Version::parse(current).map_err(|err| err.to_string())?;
    latest.build = BuildMetadata::EMPTY;
    current.build = BuildMetadata::EMPTY;
    Ok((latest.pre.is_empty() && latest > current).then_some(latest))
}

fn download_url(value: &str) -> Result<Url, String> {
    let url = Url::parse(value).map_err(|_| "安装包下载地址无效".to_string())?;
    if url.scheme() != "https"
        || url.host_str() != Some("github.com")
        || !url.path().starts_with(DOWNLOAD_PREFIX)
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err("安装包必须来自 JustToDo 官方 GitHub Releases".into());
    }
    Ok(url)
}

fn normalized_hash(hash: &str) -> Result<String, String> {
    if hash.len() != 64 || !hash.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("安装包 SHA-256 校验信息无效，请前往 GitHub Releases 下载".into());
    }
    Ok(hash.to_ascii_lowercase())
}

fn checksum_file(text: &str, filename: &str) -> Result<String, String> {
    let parts: Vec<_> = text.split_whitespace().collect();
    if parts.len() != 2 || parts[1].trim_start_matches('*') != filename {
        return Err("安装包校验文件与发布版本不匹配".into());
    }
    normalized_hash(parts[0])
}

fn request_error(err: reqwest::Error) -> String {
    if err.is_timeout() {
        "连接 GitHub 超时，请检查网络后重试".into()
    } else {
        format!("更新请求失败，请检查网络后重试：{err}")
    }
}

async fn expected_hash(client: &Client, asset: &Asset, assets: &[Asset]) -> Result<String, String> {
    // GitHub provides a digest for new assets. The published checksum file also
    // supports assets uploaded before that API field was introduced.
    if let Some(digest) = &asset.digest {
        return normalized_hash(digest.strip_prefix("sha256:").unwrap_or(""));
    }
    let filename = format!("{}.sha256", asset.name);
    let checksum = assets
        .iter()
        .find(|item| item.name == filename && item.size > 0 && item.size <= 1024)
        .ok_or("该版本缺少安装包校验信息，请前往 GitHub Releases 手动下载")?;
    let response = client
        .get(download_url(&checksum.browser_download_url)?)
        .timeout(Duration::from_secs(30))
        .send()
        .await
        .and_then(reqwest::Response::error_for_status)
        .map_err(request_error)?;
    let text = response.text().await.map_err(request_error)?;
    if text.len() > 1024 {
        return Err("安装包校验文件过大".into());
    }
    checksum_file(&text, &asset.name)
}

#[tauri::command]
pub async fn check_app_update(state: State<'_, UpdateState>) -> Result<Option<UpdateInfo>, String> {
    if !cfg!(all(target_os = "windows", target_arch = "x86_64")) {
        return Err("应用内更新仅支持 Windows x64".into());
    }
    let mut pending = state.0.try_lock().map_err(|_| "更新操作正在进行，请稍候")?;
    *pending = None;
    let client = client()?;
    let response = client
        .get(RELEASE_API)
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .timeout(Duration::from_secs(30))
        .send()
        .await
        .map_err(request_error)?;
    match response.status() {
        StatusCode::NOT_FOUND => {
            return Err("暂未找到公开发布版本，请前往 GitHub Releases 查看".into())
        }
        StatusCode::FORBIDDEN | StatusCode::TOO_MANY_REQUESTS => {
            return Err("GitHub 更新请求受限，请稍后重试或前往 Releases 下载".into());
        }
        _ => {}
    }
    let release: Release = response
        .error_for_status()
        .map_err(request_error)?
        .json()
        .await
        .map_err(request_error)?;
    if release.draft || release.prerelease {
        return Ok(None);
    }
    let Some(version) = newer_stable_version(&release.tag_name, crate::models::APP_VERSION)? else {
        return Ok(None);
    };
    let filename = format!("JustToDo_{version}_x64-setup.exe");
    let asset = release
        .assets
        .iter()
        .find(|asset| asset.name == filename)
        .ok_or("该版本尚未提供 Windows x64 安装包，请稍后重试")?;
    if asset.size == 0 || asset.size > MAX_INSTALLER_SIZE {
        return Err("安装包大小异常，请前往 GitHub Releases 查看".into());
    }
    let update = PendingUpdate {
        info: UpdateInfo {
            version: version.to_string(),
            notes: release.body.clone().unwrap_or_default(),
            size: asset.size,
        },
        url: download_url(&asset.browser_download_url)?,
        sha256: expected_hash(&client, asset, &release.assets).await?,
    };
    let info = update.info.clone();
    *pending = Some(update);
    Ok(Some(info))
}

fn verify_installer(path: &Path, size: u64, expected_hash: &str) -> Result<(), String> {
    let mut file = File::open(path).map_err(|err| format!("无法读取安装包：{err}"))?;
    if file.metadata().map_err(|err| err.to_string())?.len() != size {
        return Err("安装包下载不完整，请重试".into());
    }
    let mut header = [0; 2];
    file.read_exact(&mut header)
        .map_err(|err| err.to_string())?;
    if header != *b"MZ" {
        return Err("下载内容不是 Windows 安装程序，请重试".into());
    }
    let mut hash = Sha256::new();
    hash.update(header);
    let mut buffer = [0; 64 * 1024];
    loop {
        let count = file.read(&mut buffer).map_err(|err| err.to_string())?;
        if count == 0 {
            break;
        }
        hash.update(&buffer[..count]);
    }
    if format!("{:x}", hash.finalize()) != expected_hash {
        return Err("安装包 SHA-256 校验失败，已取消安装，请重新下载".into());
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn launch_installer(installer: &Path) -> Result<(), String> {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStrExt;
    use windows::core::{w, PCWSTR};
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

    let executable = std::env::current_exe().map_err(|err| err.to_string())?;
    let install_dir = executable.parent().ok_or("无法确定应用安装目录")?;
    // Tauri NSIS: passive update, restart on success, preserve a custom install
    // directory. NSIS requires /D to be the final, unquoted parameter.
    let mut parameters = OsString::from("/P /UPDATE /R /D=");
    parameters.push(install_dir);
    let parameters: Vec<u16> = parameters.encode_wide().chain(Some(0)).collect();
    let installer: Vec<u16> = installer.as_os_str().encode_wide().chain(Some(0)).collect();
    let result = unsafe {
        ShellExecuteW(
            None,
            w!("open"),
            PCWSTR(installer.as_ptr()),
            PCWSTR(parameters.as_ptr()),
            None,
            SW_SHOWNORMAL,
        )
    };
    let code = result.0 as isize;
    if code <= 32 {
        return Err(format!(
            "无法启动安装程序（Windows 错误 {code}），应用尚未退出"
        ));
    }
    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn launch_installer(_: &Path) -> Result<(), String> {
    Err("应用内更新仅支持 Windows".into())
}

#[tauri::command]
pub async fn install_app_update(
    app: AppHandle,
    state: State<'_, UpdateState>,
    on_progress: Channel<UpdateProgress>,
) -> Result<(), String> {
    if cfg!(debug_assertions) {
        return Err("开发模式仅支持检查更新，请在已安装的发布版本中安装更新".into());
    }
    let pending = state.0.try_lock().map_err(|_| "更新操作正在进行，请稍候")?;
    let update = pending.as_ref().ok_or("请先检查更新")?;
    let temporary = tempfile::Builder::new()
        .prefix("JustToDo-update-")
        .tempdir()
        .map_err(|err| format!("无法创建更新目录：{err}"))?;
    let path = temporary.path().join("installer.exe");
    let mut file = File::create(&path).map_err(|err| format!("无法保存安装包：{err}"))?;
    let progress = |phase, downloaded| {
        let _ = on_progress.send(UpdateProgress {
            phase,
            downloaded,
            total: update.info.size,
        });
    };
    progress("downloading", 0);
    let mut response = client()?
        .get(update.url.clone())
        .send()
        .await
        .and_then(reqwest::Response::error_for_status)
        .map_err(request_error)?;
    let mut downloaded = 0;
    let mut last_progress = Instant::now();
    while let Some(chunk) = response.chunk().await.map_err(request_error)? {
        downloaded += chunk.len() as u64;
        if downloaded > update.info.size {
            return Err("安装包大小与发布信息不匹配，已取消下载".into());
        }
        file.write_all(&chunk)
            .map_err(|err| format!("保存安装包失败：{err}"))?;
        if last_progress.elapsed() >= Duration::from_millis(100) || downloaded == update.info.size {
            progress("downloading", downloaded);
            last_progress = Instant::now();
        }
    }
    file.sync_all()
        .map_err(|err| format!("保存安装包失败：{err}"))?;
    drop(file);
    progress("verifying", downloaded);
    verify_installer(&path, update.info.size, &update.sha256)?;
    progress("installing", downloaded);
    launch_installer(&path)?;
    // The installer outlives this process and needs its files until completion.
    let _ = temporary.keep();
    app.exit(0);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compares_stable_versions_by_semver() {
        assert_eq!(
            newer_stable_version("v0.0.10", "0.0.9")
                .unwrap()
                .unwrap()
                .to_string(),
            "0.0.10"
        );
        for tag in ["v0.0.8", "0.0.7", "v0.0.9-beta.1", "v0.0.8+build.2"] {
            assert!(newer_stable_version(tag, "0.0.8").unwrap().is_none());
        }
        assert!(newer_stable_version("not-a-version", "0.0.8").is_err());
    }

    #[test]
    fn only_accepts_this_repositorys_https_release_assets() {
        assert!(download_url(
            "https://github.com/Leo-Lee-j/JustToDo/releases/download/v0.0.9/setup.exe"
        )
        .is_ok());
        for url in [
            "http://github.com/Leo-Lee-j/JustToDo/releases/download/v1/setup.exe",
            "https://github.com/another/repo/releases/download/v1/setup.exe",
            "https://github.com.evil.test/Leo-Lee-j/JustToDo/releases/download/v1/setup.exe",
            "https://github.com/Leo-Lee-j/JustToDo/releases/download/../../../../evil/setup.exe",
        ] {
            assert!(download_url(url).is_err(), "{url}");
        }
    }

    #[test]
    fn checksum_must_match_the_exact_installer_filename() {
        let hash = "AB".repeat(32);
        assert_eq!(
            checksum_file(&format!("{hash}  setup.exe\r\n"), "setup.exe").unwrap(),
            hash.to_ascii_lowercase()
        );
        assert!(checksum_file(&format!("{hash}  other.exe"), "setup.exe").is_err());
        assert!(checksum_file("not-a-hash setup.exe", "setup.exe").is_err());
        assert!(checksum_file(&format!("{hash} setup.exe\n{hash} evil.exe"), "setup.exe").is_err());
    }

    #[test]
    fn rejects_truncated_corrupt_or_non_executable_downloads() {
        let bytes = b"MZ JustToDo test installer";
        let mut file = tempfile::NamedTempFile::new().unwrap();
        file.write_all(bytes).unwrap();
        let hash = format!("{:x}", Sha256::digest(bytes));
        assert!(verify_installer(file.path(), bytes.len() as u64, &hash).is_ok());
        assert!(verify_installer(file.path(), bytes.len() as u64 + 1, &hash).is_err());
        assert!(verify_installer(file.path(), bytes.len() as u64, &"0".repeat(64)).is_err());
        std::fs::write(file.path(), b"<html>not an exe</html>").unwrap();
        let size = std::fs::metadata(file.path()).unwrap().len();
        assert!(verify_installer(file.path(), size, &hash).is_err());
    }
}
