use super::settings::{GenerationSettings, KdeIntegrationSettings};
use base64::{engine::general_purpose, Engine as B64Engine};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};
use tauri::Manager;

static PRESET_STORAGE_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

// ── New domain types ──────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct WallpaperInfo {
    pub filename: Option<String>,
    pub original_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base64: Option<String>,
    pub mime_type: Option<String>,
    pub sha256: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SchemeInfo {
    pub scheme_type: String,
    pub mode: String,
    #[serde(flatten)]
    pub generation: GenerationSettings,
    pub opacity: Option<f64>,
    pub source_color_hex: Option<String>,
    pub scheme_data: serde_json::Value,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CustomColor {
    pub name: String,
    pub value: String,
    pub blend: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TemplateSnapshot {
    pub name: String,
    pub target_app: Option<String>,
    pub enabled: bool,
    pub input_path: Option<String>,
    pub output_path: Option<String>,
    pub template_type: Option<String>,
    pub content: Option<String>,
    pub pre_hook: Option<String>,
    pub post_hook: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DesktopSettings {
    pub target_de: Option<String>,
    pub apply_wallpaper: bool,
    pub apply_kde_colorscheme: bool,
    pub apply_templates: bool,
    #[serde(default)]
    pub integrations: KdeIntegrationSettings,
}

impl Default for DesktopSettings {
    fn default() -> Self {
        Self {
            target_de: None,
            apply_wallpaper: true,
            apply_kde_colorscheme: true,
            apply_templates: true,
            integrations: KdeIntegrationSettings::default(),
        }
    }
}

/// Unified preset model — used for local storage and as the basis of the export format.
/// For local storage: `wallpaper.base64` is always `None`.
/// For `.matugen` exports: `wallpaper.base64` is populated by `export_preset`.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PresetV2 {
    pub version: u32,
    pub name: String,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub app_version: Option<String>,
    pub wallpaper: Option<WallpaperInfo>,
    pub scheme: SchemeInfo,
    pub custom_colors: Vec<CustomColor>,
    pub templates: Vec<TemplateSnapshot>,
    pub desktop: DesktopSettings,
}

/// Payload sent by the frontend when saving a preset.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SavePresetInput {
    pub name: String,
    pub wallpaper_path: Option<String>,
    pub scheme_type: String,
    pub mode: String,
    #[serde(flatten)]
    pub generation: GenerationSettings,
    pub opacity: Option<f64>,
    pub source_color_hex: Option<String>,
    pub scheme_data: serde_json::Value,
    pub custom_colors: Option<Vec<CustomColor>>,
    pub desktop: Option<DesktopSettings>,
}

/// Return type of `import_preset`, includes non-fatal warnings.
#[derive(Serialize, Deserialize, Debug)]
pub struct ImportResult {
    pub preset: PresetV2,
    pub warnings: Vec<String>,
}

// ── Legacy types ──────────────────────────────────────────────────────────────

/// Old local format stored in `gui-presets.json` (no `version` field).
#[derive(Serialize, Deserialize, Debug, Clone)]
struct LegacyLocalPreset {
    pub name: String,
    pub wallpaper_path: Option<String>,
    pub scheme_type: String,
    pub scheme_data: serde_json::Value,
}

/// Old export format (version 1 `.matugen` files).
#[derive(Serialize, Deserialize, Debug, Clone)]
struct ExportablePresetV1 {
    pub version: Option<u32>,
    pub name: String,
    pub scheme_type: String,
    pub scheme_data: serde_json::Value,
    pub installed_templates: Option<Vec<String>>,
    pub wallpaper_base64: Option<String>,
    pub wallpaper_filename: Option<String>,
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn get_presets_file_path() -> Result<PathBuf, String> {
    let config_dir = dirs::config_dir()
        .ok_or("Could not find config directory")?
        .join("matugen");
    fs::create_dir_all(&config_dir).map_err(|e| format!("Failed to create config dir: {}", e))?;
    Ok(config_dir.join("gui-presets.json"))
}

fn now_iso8601() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let epoch_secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let sec = epoch_secs % 60;
    let min = (epoch_secs / 60) % 60;
    let hour = (epoch_secs / 3600) % 24;

    let mut remaining_days = epoch_secs / 86400;
    let mut year = 1970u32;
    loop {
        let is_leap = (year % 4 == 0) && (year % 100 != 0 || year % 400 == 0);
        let days_in_year: u64 = if is_leap { 366 } else { 365 };
        if remaining_days < days_in_year {
            break;
        }
        remaining_days -= days_in_year;
        year += 1;
    }
    let is_leap = (year % 4 == 0) && (year % 100 != 0 || year % 400 == 0);
    let days_in_month: [u64; 12] = [
        31,
        if is_leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let mut month = 0usize;
    while month < 11 && remaining_days >= days_in_month[month] {
        remaining_days -= days_in_month[month];
        month += 1;
    }
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        year,
        month + 1,
        remaining_days + 1,
        hour,
        min,
        sec
    )
}

fn sha256_of(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}

fn mime_type_from_filename(filename: &str) -> String {
    let lower = filename.to_lowercase();
    if lower.ends_with(".png") {
        "image/png".to_string()
    } else if lower.ends_with(".jpg") || lower.ends_with(".jpeg") {
        "image/jpeg".to_string()
    } else if lower.ends_with(".webp") {
        "image/webp".to_string()
    } else if lower.ends_with(".jxl") {
        "image/jxl".to_string()
    } else {
        "application/octet-stream".to_string()
    }
}

fn expand_tilde_str(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest);
        }
    }
    PathBuf::from(path)
}

/// Read currently installed templates from `config.toml`, capturing their metadata and content.
fn read_installed_templates_snapshot() -> Vec<TemplateSnapshot> {
    let config_dir = match dirs::config_dir() {
        Some(d) => d.join("matugen"),
        None => return vec![],
    };
    let config_path = config_dir.join("config.toml");
    if !config_path.exists() {
        return vec![];
    }

    let content = match fs::read_to_string(&config_path) {
        Ok(c) => c,
        Err(_) => return vec![],
    };

    let config: toml::Value = match toml::from_str(&content) {
        Ok(v) => v,
        Err(_) => return vec![],
    };

    let templates_table = match config.get("templates").and_then(|t| t.as_table()) {
        Some(t) => t,
        None => return vec![],
    };

    let templates_dir = config_dir.join("templates");
    let mut snapshots = Vec::new();

    for (key, val) in templates_table {
        let input_path = val
            .get("input_path")
            .and_then(|v| v.as_str())
            .map(String::from);
        let output_path = val
            .get("output_path")
            .and_then(|v| v.as_str())
            .map(String::from);
        let post_hook = val
            .get("post_hook")
            .and_then(|v| v.as_str())
            .map(String::from);

        // Derive a friendly filename: prefer actual file name from input_path.
        let template_filename = input_path
            .as_deref()
            .and_then(|p| {
                PathBuf::from(p)
                    .file_name()
                    .map(|f| f.to_string_lossy().to_string())
            })
            .unwrap_or_else(|| key.replace('_', "."));

        // Try to read template content.
        let content_str = {
            let in_templates_dir = templates_dir.join(&template_filename);
            if in_templates_dir.exists() {
                fs::read_to_string(&in_templates_dir).ok()
            } else if let Some(ref ip) = input_path {
                fs::read_to_string(expand_tilde_str(ip)).ok()
            } else {
                None
            }
        };

        let target_app = key.split('_').next().map(String::from);

        snapshots.push(TemplateSnapshot {
            name: template_filename,
            target_app,
            enabled: true,
            input_path,
            output_path,
            template_type: None,
            content: content_str,
            pre_hook: None,
            post_hook,
        });
    }

    snapshots
}

/// Load all presets from disk, transparently migrating legacy formats.
fn load_presets() -> Result<Vec<PresetV2>, String> {
    let path = get_presets_file_path()?;
    if !path.exists() {
        return Ok(vec![]);
    }

    let content =
        fs::read_to_string(&path).map_err(|e| format!("Failed to read presets: {}", e))?;
    if content.trim().is_empty() {
        return Ok(vec![]);
    }

    // Peek at the raw JSON to decide which format to deserialise.
    let raw: Vec<serde_json::Value> = serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse presets JSON: {}", e))?;

    if raw.is_empty() {
        return Ok(vec![]);
    }
    let first_version = raw[0]
        .get("version")
        .and_then(|value| value.as_u64())
        .unwrap_or(1);
    if first_version >= 2 {
        raw.into_iter()
            .enumerate()
            .map(|(index, value)| {
                serde_json::from_value(value)
                    .map_err(|error| format!("Preset {} is invalid: {}", index + 1, error))
            })
            .collect()
    } else {
        // Legacy local format — migrate and persist.
        let legacy = raw
            .into_iter()
            .enumerate()
            .map(|(index, value)| {
                serde_json::from_value(value)
                    .map_err(|error| format!("Legacy preset {} is invalid: {}", index + 1, error))
            })
            .collect::<Result<Vec<LegacyLocalPreset>, String>>()?;
        let migrated: Vec<PresetV2> = legacy.into_iter().map(migrate_legacy_local_to_v2).collect();
        write_presets(&migrated)?;
        Ok(migrated)
    }
}

fn write_presets(presets: &[PresetV2]) -> Result<(), String> {
    let path = get_presets_file_path()?;
    let temp_path = path.with_extension("json.tmp");
    let content = serde_json::to_string_pretty(presets)
        .map_err(|e| format!("Failed to serialize presets: {}", e))?;
    fs::write(&temp_path, content).map_err(|e| format!("Failed to write presets: {}", e))?;
    fs::rename(&temp_path, &path).map_err(|e| {
        let _ = fs::remove_file(&temp_path);
        format!("Failed to commit presets: {}", e)
    })
}

fn validate_imported_filename(value: &str, label: &str) -> Result<String, String> {
    let path = Path::new(value);
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| format!("Invalid {} filename", label))?;

    if value.contains("..") || value.contains('/') || value.contains('\\') || file_name != value {
        return Err(format!(
            "Invalid {} filename '{}': directory components are not allowed",
            label, value
        ));
    }

    Ok(file_name.to_string())
}

// ── Migration ─────────────────────────────────────────────────────────────────

fn migrate_legacy_local_to_v2(old: LegacyLocalPreset) -> PresetV2 {
    let wallpaper = old.wallpaper_path.as_ref().map(|p| {
        let path = PathBuf::from(p);
        let filename = path.file_name().map(|f| f.to_string_lossy().to_string());
        let mime = filename.as_deref().map(mime_type_from_filename);
        WallpaperInfo {
            filename,
            original_path: Some(p.clone()),
            base64: None,
            mime_type: mime,
            sha256: None,
        }
    });

    let source_color = old
        .scheme_data
        .get("source_color_hex")
        .and_then(|v| v.as_str())
        .map(String::from);

    PresetV2 {
        version: 2,
        name: old.name,
        created_at: None,
        updated_at: None,
        app_version: None,
        wallpaper,
        scheme: SchemeInfo {
            scheme_type: old.scheme_type,
            mode: "dark".to_string(),
            generation: GenerationSettings::default(),
            opacity: None,
            source_color_hex: source_color,
            scheme_data: old.scheme_data,
        },
        custom_colors: vec![],
        templates: vec![],
        desktop: DesktopSettings::default(),
    }
}

fn migrate_export_v1_to_v2(old: ExportablePresetV1) -> PresetV2 {
    let wallpaper = if old.wallpaper_base64.is_some() || old.wallpaper_filename.is_some() {
        Some(WallpaperInfo {
            filename: old.wallpaper_filename.clone(),
            original_path: None,
            base64: old.wallpaper_base64,
            mime_type: old
                .wallpaper_filename
                .as_deref()
                .map(mime_type_from_filename),
            sha256: None,
        })
    } else {
        None
    };

    let templates = old
        .installed_templates
        .unwrap_or_default()
        .into_iter()
        .map(|name| {
            let target_app = name.split('_').next().map(String::from);
            TemplateSnapshot {
                name,
                target_app,
                enabled: true,
                input_path: None,
                output_path: None,
                template_type: None,
                content: None,
                pre_hook: None,
                post_hook: None,
            }
        })
        .collect();

    let source_color = old
        .scheme_data
        .get("source_color_hex")
        .and_then(|v| v.as_str())
        .map(String::from);

    PresetV2 {
        version: 2,
        name: old.name,
        created_at: None,
        updated_at: None,
        app_version: None,
        wallpaper,
        scheme: SchemeInfo {
            scheme_type: old.scheme_type,
            mode: "dark".to_string(),
            generation: GenerationSettings::default(),
            opacity: None,
            source_color_hex: source_color,
            scheme_data: old.scheme_data,
        },
        custom_colors: vec![],
        templates,
        desktop: DesktopSettings::default(),
    }
}

// ── Tauri commands ─────────────────────────────────────────────────────────────

#[tauri::command]
pub fn save_preset(input: SavePresetInput) -> Result<(), String> {
    let _guard = PRESET_STORAGE_LOCK.lock().map_err(|e| e.to_string())?;
    input.generation.validate()?;
    if let Some(desktop) = &input.desktop {
        desktop.integrations.validate()?;
    }
    let now = now_iso8601();

    let wallpaper = input.wallpaper_path.as_ref().map(|p| {
        let path = PathBuf::from(p);
        let filename = path.file_name().map(|f| f.to_string_lossy().to_string());
        let mime = filename.as_deref().map(mime_type_from_filename);
        WallpaperInfo {
            filename,
            original_path: Some(p.clone()),
            base64: None,
            mime_type: mime,
            sha256: None,
        }
    });

    let source_color = input.source_color_hex.clone().or_else(|| {
        input
            .scheme_data
            .get("source_color_hex")
            .and_then(|v| v.as_str())
            .map(String::from)
    });

    let templates = read_installed_templates_snapshot();

    let preset = PresetV2 {
        version: 2,
        name: input.name.clone(),
        created_at: Some(now.clone()),
        updated_at: Some(now),
        app_version: Some(env!("CARGO_PKG_VERSION").to_string()),
        wallpaper,
        scheme: SchemeInfo {
            scheme_type: input.scheme_type,
            mode: input.mode,
            generation: input.generation,
            opacity: input.opacity,
            source_color_hex: source_color,
            scheme_data: input.scheme_data,
        },
        custom_colors: input.custom_colors.unwrap_or_default(),
        templates,
        desktop: input.desktop.unwrap_or_default(),
    };

    let mut presets = load_presets()?;
    if let Some(pos) = presets.iter().position(|p| p.name == preset.name) {
        presets[pos] = preset;
    } else {
        presets.push(preset);
    }
    write_presets(&presets)
}

#[tauri::command]
pub fn get_presets(app: tauri::AppHandle) -> Result<Vec<PresetV2>, String> {
    let presets = load_presets()?;
    for preset in &presets {
        if let Some(path) = preset
            .wallpaper
            .as_ref()
            .and_then(|wallpaper| wallpaper.original_path.as_ref())
        {
            if image::image_dimensions(path).is_ok() {
                app.asset_protocol_scope()
                    .allow_file(path)
                    .map_err(|e| e.to_string())?;
            }
        }
    }
    Ok(presets)
}

#[tauri::command]
pub fn delete_preset(name: String) -> Result<(), String> {
    let _guard = PRESET_STORAGE_LOCK.lock().map_err(|e| e.to_string())?;
    let mut presets = load_presets()?;
    presets.retain(|p| p.name != name);
    write_presets(&presets)
}

/// Export a locally-saved preset to a self-contained `.matugen` v2 file,
/// embedding the wallpaper as base64.
#[tauri::command]
pub fn export_preset(name: String, output_path: String) -> Result<(), String> {
    if !output_path.ends_with(".matugen") {
        return Err("Output file must have .matugen extension".to_string());
    }

    let presets = load_presets()?;
    let preset = presets
        .iter()
        .find(|p| p.name == name)
        .ok_or_else(|| format!("Preset '{}' not found", name))?
        .clone();

    // Build the export wallpaper object with embedded base64.
    let export_wallpaper = match &preset.wallpaper {
        None => None,
        Some(wp) => {
            // Try original_path first, then shared-wallpapers fallback.
            let resolved = wp
                .original_path
                .as_deref()
                .map(PathBuf::from)
                .filter(|p| p.exists())
                .or_else(|| {
                    wp.filename
                        .as_deref()
                        .and_then(|fname| {
                            dirs::config_dir()
                                .map(|d| d.join("matugen").join("shared-wallpapers").join(fname))
                        })
                        .filter(|p| p.exists())
                });

            match resolved {
                Some(path) => {
                    let data = fs::read(&path)
                        .map_err(|e| format!("Failed to read wallpaper for export: {}", e))?;
                    let hash = sha256_of(&data);
                    let encoded = general_purpose::STANDARD.encode(&data);
                    let filename = path
                        .file_name()
                        .map(|f| f.to_string_lossy().to_string())
                        .or_else(|| wp.filename.clone());
                    let mime = filename
                        .as_deref()
                        .map(mime_type_from_filename)
                        .or_else(|| wp.mime_type.clone());
                    Some(WallpaperInfo {
                        filename,
                        original_path: wp.original_path.clone(),
                        base64: Some(encoded),
                        mime_type: mime,
                        sha256: Some(hash),
                    })
                }
                None => Some(wp.clone()), // Include metadata even if file not found.
            }
        }
    };

    let exportable = PresetV2 {
        wallpaper: export_wallpaper,
        ..preset
    };

    let json = serde_json::to_string_pretty(&exportable)
        .map_err(|e| format!("Failed to serialize preset: {}", e))?;
    fs::write(&output_path, json).map_err(|e| format!("Failed to write export file: {}", e))?;

    Ok(())
}

/// Import a `.matugen` file (v1 or v2), extract embedded assets, and save locally.
/// Returns the saved preset plus any non-fatal warnings.
#[tauri::command]
pub fn import_preset(file_path: String) -> Result<ImportResult, String> {
    let _guard = PRESET_STORAGE_LOCK.lock().map_err(|e| e.to_string())?;
    if !file_path.ends_with(".matugen") {
        return Err("File must have .matugen extension".to_string());
    }

    let content =
        fs::read_to_string(&file_path).map_err(|e| format!("Failed to read file: {}", e))?;

    let raw: serde_json::Value =
        serde_json::from_str(&content).map_err(|e| format!("Invalid .matugen file: {}", e))?;

    let file_version = raw.get("version").and_then(|v| v.as_u64()).unwrap_or(1);

    let mut preset = if file_version >= 2 {
        serde_json::from_value::<PresetV2>(raw)
            .map_err(|e| format!("Failed to parse v2 preset: {}", e))?
    } else {
        let v1: ExportablePresetV1 = serde_json::from_str(&content)
            .map_err(|e| format!("Failed to parse v1 preset: {}", e))?;
        migrate_export_v1_to_v2(v1)
    };

    preset.scheme.generation.validate()?;
    preset.desktop.integrations.validate()?;
    let mut warnings: Vec<String> = Vec::new();

    // Extract embedded wallpaper to shared-wallpapers/.
    if let Some(wp) = &mut preset.wallpaper {
        if let Some(b64) = wp.base64.take() {
            let wallpapers_dir = dirs::config_dir()
                .ok_or("Could not find config directory")?
                .join("matugen")
                .join("shared-wallpapers");
            fs::create_dir_all(&wallpapers_dir)
                .map_err(|e| format!("Failed to create wallpapers dir: {}", e))?;

            let raw_filename = wp
                .filename
                .clone()
                .unwrap_or_else(|| "wallpaper.png".to_string());
            let filename = validate_imported_filename(&raw_filename, "wallpaper")?;
            let dest = wallpapers_dir.join(&filename);

            match general_purpose::STANDARD.decode(&b64) {
                Ok(decoded) => {
                    if let Err(e) = fs::write(&dest, &decoded) {
                        warnings.push(format!("Could not save wallpaper: {}", e));
                    } else {
                        // Use the shared path when original is missing.
                        if wp
                            .original_path
                            .as_deref()
                            .map(|p| !PathBuf::from(p).exists())
                            .unwrap_or(true)
                        {
                            wp.original_path = Some(dest.to_string_lossy().to_string());
                        }
                    }
                }
                Err(e) => warnings.push(format!("Could not decode wallpaper: {}", e)),
            }
        } else if let Some(orig) = &wp.original_path {
            // No base64 but there is an original_path — check if it still exists.
            if !PathBuf::from(orig).exists() {
                warnings.push(format!(
                    "Wallpaper '{}' not found on disk. Re-select it manually.",
                    orig
                ));
            }
        }
    }

    // Restore embedded template files, but never trust an imported preset to
    // activate output paths or shell hooks. Activation remains an explicit
    // action in the Apps tab where the user chooses the destination and hook.
    let config_dir = dirs::config_dir()
        .ok_or("Could not find config directory")?
        .join("matugen");
    let templates_dir = config_dir.join("templates");

    for template in &preset.templates {
        if let Some(tmpl_content) = &template.content {
            fs::create_dir_all(&templates_dir).ok();
            let template_filename = validate_imported_filename(&template.name, "template")?;
            let dest = templates_dir.join(&template_filename);
            if !dest.exists() {
                if let Err(e) = fs::write(&dest, tmpl_content) {
                    warnings.push(format!(
                        "Could not restore template '{}': {}",
                        template.name, e
                    ));
                    continue;
                }
            }
            warnings.push(format!(
                "Template '{}' was restored but not activated. Review and install it from the Apps tab to choose its output path and optional hook.",
                template.name
            ));
        }
    }

    // Ensure the preset name is unique in local storage.
    let mut existing = load_presets()?;
    let base_name = preset.name.clone();
    let mut counter = 1u32;
    while existing.iter().any(|p| p.name == preset.name) {
        preset.name = format!("{} ({})", base_name, counter);
        counter += 1;
    }

    preset.version = 2;
    preset.updated_at = Some(now_iso8601());

    existing.push(preset.clone());
    write_presets(&existing)?;

    Ok(ImportResult { preset, warnings })
}

#[cfg(test)]
mod tests {
    use super::validate_imported_filename;

    #[test]
    fn imported_filenames_cannot_escape_their_target_directory() {
        assert!(validate_imported_filename("../../.bashrc", "wallpaper").is_err());
        assert!(validate_imported_filename("/tmp/payload", "template").is_err());
        assert!(validate_imported_filename("folder\\payload", "template").is_err());
        assert_eq!(
            validate_imported_filename("theme.css", "template").unwrap(),
            "theme.css"
        );
    }
}

#[cfg(test)]
mod advanced_tests {
    use super::super::settings::MaterialSpec;
    use super::*;
    fn old_preset() -> PresetV2 {
        serde_json::from_value(serde_json::json!({
            "version":2,"name":"legacy","created_at":null,"updated_at":null,"app_version":null,"wallpaper":null,
            "scheme":{"scheme_type":"Tinted Content","mode":"dark","contrast":null,"opacity":null,"source_color_hex":"#123456","scheme_data":{"colors":{}}},
            "custom_colors":[],"templates":[],"desktop":{"target_de":"KDE","apply_wallpaper":true,"apply_kde_colorscheme":true,"apply_templates":true}
        })).unwrap()
    }
    #[test]
    fn older_presets_keep_defaults_and_snapshots() {
        let old = old_preset();
        assert_eq!(old.scheme.generation, GenerationSettings::default());
        assert_eq!(old.scheme.source_color_hex.as_deref(), Some("#123456"));
        assert!(!old.desktop.integrations.klassy.enabled);
        let migrated = migrate_legacy_local_to_v2(LegacyLocalPreset {
            name: "old".into(),
            wallpaper_path: None,
            scheme_type: "Content".into(),
            scheme_data: serde_json::json!({"colors":{}}),
        });
        assert_eq!(migrated.scheme.generation.seed_index, 0);
    }
    #[test]
    fn new_settings_roundtrip_in_original_preset_format() {
        let mut preset = old_preset();
        preset.scheme.generation = GenerationSettings {
            seed_index: 2,
            contrast: 0.3,
            chroma: 1.4,
            tone: 0.8,
            material_spec: MaterialSpec::V2021,
        };
        preset.desktop.integrations.klassy.enabled = true;
        preset.desktop.integrations.klassy.titlebar_opacity = Some(83);
        let json = serde_json::to_value(&preset).unwrap();
        assert_eq!(json["scheme"]["contrast"], 0.3);
        let loaded: PresetV2 = serde_json::from_value(json).unwrap();
        assert_eq!(loaded.scheme.generation, preset.scheme.generation);
        assert_eq!(loaded.desktop.integrations, preset.desktop.integrations);
    }
    #[test]
    fn import_export_preserves_advanced_settings_on_disk() {
        // Run actual storage commands in a child test process with its own XDG root.
        // Never change the user's presets or mutate this process's environment.
        if std::env::var_os("STUDIO_PRESET_TEST_CHILD").is_some() {
            let mut preset = old_preset();
            preset.scheme.generation.seed_index = 2;
            preset.scheme.generation.chroma = 1.7;
            preset.desktop.integrations.rounded_corners.enabled = true;
            write_presets(&[preset.clone()]).unwrap();
            let file = dirs::config_dir()
                .unwrap()
                .join("roundtrip.matugen")
                .to_string_lossy()
                .to_string();
            export_preset(preset.name.clone(), file.clone()).unwrap();
            let imported = import_preset(file).unwrap().preset;
            assert_eq!(imported.scheme.generation, preset.scheme.generation);
            assert_eq!(imported.desktop.integrations, preset.desktop.integrations);
            assert_eq!(load_presets().unwrap().len(), 2);
            return;
        }
        let root = std::env::temp_dir().join(format!("studio-preset-test-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "commands::preset::advanced_tests::import_export_preserves_advanced_settings_on_disk", "--nocapture"])
            .env("STUDIO_PRESET_TEST_CHILD", "1").env("XDG_CONFIG_HOME", &root).status().unwrap();
        assert!(status.success());
        fs::remove_dir_all(root).unwrap();
    }
}
