//! Persistent, provider-neutral wallpaper sources and download locations.
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{LazyLock, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{Emitter, Manager};

static LIBRARY_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));
static SAVE_COUNTER: AtomicU64 = AtomicU64::new(0);
const PROVIDER: &str = "wallhaven";
const MAX_SCAN_DEPTH: usize = 48;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SourceKind {
    UserFolder,
    MatugenDownloads,
    DownloadArchive,
    KdeUser,
    KdeSystem,
    SystemBackgrounds,
    CurrentWallpaper,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WallpaperSource {
    pub id: String,
    pub kind: SourceKind,
    pub path: String,
    pub enabled: bool,
    pub recursive: bool,
    pub removable: bool,
    pub display_name: String,
}
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderRecord {
    pub provider: String,
    pub provider_id: String,
    pub cache_path: Option<String>,
    #[serde(default)]
    pub active_path: Option<String>,
    pub saved_path: Option<String>,
    pub thumb: Option<String>,
    pub last_used_at: Option<u64>,
    pub saved_at: Option<u64>,
    #[serde(default)]
    pub hidden_from_history: bool,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WallpaperLibrary {
    pub download_root: String,
    pub sources: Vec<WallpaperSource>,
    #[serde(default)]
    pub provider_records: BTreeMap<String, ProviderRecord>,
    #[serde(default)]
    pub legacy_folders_migrated: bool,
    #[serde(default)]
    pub legacy_history_migrated: bool,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WallpaperItem {
    pub path: String,
    pub file_name: String,
    pub source_id: String,
    pub source_kind: SourceKind,
    pub provider: Option<String>,
    pub provider_id: Option<String>,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceScanError {
    pub source_id: String,
    pub message: String,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanResult {
    pub items: Vec<WallpaperItem>,
    pub errors: Vec<SourceScanError>,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LegacyHistoryEntry {
    pub id: String,
    pub local_path: String,
    pub thumb: Option<String>,
    pub downloaded_at: Option<u64>,
}

fn data_path() -> Result<PathBuf, String> {
    Ok(dirs::config_dir()
        .ok_or("XDG config directory unavailable")?
        .join("matugen-studio/wallpaper-library.json"))
}
fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
fn path_id(kind: SourceKind, path: &Path) -> String {
    format!(
        "{:?}-{:x}",
        kind,
        md5::compute(path.to_string_lossy().as_bytes())
    )
}
fn normalized(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}
fn source(
    kind: SourceKind,
    path: PathBuf,
    name: &str,
    recursive: bool,
    removable: bool,
) -> WallpaperSource {
    let path = normalized(&path);
    WallpaperSource {
        id: path_id(kind, &path),
        kind,
        path: path.to_string_lossy().into_owned(),
        enabled: true,
        recursive,
        removable,
        display_name: name.into(),
    }
}
fn pictures_dir() -> PathBuf {
    dirs::picture_dir()
        .or_else(dirs::data_dir)
        .unwrap_or_else(|| {
            dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("/tmp"))
                .join(".local/share")
        })
}
fn download_root_for(pictures: &Path) -> PathBuf {
    pictures.join("Wallpapers/Matugen Studio")
}
fn picture_wallpapers_dir(pictures: Option<&Path>) -> Option<PathBuf> {
    pictures
        .map(|path| path.join("Wallpapers"))
        .filter(|path| path.is_dir())
}
fn add_detected_source(
    sources: &mut Vec<WallpaperSource>,
    paths: &mut HashSet<PathBuf>,
    pictures_root: Option<&Path>,
    kind: SourceKind,
    path: PathBuf,
    name: &str,
) {
    let normalized_path = normalized(&path);
    if pictures_root.is_some_and(|root| normalized(root) == normalized_path) {
        return;
    }
    if path.is_dir() && paths.insert(normalized_path) {
        sources.push(source(
            kind,
            path,
            name,
            true,
            kind == SourceKind::UserFolder,
        ));
    }
}
pub fn default_download_root() -> PathBuf {
    download_root_for(&pictures_dir())
}
fn default_library() -> WallpaperLibrary {
    let root = default_download_root();
    let pictures = dirs::picture_dir();
    let mut sources = vec![source(
        SourceKind::MatugenDownloads,
        root.clone(),
        "Matugen Studio Downloads",
        true,
        false,
    )];
    let detected = [
        (
            SourceKind::KdeUser,
            dirs::data_dir().map(|p| p.join("wallpapers")),
            "KDE user wallpapers",
        ),
        (
            SourceKind::KdeSystem,
            Some(PathBuf::from("/usr/share/wallpapers")),
            "KDE system wallpapers",
        ),
        (
            SourceKind::SystemBackgrounds,
            Some(PathBuf::from("/usr/share/backgrounds")),
            "System backgrounds",
        ),
        (
            SourceKind::UserFolder,
            picture_wallpapers_dir(pictures.as_deref()),
            "My Wallpapers",
        ),
    ];
    let mut paths = HashSet::new();
    paths.insert(normalized(&root));
    for (kind, maybe_path, name) in detected {
        if let Some(path) = maybe_path {
            add_detected_source(
                &mut sources,
                &mut paths,
                pictures.as_deref(),
                kind,
                path,
                name,
            );
        }
    }
    WallpaperLibrary {
        download_root: root.to_string_lossy().into_owned(),
        sources,
        provider_records: BTreeMap::new(),
        legacy_folders_migrated: false,
        legacy_history_migrated: false,
    }
}
fn load() -> Result<WallpaperLibrary, String> {
    let path = data_path()?;
    if !path.exists() {
        return Ok(default_library());
    }
    let data: WallpaperLibrary =
        serde_json::from_slice(&fs::read(path).map_err(|e| e.to_string())?)
            .map_err(|e| format!("Invalid wallpaper library: {e}"))?;
    Ok(data)
}
fn write(data: &WallpaperLibrary) -> Result<(), String> {
    let path = data_path()?;
    let dir = path.parent().ok_or("Invalid config path")?;
    fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let tmp = dir.join(format!(
        ".wallpaper-library-{}-{}-{}.tmp",
        std::process::id(),
        now_ms(),
        SAVE_COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp)
            .map_err(|e| e.to_string())?;
        let bytes = serde_json::to_vec_pretty(data).map_err(|e| e.to_string())?;
        if let Err(e) = file.write_all(&bytes).and_then(|_| file.sync_all()) {
            let _ = fs::remove_file(&tmp);
            return Err(e.to_string());
        }
    }
    fs::rename(&tmp, path).map_err(|e| {
        let _ = fs::remove_file(&tmp);
        e.to_string()
    })
}
fn with_mut<R>(f: impl FnOnce(&mut WallpaperLibrary) -> Result<R, String>) -> Result<R, String> {
    let _guard = LIBRARY_LOCK.lock().map_err(|e| e.to_string())?;
    let mut data = load()?;
    let result = f(&mut data)?;
    write(&data)?;
    Ok(result)
}
fn same_path(a: &str, b: &str) -> bool {
    normalized(Path::new(a)) == normalized(Path::new(b))
}
fn add_source_internal(
    data: &mut WallpaperLibrary,
    kind: SourceKind,
    path: PathBuf,
    name: &str,
    removable: bool,
) -> Result<(), String> {
    if !path.is_dir() {
        return Err(format!("Folder does not exist: {}", path.display()));
    }
    let new = source(kind, path, name, true, removable);
    if !data.sources.iter().any(|s| same_path(&s.path, &new.path)) {
        data.sources.push(new);
    }
    Ok(())
}
pub fn valid_image(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .map(|meta| meta.file_type().is_file())
        .unwrap_or(false)
        && supported_ext(path)
        && image::image_dimensions(path).is_ok()
}
pub fn supported_ext(path: &Path) -> bool {
    path.extension()
        .and_then(|x| x.to_str())
        .map(|x| {
            matches!(
                x.to_ascii_lowercase().as_str(),
                "jpg" | "jpeg" | "png" | "webp"
            )
        })
        .unwrap_or(false)
}
fn valid_record_path(value: &Option<String>) -> Option<String> {
    value
        .as_ref()
        .filter(|p| valid_image(Path::new(p)))
        .cloned()
}
fn repair_with_active_root(data: &mut WallpaperLibrary, active_root: Option<&Path>) {
    for record in data.provider_records.values_mut() {
        record.cache_path = valid_record_path(&record.cache_path);
        record.active_path = record
            .active_path
            .as_ref()
            .filter(|path| {
                active_root
                    .is_some_and(|root| normalized(Path::new(path)).starts_with(normalized(root)))
                    && valid_image(Path::new(path))
            })
            .cloned();
        record.saved_path = valid_record_path(&record.saved_path);
    }
}
fn repair(data: &mut WallpaperLibrary) {
    let active_root = managed_active_root().ok();
    repair_with_active_root(data, active_root.as_deref());
}
fn record_key(provider: &str, id: &str) -> String {
    format!("{provider}:{id}")
}
fn safe_segment(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 80
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}
pub fn provider_dir(root: &Path, provider: &str) -> Result<PathBuf, String> {
    if !safe_segment(provider) {
        return Err("Invalid provider".into());
    }
    Ok(root.join(if provider == PROVIDER {
        "Wallhaven"
    } else {
        provider
    }))
}
pub fn provider_cache_dir(provider: &str) -> Result<PathBuf, String> {
    if !safe_segment(provider) {
        return Err("Invalid provider".into());
    }
    Ok(dirs::cache_dir()
        .ok_or("XDG cache unavailable")?
        .join("matugen-studio")
        .join(provider))
}
fn active_root_for(data_dir: &Path) -> PathBuf {
    data_dir.join("matugen-studio/active-wallpapers")
}
pub fn managed_active_root() -> Result<PathBuf, String> {
    Ok(active_root_for(
        &dirs::data_dir().ok_or("XDG data directory unavailable")?,
    ))
}
#[derive(Default)]
pub struct ProviderPaths {
    pub cache: Option<String>,
    pub active: Option<String>,
    pub saved: Option<String>,
}
impl ProviderPaths {
    pub fn for_colors(&self) -> Option<&str> {
        self.saved.as_deref().or(self.cache.as_deref())
    }
    pub fn for_apply(&self) -> Option<&str> {
        self.saved.as_deref().or(self.active.as_deref())
    }
}
pub fn provider_paths(provider: &str, id: &str) -> Result<ProviderPaths, String> {
    if !safe_segment(provider) || !safe_segment(id) {
        return Err("Invalid provider identity".into());
    }
    let _guard = LIBRARY_LOCK.lock().map_err(|e| e.to_string())?;
    let mut data = load()?;
    repair(&mut data);
    let result = data
        .provider_records
        .get(&record_key(provider, id))
        .map(|r| ProviderPaths {
            cache: r.cache_path.clone(),
            active: r.active_path.clone(),
            saved: r.saved_path.clone(),
        })
        .unwrap_or_default();
    write(&data)?;
    Ok(result)
}
pub fn record_provider_path(
    provider: &str,
    id: &str,
    path: &Path,
    saved: bool,
    thumb: Option<String>,
) -> Result<(), String> {
    if !safe_segment(provider) || !safe_segment(id) || !valid_image(path) {
        return Err("Invalid provider wallpaper".into());
    }
    with_mut(|data| {
        let record = data
            .provider_records
            .entry(record_key(provider, id))
            .or_insert_with(|| ProviderRecord {
                provider: provider.into(),
                provider_id: id.into(),
                ..Default::default()
            });
        if saved {
            record.saved_path = Some(path.to_string_lossy().into_owned());
            record.saved_at = Some(now_ms());
        } else {
            record.cache_path = Some(path.to_string_lossy().into_owned());
        }
        if thumb.is_some() {
            record.thumb = thumb;
        }
        record.last_used_at = Some(now_ms());
        record.hidden_from_history = false;
        Ok(())
    })
}
pub fn record_managed_active_path(
    provider: &str,
    id: &str,
    path: &Path,
    thumb: Option<String>,
) -> Result<(), String> {
    if !safe_segment(provider)
        || !safe_segment(id)
        || !valid_image(path)
        || !normalized(path).starts_with(normalized(&managed_active_root()?))
    {
        return Err("Invalid managed wallpaper".into());
    }
    with_mut(|data| {
        let record = data
            .provider_records
            .entry(record_key(provider, id))
            .or_insert_with(|| ProviderRecord {
                provider: provider.into(),
                provider_id: id.into(),
                ..Default::default()
            });
        record.active_path = Some(path.to_string_lossy().into_owned());
        record.last_used_at = Some(now_ms());
        record.hidden_from_history = false;
        if thumb.is_some() {
            record.thumb = thumb;
        }
        Ok(())
    })
}
fn scan_one(
    source: &WallpaperSource,
    seen: &mut HashSet<PathBuf>,
    items: &mut Vec<WallpaperItem>,
    errors: &mut Vec<SourceScanError>,
) {
    if matches!(
        source.kind,
        SourceKind::MatugenDownloads | SourceKind::DownloadArchive
    ) && !Path::new(&source.path).exists()
    {
        return;
    }
    let mut stack = vec![(PathBuf::from(&source.path), 0usize)];
    while let Some((dir, depth)) = stack.pop() {
        let entries = match fs::read_dir(&dir) {
            Ok(x) => x,
            Err(e) => {
                errors.push(SourceScanError {
                    source_id: source.id.clone(),
                    message: format!("{}: {e}", dir.display()),
                });
                continue;
            }
        };
        for entry in entries {
            let entry = match entry {
                Ok(x) => x,
                Err(e) => {
                    errors.push(SourceScanError {
                        source_id: source.id.clone(),
                        message: e.to_string(),
                    });
                    continue;
                }
            };
            let path = entry.path();
            let file_type = match entry.file_type() {
                Ok(x) => x,
                Err(_) => continue,
            };
            if file_type.is_symlink() {
                continue;
            }
            if file_type.is_dir() {
                if source.recursive && depth < MAX_SCAN_DEPTH {
                    stack.push((path, depth + 1));
                }
                continue;
            }
            if !file_type.is_file()
                || !supported_ext(&path)
                || image::image_dimensions(&path).is_err()
            {
                continue;
            }
            let key = normalized(&path);
            if !seen.insert(key.clone()) {
                continue;
            }
            let file_name = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
            items.push(WallpaperItem {
                path: key.to_string_lossy().into_owned(),
                file_name,
                source_id: source.id.clone(),
                source_kind: source.kind,
                provider: None,
                provider_id: None,
            });
        }
    }
}
fn scan(data: &WallpaperLibrary, current: Option<&Path>) -> ScanResult {
    let mut seen = HashSet::new();
    let mut items = Vec::new();
    let mut errors = Vec::new();
    for source in data.sources.iter().filter(|s| s.enabled) {
        scan_one(source, &mut seen, &mut items, &mut errors)
    }
    if let Some(current) = current {
        if valid_image(current) {
            let key = normalized(current);
            if seen.insert(key.clone()) {
                items.push(WallpaperItem {
                    path: key.to_string_lossy().into_owned(),
                    file_name: current
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into_owned(),
                    source_id: "current-wallpaper".into(),
                    source_kind: SourceKind::CurrentWallpaper,
                    provider: None,
                    provider_id: None,
                });
            }
        }
    }
    let saved: HashMap<_, _> = data
        .provider_records
        .values()
        .filter_map(|record| {
            record.saved_path.as_ref().map(|path| {
                (
                    normalized(Path::new(path)),
                    (record.provider.clone(), record.provider_id.clone()),
                )
            })
        })
        .collect();
    for item in &mut items {
        if let Some((provider, id)) = saved.get(Path::new(&item.path)) {
            item.provider = Some(provider.clone());
            item.provider_id = Some(id.clone());
        }
    }
    items.sort_by(|a, b| a.path.cmp(&b.path));
    ScanResult { items, errors }
}
#[tauri::command]
pub fn get_wallpaper_library() -> Result<WallpaperLibrary, String> {
    let _guard = LIBRARY_LOCK.lock().map_err(|e| e.to_string())?;
    let mut data = load()?;
    repair(&mut data);
    write(&data)?;
    Ok(data)
}
fn migrate_legacy_history(
    data: &mut WallpaperLibrary,
    entries: Vec<LegacyHistoryEntry>,
    cache: &Path,
) {
    for entry in entries {
        if !safe_segment(&entry.id) {
            continue;
        }
        let path = PathBuf::from(entry.local_path);
        let cached = valid_image(&path) && normalized(&path).starts_with(normalized(cache));
        let record = ProviderRecord {
            provider: PROVIDER.into(),
            provider_id: entry.id.clone(),
            cache_path: cached.then(|| path.to_string_lossy().into_owned()),
            active_path: None,
            saved_path: None,
            thumb: entry.thumb,
            last_used_at: entry.downloaded_at,
            saved_at: None,
            hidden_from_history: false,
        };
        data.provider_records
            .entry(record_key(PROVIDER, &entry.id))
            .or_insert(record);
    }
}
#[tauri::command]
pub fn initialize_wallpaper_library(
    legacy_folders: Vec<String>,
    legacy_history: Vec<LegacyHistoryEntry>,
) -> Result<WallpaperLibrary, String> {
    with_mut(|data| {
        if !data.legacy_folders_migrated {
            for path in legacy_folders {
                let path = PathBuf::from(path);
                if path.is_dir() {
                    let _ = add_source_internal(
                        data,
                        SourceKind::UserFolder,
                        path,
                        "Imported folder",
                        true,
                    );
                }
            }
            data.legacy_folders_migrated = true;
        }
        if !data.legacy_history_migrated {
            let cache = dirs::cache_dir()
                .ok_or("XDG cache unavailable")?
                .join("matugen-studio/wallhaven");
            migrate_legacy_history(data, legacy_history, &cache);
            data.legacy_history_migrated = true;
        }
        repair(data);
        Ok(data.clone())
    })
}
#[tauri::command]
pub fn add_wallpaper_source(path: String) -> Result<WallpaperLibrary, String> {
    with_mut(|data| {
        let path = PathBuf::from(path);
        let name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        add_source_internal(data, SourceKind::UserFolder, path, &name, true)?;
        Ok(data.clone())
    })
}
fn remove_source(data: &mut WallpaperLibrary, id: &str) -> Result<(), String> {
    let pos = data
        .sources
        .iter()
        .position(|s| s.id == id)
        .ok_or("Unknown source")?;
    if !data.sources[pos].removable {
        return Err("Managed source cannot be removed".into());
    }
    data.sources.remove(pos);
    Ok(())
}
fn enable_source(data: &mut WallpaperLibrary, id: &str, enabled: bool) -> Result<(), String> {
    let src = data
        .sources
        .iter_mut()
        .find(|s| s.id == id)
        .ok_or("Unknown source")?;
    src.enabled = enabled;
    Ok(())
}
#[tauri::command]
pub fn remove_wallpaper_source(id: String) -> Result<WallpaperLibrary, String> {
    with_mut(|data| {
        remove_source(data, &id)?;
        Ok(data.clone())
    })
}
#[tauri::command]
pub fn set_wallpaper_source_enabled(id: String, enabled: bool) -> Result<WallpaperLibrary, String> {
    with_mut(|data| {
        enable_source(data, &id, enabled)?;
        Ok(data.clone())
    })
}
fn change_download_root(data: &mut WallpaperLibrary, new: PathBuf) {
    let old = PathBuf::from(&data.download_root);
    if normalized(&old) == normalized(&new) {
        return;
    }
    if let Some(src) = data
        .sources
        .iter_mut()
        .find(|s| s.kind == SourceKind::MatugenDownloads)
    {
        src.kind = SourceKind::DownloadArchive;
        src.removable = false;
        src.display_name = "Previous Matugen downloads".into();
    }
    data.sources
        .retain(|source| normalized(Path::new(&source.path)) != normalized(&new));
    data.download_root = new.to_string_lossy().into_owned();
    data.sources.insert(
        0,
        source(
            SourceKind::MatugenDownloads,
            new,
            "Matugen Studio Downloads",
            true,
            false,
        ),
    );
}
#[tauri::command]
pub fn set_wallpaper_download_root(path: String) -> Result<WallpaperLibrary, String> {
    with_mut(|data| {
        let new = PathBuf::from(path);
        if !new.is_absolute() {
            return Err("Download folder must be absolute".into());
        }
        fs::create_dir_all(&new).map_err(|e| e.to_string())?;
        change_download_root(data, normalized(&new));
        Ok(data.clone())
    })
}
#[tauri::command]
pub async fn rescan_wallpaper_library() -> Result<ScanResult, String> {
    let current = super::kde::get_kde_current_wallpaper()
        .await
        .ok()
        .flatten()
        .map(PathBuf::from);
    tauri::async_runtime::spawn_blocking(move || {
        let data = get_wallpaper_library()?;
        Ok(scan(&data, current.as_deref()))
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
pub fn hide_provider_history(provider: String, id: Option<String>) -> Result<(), String> {
    if !safe_segment(&provider) || id.as_ref().is_some_and(|id| !safe_segment(id)) {
        return Err("Invalid provider identity".into());
    }
    with_mut(|data| {
        for record in data.provider_records.values_mut() {
            if record.provider == provider && id.as_ref().is_none_or(|id| *id == record.provider_id)
            {
                record.hidden_from_history = true;
            }
        }
        Ok(())
    })
}
#[tauri::command]
pub fn get_provider_wallpapers() -> Result<Vec<ProviderRecord>, String> {
    let data = get_wallpaper_library()?;
    Ok(data.provider_records.into_values().collect())
}

fn image_matches_extension(path: &Path, bytes: &[u8]) -> bool {
    let kind = image::guess_format(bytes).ok();
    matches!((path.extension().and_then(|x|x.to_str()).map(|x|x.to_ascii_lowercase()),kind),(Some(ext),Some(image::ImageFormat::Jpeg)) if ext=="jpg"||ext=="jpeg")
        || matches!((path.extension().and_then(|x|x.to_str()).map(|x|x.to_ascii_lowercase()),kind),(Some(ext),Some(image::ImageFormat::Png)) if ext=="png")
        || matches!((path.extension().and_then(|x|x.to_str()).map(|x|x.to_ascii_lowercase()),kind),(Some(ext),Some(image::ImageFormat::WebP)) if ext=="webp")
}
pub fn save_bytes(dir: &Path, id: &str, extension: &str, bytes: &[u8]) -> Result<PathBuf, String> {
    if !safe_segment(id) || !matches!(extension, "jpg" | "jpeg" | "png" | "webp") {
        return Err("Invalid wallpaper filename".into());
    }
    if bytes.is_empty() || bytes.len() > 50 * 1024 * 1024 {
        return Err("Invalid image size".into());
    }
    let base = dir.join(format!("{id}.{extension}"));
    if !image_matches_extension(&base, bytes) || image::load_from_memory(bytes).is_err() {
        return Err("Invalid image content".into());
    }
    fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let hash = format!("{:x}", md5::compute(bytes));
    let mut dest = base;
    if fs::symlink_metadata(&dest).is_ok() {
        if !fs::symlink_metadata(&dest)
            .map_err(|e| e.to_string())?
            .file_type()
            .is_file()
        {
            return Err("Destination is not a regular file".into());
        }
        let existing = fs::read(&dest).map_err(|e| e.to_string())?;
        if existing == bytes {
            return Ok(dest);
        }
        dest = dir.join(format!("{id}-{}.{extension}", &hash[..12]));
        if fs::symlink_metadata(&dest).is_ok() {
            if !fs::symlink_metadata(&dest)
                .map_err(|e| e.to_string())?
                .file_type()
                .is_file()
            {
                return Err("Destination is not a regular file".into());
            }
            let existing = fs::read(&dest).map_err(|e| e.to_string())?;
            if existing == bytes {
                return Ok(dest);
            }
            return Err("Wallpaper filename collision".into());
        }
    }
    let temp = dir.join(format!(
        ".{id}-{}-{}-{}.tmp",
        std::process::id(),
        now_ms(),
        SAVE_COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)
            .map_err(|e| e.to_string())?;
        if let Err(e) = file.write_all(bytes).and_then(|_| file.sync_all()) {
            let _ = fs::remove_file(&temp);
            return Err(e.to_string());
        }
    }
    let linked = fs::hard_link(&temp, &dest);
    let _ = fs::remove_file(&temp);
    linked.map_err(|e| e.to_string())?;
    Ok(dest)
}
fn save_cached_to_root(
    root: &Path,
    provider: &str,
    id: &str,
    ext: &str,
    cache_path: Option<&Path>,
    bytes: Option<&[u8]>,
) -> Result<PathBuf, String> {
    let dir = provider_dir(root, provider)?;
    let bytes = if let Some(path) = cache_path.filter(|p| valid_image(p)) {
        fs::read(path).map_err(|e| e.to_string())?
    } else {
        bytes.ok_or("No cached image or downloaded bytes")?.to_vec()
    };
    save_bytes(&dir, id, ext, &bytes)
}
fn promote_active_to_root(
    active_root: &Path,
    provider: &str,
    id: &str,
    ext: &str,
    source_path: &Path,
) -> Result<PathBuf, String> {
    if !safe_segment(provider) || !valid_image(source_path) {
        return Err("Invalid source wallpaper".into());
    }
    let bytes = fs::read(source_path).map_err(|e| e.to_string())?;
    save_bytes(&active_root.join(provider), id, ext, &bytes)
}
pub fn promote_managed_active(
    provider: &str,
    id: &str,
    ext: &str,
    source_path: &Path,
) -> Result<PathBuf, String> {
    promote_active_to_root(&managed_active_root()?, provider, id, ext, source_path)
}
fn stabilize_kde_path_at(
    image_path: &Path,
    cache_dir: &Path,
    data_dir: &Path,
) -> Result<Option<(PathBuf, String, String)>, String> {
    let app_cache = cache_dir.join("matugen-studio");
    if !image_path.starts_with(&app_cache) {
        return Ok(None);
    }
    if !valid_image(image_path) {
        return Err("Cached wallpaper is unavailable or invalid".into());
    }
    let canonical_cache = normalized(&app_cache);
    let canonical_path = normalized(image_path);
    let relative = canonical_path
        .strip_prefix(&canonical_cache)
        .map_err(|_| "Cached wallpaper escapes the application cache")?;
    let parts: Vec<_> = relative.components().collect();
    let [Component::Normal(provider), Component::Normal(file)] = parts.as_slice() else {
        return Err("Unsupported cached wallpaper path".into());
    };
    let provider = provider.to_str().ok_or("Invalid provider name")?;
    let file = Path::new(file);
    let id = file
        .file_stem()
        .and_then(|part| part.to_str())
        .ok_or("Invalid wallpaper ID")?;
    let ext = file
        .extension()
        .and_then(|part| part.to_str())
        .ok_or("Missing image extension")?
        .to_ascii_lowercase();
    if !safe_segment(provider) || !safe_segment(id) {
        return Err("Invalid cached wallpaper identity".into());
    }
    let path = promote_active_to_root(&active_root_for(data_dir), provider, id, &ext, image_path)?;
    Ok(Some((path, provider.into(), id.into())))
}
#[tauri::command]
pub fn prepare_wallpaper_for_kde(image_path: String) -> Result<String, String> {
    let path = PathBuf::from(&image_path);
    let Some(cache_dir) = dirs::cache_dir() else {
        return Ok(image_path);
    };
    if !path.starts_with(cache_dir.join("matugen-studio")) {
        return Ok(image_path);
    }
    let data_dir = dirs::data_dir().ok_or("XDG data directory unavailable")?;
    if let Some((active, provider, id)) = stabilize_kde_path_at(&path, &cache_dir, &data_dir)? {
        record_managed_active_path(&provider, &id, &active, None)?;
        Ok(active.to_string_lossy().into_owned())
    } else {
        Ok(image_path)
    }
}
pub fn save_cached(
    provider: &str,
    id: &str,
    ext: &str,
    cache_path: Option<&Path>,
    bytes: Option<&[u8]>,
) -> Result<PathBuf, String> {
    let data = get_wallpaper_library()?;
    let result = save_cached_to_root(
        Path::new(&data.download_root),
        provider,
        id,
        ext,
        cache_path,
        bytes,
    )?;
    record_provider_path(provider, id, &result, true, None)?;
    Ok(result)
}
pub fn emit_changed(app: &tauri::AppHandle) {
    let _ = app.emit("wallpaper-library-changed", ());
}
pub fn allow_asset(app: &tauri::AppHandle, path: &Path) {
    let _ = app.asset_protocol_scope().allow_file(path);
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgb, RgbImage};
    use tempfile::tempdir;

    fn png(path: &Path) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        RgbImage::from_pixel(2, 2, Rgb([20, 40, 60]))
            .save(path)
            .unwrap();
    }
    fn empty(root: &Path) -> WallpaperLibrary {
        WallpaperLibrary {
            download_root: root.join("downloads").to_string_lossy().into_owned(),
            sources: Vec::new(),
            provider_records: BTreeMap::new(),
            legacy_folders_migrated: false,
            legacy_history_migrated: false,
        }
    }
    #[test]
    fn download_root_uses_supplied_pictures_path() {
        assert_eq!(
            download_root_for(Path::new("/xdg/Pictures")),
            PathBuf::from("/xdg/Pictures/Wallpapers/Matugen Studio")
        );
        assert_eq!(
            provider_dir(Path::new("/library"), "wallhaven").unwrap(),
            PathBuf::from("/library/Wallhaven")
        );
        assert!(provider_dir(Path::new("/library"), "../../bad").is_err());
    }
    #[test]
    fn pictures_root_is_not_an_automatic_wallpaper_source() {
        let dir = tempdir().unwrap();
        let pictures = dir.path().join("Pictures");
        fs::create_dir(&pictures).unwrap();
        png(&pictures.join("root.png"));
        assert!(picture_wallpapers_dir(Some(&pictures)).is_none());
        assert!(picture_wallpapers_dir(None).is_none());
        let wallpapers = pictures.join("Wallpapers");
        png(&wallpapers.join("nested/personal.png"));
        assert_eq!(
            picture_wallpapers_dir(Some(&pictures)),
            Some(wallpapers.clone())
        );
        let mut data = empty(dir.path());
        let mut paths = HashSet::new();
        add_detected_source(
            &mut data.sources,
            &mut paths,
            Some(&pictures),
            SourceKind::KdeUser,
            pictures.clone(),
            "KDE",
        );
        assert!(
            data.sources.is_empty(),
            "Pictures root must not be scanned even if it overlaps another automatic source"
        );
        add_detected_source(
            &mut data.sources,
            &mut paths,
            Some(&pictures),
            SourceKind::UserFolder,
            wallpapers,
            "personal",
        );
        let result = scan(&data, None);
        assert_eq!(result.items.len(), 1);
        assert_eq!(result.items[0].file_name, "personal.png");
    }
    #[test]
    fn remote_apply_promotes_cache_into_xdg_data_without_saving() {
        let dir = tempdir().unwrap();
        let cache_dir = dir.path().join("cache");
        let data_dir = dir.path().join("data");
        let cached = cache_dir.join("matugen-studio/wallhaven/remote.png");
        png(&cached);
        let mut paths = ProviderPaths {
            cache: Some(cached.to_string_lossy().into_owned()),
            ..Default::default()
        };
        assert!(
            paths.for_apply().is_none(),
            "KDE must never receive a cache-only path"
        );
        let (active, provider, id) = stabilize_kde_path_at(&cached, &cache_dir, &data_dir)
            .unwrap()
            .unwrap();
        assert_eq!(
            active,
            active_root_for(&data_dir).join("wallhaven/remote.png")
        );
        assert_eq!((provider.as_str(), id.as_str()), ("wallhaven", "remote"));
        assert!(active.exists());
        paths.active = Some(active.to_string_lossy().into_owned());
        assert_eq!(paths.for_apply(), Some(active.to_str().unwrap()));
        assert!(paths.saved.is_none());
        let mut library = empty(dir.path());
        library.sources.push(source(
            SourceKind::MatugenDownloads,
            dir.path().join("downloads"),
            "downloads",
            true,
            false,
        ));
        assert!(
            scan(&library, None).items.is_empty(),
            "managed active is not a download"
        );
        fs::remove_file(&cached).unwrap();
        assert!(
            active.exists(),
            "cache cleanup must not remove the active wallpaper"
        );
        assert_eq!(paths.for_apply(), Some(active.to_str().unwrap()));
        library.provider_records.insert(
            "wallhaven:remote".into(),
            ProviderRecord {
                provider: "wallhaven".into(),
                provider_id: "remote".into(),
                cache_path: Some(cached.to_string_lossy().into_owned()),
                active_path: Some(active.to_string_lossy().into_owned()),
                ..Default::default()
            },
        );
        repair_with_active_root(&mut library, Some(&active_root_for(&data_dir)));
        let record = &library.provider_records["wallhaven:remote"];
        assert!(record.cache_path.is_none());
        assert_eq!(record.active_path.as_deref(), active.to_str());
        assert!(record.saved_path.is_none());
        library
            .provider_records
            .get_mut("wallhaven:remote")
            .unwrap()
            .active_path = Some(
            dir.path()
                .join("outside.png")
                .to_string_lossy()
                .into_owned(),
        );
        png(&dir.path().join("outside.png"));
        repair_with_active_root(&mut library, Some(&active_root_for(&data_dir)));
        assert!(library.provider_records["wallhaven:remote"]
            .active_path
            .is_none());
    }
    #[test]
    fn load_colors_uses_cache_without_creating_managed_active() {
        let dir = tempdir().unwrap();
        let cached = dir.path().join("cache/matugen-studio/wallhaven/colors.png");
        png(&cached);
        let paths = ProviderPaths {
            cache: Some(cached.to_string_lossy().into_owned()),
            ..Default::default()
        };
        assert_eq!(paths.for_colors(), Some(cached.to_str().unwrap()));
        assert!(paths.for_apply().is_none());
        assert!(!active_root_for(&dir.path().join("data")).exists());
        let saved = dir.path().join("saved.png");
        png(&saved);
        let with_saved = ProviderPaths {
            saved: Some(saved.to_string_lossy().into_owned()),
            ..paths
        };
        assert_eq!(with_saved.for_colors(), Some(saved.to_str().unwrap()));
        assert_eq!(with_saved.for_apply(), Some(saved.to_str().unwrap()));
    }
    #[test]
    fn older_provider_records_default_to_no_managed_active_path() {
        let record: ProviderRecord = serde_json::from_value(serde_json::json!({
            "provider": "wallhaven", "providerId": "legacy", "cachePath": null,
            "savedPath": null, "thumb": null, "lastUsedAt": null, "savedAt": null
        }))
        .unwrap();
        assert!(record.active_path.is_none());
        assert!(record.saved_path.is_none());
    }
    #[test]
    fn download_can_reuse_managed_active_after_cache_is_deleted() {
        let dir = tempdir().unwrap();
        let cached = dir.path().join("cache/matugen-studio/wallhaven/reuse.png");
        png(&cached);
        let active = promote_active_to_root(
            &active_root_for(&dir.path().join("data")),
            "wallhaven",
            "reuse",
            "png",
            &cached,
        )
        .unwrap();
        let original = fs::read(&active).unwrap();
        fs::remove_file(cached).unwrap();
        let saved = save_cached_to_root(
            &dir.path().join("downloads"),
            "wallhaven",
            "reuse",
            "png",
            Some(&active),
            None,
        )
        .unwrap();
        assert_eq!(fs::read(&saved).unwrap(), original);
        assert!(active.exists());
        assert!(saved.starts_with(dir.path().join("downloads/Wallhaven")));
    }
    #[test]
    fn recursive_scan_formats_validation_and_no_symlink_loops() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        for ext in ["jpg", "jpeg", "png", "webp"] {
            png(&root.join("nested/deep").join(format!("image.{ext}")));
        }
        fs::write(root.join("nested/deep/ignored.txt"), b"x").unwrap();
        fs::write(root.join("nested/deep/corrupt.jpg"), b"not image").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(root, root.join("nested/deep/loop")).unwrap();
        let mut data = empty(root);
        data.sources.push(source(
            SourceKind::UserFolder,
            root.into(),
            "fixture",
            true,
            true,
        ));
        let result = scan(&data, None);
        assert_eq!(result.items.len(), 4);
        assert!(result
            .items
            .iter()
            .all(|item| item.path.contains("nested/deep")));
        assert!(result.errors.is_empty());
    }
    #[test]
    fn overlapping_sources_and_current_wallpaper_are_deduplicated() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        png(&root.join("a.png"));
        png(&root.join("other.png"));
        let mut data = empty(root);
        data.sources.push(source(
            SourceKind::KdeSystem,
            root.into(),
            "system",
            true,
            false,
        ));
        data.sources.push(source(
            SourceKind::UserFolder,
            root.into(),
            "user",
            true,
            true,
        ));
        assert_eq!(scan(&data, Some(&root.join("a.png"))).items.len(), 2);
        let current = tempdir().unwrap();
        png(&current.path().join("external.png"));
        let result = scan(&data, Some(&current.path().join("external.png")));
        assert_eq!(result.items.len(), 3);
        assert_eq!(
            result
                .items
                .iter()
                .filter(|i| i.source_kind == SourceKind::CurrentWallpaper)
                .count(),
            1
        );
    }
    #[test]
    fn missing_disabled_and_removable_sources_are_safe() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        png(&root.join("good.png"));
        let mut data = empty(root);
        add_source_internal(&mut data, SourceKind::UserFolder, root.into(), "user", true).unwrap();
        add_source_internal(
            &mut data,
            SourceKind::UserFolder,
            root.into(),
            "duplicate",
            true,
        )
        .unwrap();
        assert_eq!(data.sources.len(), 1);
        let user = data.sources[0].id.clone();
        enable_source(&mut data, &user, false).unwrap();
        assert!(scan(&data, None).items.is_empty());
        enable_source(&mut data, &user, true).unwrap();
        data.sources.push(source(
            SourceKind::KdeSystem,
            root.join("missing"),
            "missing",
            true,
            false,
        ));
        let result = scan(&data, None);
        assert_eq!(result.items.len(), 1);
        assert_eq!(result.errors.len(), 1);
        let system_id = data.sources[1].id.clone();
        assert!(remove_source(&mut data, &system_id).is_err());
        remove_source(&mut data, &user).unwrap();
        assert!(root.join("good.png").exists());
    }
    #[test]
    fn cache_and_permanent_save_are_separate_and_idempotent() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        let input = root.join("input.png");
        png(&input);
        let bytes = fs::read(&input).unwrap();
        let cache = save_bytes(&root.join("cache/wallhaven"), "abc123", "png", &bytes).unwrap();
        assert!(!root.join("downloads/Wallhaven").exists());
        let saved = save_cached_to_root(
            &root.join("downloads"),
            "wallhaven",
            "abc123",
            "png",
            Some(&cache),
            None,
        )
        .unwrap();
        assert!(saved.exists());
        assert_eq!(
            save_cached_to_root(
                &root.join("downloads"),
                "wallhaven",
                "abc123",
                "png",
                Some(&cache),
                None
            )
            .unwrap(),
            saved
        );
        fs::remove_file(cache).unwrap();
        assert!(saved.exists());
    }
    #[test]
    fn save_rejects_traversal_bad_extensions_and_partial_files() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        let input = root.join("input.png");
        png(&input);
        let bytes = fs::read(input).unwrap();
        let target = root.join("target");
        assert!(save_bytes(&target, "../../escape", "png", &bytes).is_err());
        assert!(save_bytes(&target, "safe", "exe", &bytes).is_err());
        assert!(save_bytes(&target, "safe", "jpg", &bytes).is_err());
        assert!(save_bytes(&target, "safe", "png", b"broken").is_err());
        assert!(!target.exists());
    }
    #[test]
    fn collision_is_deterministic_and_never_overwrites() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        let first = root.join("first.png");
        png(&first);
        let a = fs::read(first).unwrap();
        let mut changed = RgbImage::from_pixel(2, 2, Rgb([200, 10, 10]));
        changed.put_pixel(0, 0, Rgb([0, 0, 0]));
        let second = root.join("second.png");
        changed.save(&second).unwrap();
        let b = fs::read(second).unwrap();
        let target = root.join("save");
        let one = save_bytes(&target, "id", "png", &a).unwrap();
        let two = save_bytes(&target, "id", "png", &b).unwrap();
        assert_ne!(one, two);
        assert_eq!(fs::read(one).unwrap(), a);
        assert_eq!(fs::read(two).unwrap(), b);
    }
    #[test]
    fn dead_paths_are_repaired_without_marking_cache_as_saved() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        let cache = root.join("cache.png");
        png(&cache);
        let mut data = empty(root);
        data.provider_records.insert(
            "wallhaven:id".into(),
            ProviderRecord {
                provider: "wallhaven".into(),
                provider_id: "id".into(),
                cache_path: Some(cache.to_string_lossy().into_owned()),
                saved_path: Some(root.join("missing.png").to_string_lossy().into_owned()),
                ..Default::default()
            },
        );
        repair(&mut data);
        assert!(data.provider_records["wallhaven:id"].saved_path.is_none());
        assert!(data.provider_records["wallhaven:id"].cache_path.is_some());
        fs::remove_file(cache).unwrap();
        repair(&mut data);
        assert!(data.provider_records["wallhaven:id"].cache_path.is_none());
    }
    #[test]
    fn legacy_history_only_recovers_live_cache() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        let cache = root.join("cache");
        let live = cache.join("live.png");
        png(&live);
        let outside = root.join("outside.png");
        png(&outside);
        let mut data = empty(root);
        migrate_legacy_history(
            &mut data,
            vec![
                LegacyHistoryEntry {
                    id: "live".into(),
                    local_path: live.to_string_lossy().into_owned(),
                    thumb: Some("thumb".into()),
                    downloaded_at: Some(1),
                },
                LegacyHistoryEntry {
                    id: "outside".into(),
                    local_path: outside.to_string_lossy().into_owned(),
                    thumb: None,
                    downloaded_at: None,
                },
                LegacyHistoryEntry {
                    id: "missing".into(),
                    local_path: cache.join("gone.png").to_string_lossy().into_owned(),
                    thumb: None,
                    downloaded_at: None,
                },
            ],
            &cache,
        );
        assert!(data.provider_records["wallhaven:live"].cache_path.is_some());
        assert!(data.provider_records["wallhaven:live"].saved_path.is_none());
        assert!(data.provider_records["wallhaven:outside"]
            .cache_path
            .is_none());
        assert!(data.provider_records["wallhaven:missing"]
            .cache_path
            .is_none());
    }
    #[test]
    fn root_change_preserves_old_downloads_as_archive() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        let old = root.join("old");
        fs::create_dir(&old).unwrap();
        let item = old.join("photo.png");
        png(&item);
        let mut data = empty(root);
        data.download_root = old.to_string_lossy().into_owned();
        data.sources.push(source(
            SourceKind::MatugenDownloads,
            old.clone(),
            "downloads",
            true,
            false,
        ));
        let new = root.join("new");
        fs::create_dir(&new).unwrap();
        change_download_root(&mut data, new.clone());
        assert_eq!(data.download_root, new.to_string_lossy());
        assert!(data
            .sources
            .iter()
            .any(|s| s.kind == SourceKind::DownloadArchive));
        assert_eq!(scan(&data, None).items.len(), 1);
        assert!(item.exists());
        change_download_root(&mut data, old);
        assert_eq!(
            data.sources
                .iter()
                .filter(|s| s.kind == SourceKind::MatugenDownloads)
                .count(),
            1
        );
        assert_eq!(
            data.sources
                .iter()
                .filter(|s| s.kind == SourceKind::DownloadArchive)
                .count(),
            1
        );
    }
    #[test]
    fn saved_item_enters_library_with_provider_identity() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        let input = root.join("input.png");
        png(&input);
        let bytes = fs::read(input).unwrap();
        let mut data = empty(root);
        let saved = save_cached_to_root(
            Path::new(&data.download_root),
            "wallhaven",
            "identity",
            "png",
            None,
            Some(&bytes),
        )
        .unwrap();
        data.sources.push(source(
            SourceKind::MatugenDownloads,
            PathBuf::from(&data.download_root),
            "downloads",
            true,
            false,
        ));
        data.provider_records.insert(
            "wallhaven:identity".into(),
            ProviderRecord {
                provider: "wallhaven".into(),
                provider_id: "identity".into(),
                saved_path: Some(saved.to_string_lossy().into_owned()),
                ..Default::default()
            },
        );
        let result = scan(&data, None);
        assert_eq!(result.items.len(), 1);
        assert_eq!(result.items[0].provider.as_deref(), Some("wallhaven"));
    }
}
