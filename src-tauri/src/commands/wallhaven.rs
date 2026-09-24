use std::collections::VecDeque;
use std::path::Path;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use tokio::time::Instant;

const SEARCH_URL: &str = "https://wallhaven.cc/api/v1/search";
const MAX_REQUESTS_PER_WINDOW: usize = 45;
const RATE_LIMIT_WINDOW: Duration = Duration::from_secs(60);

/// Simple sliding-window limiter so bursts from search-as-you-type / pagination
/// never trip Wallhaven's 45 req/min guest limit; callers just await a slot.
struct RateLimiter {
    timestamps: Mutex<VecDeque<Instant>>,
}

impl RateLimiter {
    fn new() -> Self {
        Self {
            timestamps: Mutex::new(VecDeque::new()),
        }
    }

    async fn acquire(&self) {
        loop {
            let wait = {
                let mut timestamps = self.timestamps.lock().await;
                let now = Instant::now();
                while let Some(&front) = timestamps.front() {
                    if now.duration_since(front) > RATE_LIMIT_WINDOW {
                        timestamps.pop_front();
                    } else {
                        break;
                    }
                }

                if timestamps.len() < MAX_REQUESTS_PER_WINDOW {
                    timestamps.push_back(now);
                    None
                } else {
                    let oldest = *timestamps.front().expect("checked non-empty above");
                    Some(RATE_LIMIT_WINDOW - now.duration_since(oldest))
                }
            };

            match wait {
                None => return,
                Some(duration) => tokio::time::sleep(duration).await,
            }
        }
    }
}

pub struct WallhavenClient {
    http: reqwest::Client,
    rate_limiter: RateLimiter,
}

impl WallhavenClient {
    pub fn new() -> Self {
        let http = reqwest::Client::builder()
            .user_agent("MatugenStudio/0.1 (+https://github.com/raellx22/Matugen-Studio)")
            .build()
            .unwrap_or_default();

        Self {
            http,
            rate_limiter: RateLimiter::new(),
        }
    }
}

impl Default for WallhavenClient {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WallhavenSearchParams {
    pub q: Option<String>,
    pub categories: Option<String>,
    pub purity: Option<String>,
    pub sorting: Option<String>,
    pub order: Option<String>,
    pub top_range: Option<String>,
    pub colors: Option<String>,
    pub ratios: Option<String>,
    pub atleast: Option<String>,
    pub page: Option<u32>,
    pub seed: Option<String>,
    pub api_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WallhavenThumbs {
    pub large: String,
    pub original: String,
    pub small: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WallhavenWallpaper {
    pub id: String,
    pub url: String,
    #[serde(alias = "short_url")]
    pub short_url: String,
    pub views: u64,
    pub favorites: u64,
    pub source: String,
    pub purity: String,
    pub category: String,
    #[serde(alias = "dimension_x")]
    pub dimension_x: u32,
    #[serde(alias = "dimension_y")]
    pub dimension_y: u32,
    pub resolution: String,
    pub ratio: String,
    #[serde(alias = "file_size")]
    pub file_size: u64,
    #[serde(alias = "file_type")]
    pub file_type: String,
    #[serde(alias = "created_at")]
    pub created_at: String,
    pub colors: Vec<String>,
    pub path: String,
    pub thumbs: WallhavenThumbs,
    #[serde(default)]
    pub tags: Option<Vec<WallhavenTag>>,
    #[serde(default)]
    pub uploader: Option<WallhavenUploader>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WallhavenTag {
    pub id: u64,
    pub name: String,
    #[serde(default)]
    pub alias: String,
    #[serde(alias = "category_id")]
    pub category_id: u64,
    pub category: String,
    pub purity: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WallhavenUploader {
    pub username: String,
    pub group: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WallhavenMeta {
    #[serde(alias = "current_page")]
    pub current_page: u32,
    #[serde(alias = "last_page")]
    pub last_page: u32,
    #[serde(alias = "per_page")]
    pub per_page: serde_json::Value,
    pub total: u32,
    pub seed: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WallhavenSearchResponse {
    pub data: Vec<WallhavenWallpaper>,
    pub meta: WallhavenMeta,
}

#[tauri::command]
pub async fn wallhaven_search(
    client: tauri::State<'_, WallhavenClient>,
    params: WallhavenSearchParams,
) -> Result<WallhavenSearchResponse, String> {
    client.rate_limiter.acquire().await;

    let mut query: Vec<(&str, String)> = Vec::new();
    if let Some(q) = params.q.filter(|value| !value.trim().is_empty()) {
        query.push(("q", q));
    }
    query.push((
        "categories",
        params.categories.unwrap_or_else(|| "111".to_string()),
    ));
    query.push(("purity", params.purity.unwrap_or_else(|| "100".to_string())));
    query.push((
        "sorting",
        params.sorting.unwrap_or_else(|| "date_added".to_string()),
    ));
    query.push(("order", params.order.unwrap_or_else(|| "desc".to_string())));
    if let Some(top_range) = params.top_range.filter(|value| !value.trim().is_empty()) {
        query.push(("topRange", top_range));
    }
    if let Some(colors) = params.colors.filter(|value| !value.trim().is_empty()) {
        query.push(("colors", colors));
    }
    if let Some(ratios) = params.ratios.filter(|value| !value.trim().is_empty()) {
        query.push(("ratios", ratios));
    }
    if let Some(atleast) = params.atleast.filter(|value| !value.trim().is_empty()) {
        query.push(("atleast", atleast));
    }
    query.push(("page", params.page.unwrap_or(1).to_string()));
    if let Some(seed) = params.seed.filter(|value| !value.trim().is_empty()) {
        query.push(("seed", seed));
    }
    if let Some(api_key) = params.api_key.filter(|value| !value.trim().is_empty()) {
        query.push(("apikey", api_key));
    }

    let response = client
        .http
        .get(SEARCH_URL)
        .query(&query)
        .send()
        .await
        .map_err(|e| format!("Falha ao consultar Wallhaven: {e}"))?;

    let status = response.status();
    if !status.is_success() {
        return Err(format!("Wallhaven retornou o erro {status}"));
    }

    response
        .json::<WallhavenSearchResponse>()
        .await
        .map_err(|e| format!("Falha ao interpretar resposta do Wallhaven: {e}"))
}

fn validate_wallpaper_identity(id: &str, url: &str) -> Result<String, String> {
    if id.len() < 3 || id.len() > 40 || !id.bytes().all(|b| b.is_ascii_alphanumeric()) {
        return Err("Invalid Wallhaven ID".into());
    }
    let parsed = reqwest::Url::parse(url).map_err(|e| e.to_string())?;
    if parsed.scheme() != "https" || parsed.host_str() != Some("w.wallhaven.cc") {
        return Err("Wallpaper URL must use the Wallhaven image host".into());
    }
    let extension = Path::new(parsed.path())
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .ok_or("Missing image extension")?;
    if !matches!(extension.as_str(), "jpg" | "jpeg" | "png" | "webp") {
        return Err("Unsupported image extension".into());
    }
    Ok(extension)
}

async fn fetch_wallpaper(client: &WallhavenClient, url: &str) -> Result<Vec<u8>, String> {
    client.rate_limiter.acquire().await;
    let response = client
        .http
        .get(url)
        .send()
        .await
        .map_err(|e| format!("Failed to fetch wallpaper: {e}"))?;
    if !response.status().is_success() {
        return Err(format!("Wallhaven returned {}", response.status()));
    }
    if response
        .content_length()
        .is_some_and(|n| n > 50 * 1024 * 1024)
    {
        return Err("Wallpaper exceeds 50 MB".into());
    }
    let bytes = response.bytes().await.map_err(|e| e.to_string())?;
    if bytes.len() > 50 * 1024 * 1024 {
        return Err("Wallpaper exceeds 50 MB".into());
    }
    Ok(bytes.to_vec())
}

/// Compatibility command: Apply and Load Colors fetch only into disposable cache.
#[tauri::command]
pub async fn wallhaven_download(
    app: tauri::AppHandle,
    client: tauri::State<'_, WallhavenClient>,
    id: String,
    url: String,
    thumb: Option<String>,
) -> Result<String, String> {
    let extension = validate_wallpaper_identity(&id, &url)?;
    let cache = super::wallpaper_library::provider_cache_dir("wallhaven")?;
    if let Some(cached) = super::wallpaper_library::provider_paths("wallhaven", &id)?.cache {
        let path = std::path::PathBuf::from(cached);
        super::wallpaper_library::record_provider_path("wallhaven", &id, &path, false, thumb)?;
        super::wallpaper_library::allow_asset(&app, &path);
        return Ok(path.to_string_lossy().into_owned());
    }
    let candidate = cache.join(format!("{id}.{extension}"));
    if super::wallpaper_library::valid_image(&candidate) {
        super::wallpaper_library::record_provider_path("wallhaven", &id, &candidate, false, thumb)?;
        super::wallpaper_library::allow_asset(&app, &candidate);
        return Ok(candidate.to_string_lossy().into_owned());
    }
    let bytes = fetch_wallpaper(&client, &url).await?;
    let id_for_task = id.clone();
    let path = tauri::async_runtime::spawn_blocking(move || {
        super::wallpaper_library::save_bytes(&cache, &id_for_task, &extension, &bytes)
    })
    .await
    .map_err(|e| e.to_string())??;
    super::wallpaper_library::record_provider_path("wallhaven", &id, &path, false, thumb)?;
    super::wallpaper_library::allow_asset(&app, &path);
    Ok(path.to_string_lossy().into_owned())
}

/// Color generation may use a saved download, but otherwise remains cache-only.
#[tauri::command]
pub async fn wallhaven_prepare_colors(
    app: tauri::AppHandle,
    client: tauri::State<'_, WallhavenClient>,
    id: String,
    url: String,
    thumb: Option<String>,
) -> Result<String, String> {
    validate_wallpaper_identity(&id, &url)?;
    let paths = super::wallpaper_library::provider_paths("wallhaven", &id)?;
    if let Some(path) = paths.for_colors() {
        let path = std::path::PathBuf::from(path);
        if paths.saved.is_none() {
            super::wallpaper_library::record_provider_path("wallhaven", &id, &path, false, thumb)?;
        }
        super::wallpaper_library::allow_asset(&app, &path);
        return Ok(path.to_string_lossy().into_owned());
    }
    wallhaven_download(app, client, id, url, thumb).await
}

/// The KDE path must survive XDG cache cleanup. It is not a saved download.
#[tauri::command]
pub async fn wallhaven_prepare_active(
    app: tauri::AppHandle,
    client: tauri::State<'_, WallhavenClient>,
    id: String,
    url: String,
    thumb: Option<String>,
) -> Result<String, String> {
    let extension = validate_wallpaper_identity(&id, &url)?;
    let paths = super::wallpaper_library::provider_paths("wallhaven", &id)?;
    if let Some(path) = paths.for_apply() {
        super::wallpaper_library::allow_asset(&app, Path::new(path));
        return Ok(path.into());
    }
    let cache_path = match paths.cache {
        Some(path) => path,
        None => wallhaven_download(app.clone(), client, id.clone(), url, thumb.clone()).await?,
    };
    let id_for_task = id.clone();
    let path = tauri::async_runtime::spawn_blocking(move || {
        super::wallpaper_library::promote_managed_active(
            "wallhaven",
            &id_for_task,
            &extension,
            Path::new(&cache_path),
        )
    })
    .await
    .map_err(|e| e.to_string())??;
    super::wallpaper_library::record_managed_active_path("wallhaven", &id, &path, thumb)?;
    super::wallpaper_library::allow_asset(&app, &path);
    Ok(path.to_string_lossy().into_owned())
}

/// Permanent save reuses cached data when present and publishes it to the library.
#[tauri::command]
pub async fn wallhaven_save(
    app: tauri::AppHandle,
    client: tauri::State<'_, WallhavenClient>,
    id: String,
    url: String,
    thumb: Option<String>,
) -> Result<String, String> {
    let extension = validate_wallpaper_identity(&id, &url)?;
    let paths = super::wallpaper_library::provider_paths("wallhaven", &id)?;
    if let Some(path) = paths.saved {
        let path = std::path::PathBuf::from(path);
        super::wallpaper_library::record_provider_path("wallhaven", &id, &path, true, thumb)?;
        super::wallpaper_library::allow_asset(&app, &path);
        return Ok(path.to_string_lossy().into_owned());
    }
    let cache_candidate = super::wallpaper_library::provider_cache_dir("wallhaven")?
        .join(format!("{id}.{extension}"));
    let source_path = paths
        .cache
        .map(std::path::PathBuf::from)
        .or_else(|| {
            super::wallpaper_library::valid_image(&cache_candidate).then_some(cache_candidate)
        })
        .or_else(|| paths.active.map(std::path::PathBuf::from));
    let bytes = if source_path.is_none() {
        Some(fetch_wallpaper(&client, &url).await?)
    } else {
        None
    };
    let id_for_task = id.clone();
    let path = tauri::async_runtime::spawn_blocking(move || {
        super::wallpaper_library::save_cached(
            "wallhaven",
            &id_for_task,
            &extension,
            source_path.as_deref(),
            bytes.as_deref(),
        )
    })
    .await
    .map_err(|e| e.to_string())??;
    super::wallpaper_library::record_provider_path("wallhaven", &id, &path, true, thumb)?;
    super::wallpaper_library::allow_asset(&app, &path);
    super::wallpaper_library::emit_changed(&app);
    Ok(path.to_string_lossy().into_owned())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WallhavenSettingsData {
    #[serde(default)]
    pub purity: Vec<String>,
    #[serde(default)]
    pub categories: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct WallhavenSettingsEnvelope {
    data: WallhavenSettingsData,
}

#[tauri::command]
pub async fn wallhaven_validate_key(
    client: tauri::State<'_, WallhavenClient>,
    api_key: String,
) -> Result<WallhavenSettingsData, String> {
    let key = api_key.trim();
    if key.is_empty() {
        return Err("Informe uma chave de API".to_string());
    }

    client.rate_limiter.acquire().await;

    let response = client
        .http
        .get("https://wallhaven.cc/api/v1/settings")
        .query(&[("apikey", key)])
        .send()
        .await
        .map_err(|e| format!("Falha ao validar chave: {e}"))?;

    let status = response.status();
    if status == reqwest::StatusCode::UNAUTHORIZED
        || status == reqwest::StatusCode::FORBIDDEN
        || status == reqwest::StatusCode::NOT_FOUND
    {
        return Err("Chave de API inválida".to_string());
    }
    if !status.is_success() {
        return Err(format!("Wallhaven retornou o erro {status}"));
    }

    response
        .json::<WallhavenSettingsEnvelope>()
        .await
        .map(|envelope| envelope.data)
        .map_err(|e| format!("Falha ao interpretar resposta do Wallhaven: {e}"))
}

#[derive(Debug, Clone, Deserialize)]
struct WallhavenWallpaperEnvelope {
    data: WallhavenWallpaper,
}

#[tauri::command]
pub async fn wallhaven_get_wallpaper(
    client: tauri::State<'_, WallhavenClient>,
    id: String,
    api_key: Option<String>,
) -> Result<WallhavenWallpaper, String> {
    client.rate_limiter.acquire().await;

    let mut query: Vec<(&str, String)> = Vec::new();
    if let Some(key) = api_key.filter(|value| !value.trim().is_empty()) {
        query.push(("apikey", key));
    }

    let url = format!("https://wallhaven.cc/api/v1/w/{id}");
    let response = client
        .http
        .get(&url)
        .query(&query)
        .send()
        .await
        .map_err(|e| format!("Falha ao buscar detalhes do wallpaper: {e}"))?;

    let status = response.status();
    if !status.is_success() {
        return Err(format!("Wallhaven retornou o erro {status}"));
    }

    response
        .json::<WallhavenWallpaperEnvelope>()
        .await
        .map(|envelope| envelope.data)
        .map_err(|e| format!("Falha ao interpretar resposta do Wallhaven: {e}"))
}
