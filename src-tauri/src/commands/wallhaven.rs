use std::collections::VecDeque;
use std::fs;
use std::path::Path;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::Manager;
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

#[tauri::command]
pub async fn wallhaven_download(
    app: tauri::AppHandle,
    client: tauri::State<'_, WallhavenClient>,
    id: String,
    url: String,
) -> Result<String, String> {
    let cache_dir = dirs::cache_dir()
        .ok_or("Não foi possível localizar o diretório de cache")?
        .join("matugen-studio")
        .join("wallhaven");
    fs::create_dir_all(&cache_dir).map_err(|e| e.to_string())?;

    let extension = Path::new(&url)
        .extension()
        .and_then(|ext| ext.to_str())
        .filter(|ext| ext.len() <= 5)
        .unwrap_or("jpg");
    let file_path = cache_dir.join(format!("{id}.{extension}"));

    if file_path.exists() {
        allow_asset_file(&app, &file_path);
        return Ok(file_path.to_string_lossy().to_string());
    }

    client.rate_limiter.acquire().await;

    let response = client
        .http
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Falha ao baixar wallpaper: {e}"))?;

    let status = response.status();
    if !status.is_success() {
        return Err(format!("Wallhaven retornou o erro {status} ao baixar"));
    }

    let bytes = response
        .bytes()
        .await
        .map_err(|e| format!("Falha ao ler o conteúdo baixado: {e}"))?;

    fs::write(&file_path, &bytes).map_err(|e| e.to_string())?;

    allow_asset_file(&app, &file_path);
    Ok(file_path.to_string_lossy().to_string())
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

fn allow_asset_file(app: &tauri::AppHandle, path: &Path) {
    let _ = app.asset_protocol_scope().allow_file(path);
}
