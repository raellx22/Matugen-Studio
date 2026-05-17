use execute::{shell, Execute};
use percent_encoding::percent_decode_str;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;
use tauri::{async_runtime::Mutex, Emitter, Manager};
use tokio::time::sleep;
use zbus::zvariant::OwnedValue;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct KdeServiceStatus {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub run_in_background: bool,
    #[serde(default)]
    pub current_wallpaper: Option<String>,
    #[serde(default = "default_poll_interval_secs")]
    pub poll_interval_secs: u64,
    #[serde(default = "default_scheme_type")]
    pub scheme_type: String,
    #[serde(default)]
    pub gtk_enabled: bool,
    #[serde(default = "default_gtk_dark")]
    pub gtk_dark: bool,
}

impl Default for KdeServiceStatus {
    fn default() -> Self {
        Self {
            enabled: false,
            run_in_background: false,
            current_wallpaper: None,
            poll_interval_secs: default_poll_interval_secs(),
            scheme_type: default_scheme_type(),
            gtk_enabled: false,
            gtk_dark: default_gtk_dark(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct KdeWatcherOptions {
    pub poll_interval_secs: u64,
    pub scheme_type: String,
    #[serde(default)]
    pub gtk_enabled: bool,
    #[serde(default = "default_gtk_dark")]
    pub gtk_dark: bool,
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct KdeWatcherSnapshot {
    pub enabled: bool,
    pub run_in_background: bool,
    pub current_wallpaper: Option<String>,
    pub last_handled_wallpaper: Option<String>,
    pub is_processing: bool,
    pub poll_interval_secs: u64,
    pub scheme_type: String,
    pub gtk_enabled: bool,
    pub gtk_dark: bool,
    pub status_message: String,
    pub last_error: Option<String>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct KdeColorValue {
    key: String,
    hex: String,
}

struct KdeWatcherRuntime {
    handle: tauri::async_runtime::JoinHandle<()>,
}

#[derive(Default)]
pub struct KdeWallpaperWatcher {
    inner: Mutex<KdeWatcherInner>,
}

#[derive(Default)]
struct KdeWatcherInner {
    runtime: Option<KdeWatcherRuntime>,
    snapshot: KdeWatcherSnapshot,
}

impl Default for KdeWatcherSnapshot {
    fn default() -> Self {
        Self {
            enabled: false,
            run_in_background: false,
            current_wallpaper: None,
            last_handled_wallpaper: None,
            is_processing: false,
            poll_interval_secs: default_poll_interval_secs(),
            scheme_type: default_scheme_type(),
            gtk_enabled: false,
            gtk_dark: default_gtk_dark(),
            status_message: "Service stopped.".to_string(),
            last_error: None,
        }
    }
}

fn default_poll_interval_secs() -> u64 {
    10
}

fn default_scheme_type() -> String {
    "Tinted Smart".to_string()
}

fn default_gtk_dark() -> bool {
    true
}

/// Extract RGB values (0-255) from a hex color string like "#aabbcc"
fn hex_to_rgb(hex: &str) -> (u8, u8, u8) {
    let hex = hex.trim_start_matches('#');
    let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0);
    let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0);
    let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0);
    (r, g, b)
}

/// Format RGB tuple as KDE color string "r,g,b"
fn rgb_str(hex: &str) -> String {
    let (r, g, b) = hex_to_rgb(hex);
    format!("{},{},{}", r, g, b)
}

/// Get a hex color from the scheme context with fallback chain: requested → default → dark → light
/// The raw data uses "color" key (not "hex") with format #RRGGBBAA
fn get_color(context: &serde_json::Value, name: &str, variant: &str) -> String {
    let colors = match context.get("colors") {
        Some(c) => c,
        None => return "#000000".to_string(),
    };
    let group = match colors.get(name) {
        Some(g) => g,
        None => return "#000000".to_string(),
    };
    let variants = [variant, "default", "dark", "light"];
    // Try "color" key first (raw matugen data), then "hex" (after fixMatugenColors)
    let keys = ["color", "hex"];
    for v in variants {
        if let Some(variant_obj) = group.get(v) {
            for key in &keys {
                if let Some(val) = variant_obj.get(*key).and_then(|h| h.as_str()) {
                    if !val.is_empty() {
                        // Strip alpha from #RRGGBBAA → #RRGGBB
                        let hex = if val.len() == 9 && val.starts_with('#') {
                            format!("#{}", &val[1..7])
                        } else {
                            val.to_string()
                        };
                        return hex;
                    }
                }
            }
        }
    }
    "#000000".to_string()
}

/// Get a color with a fallback name if the primary name doesn't exist
fn get_color_or(context: &serde_json::Value, name: &str, fallback: &str, variant: &str) -> String {
    let result = get_color(context, name, variant);
    if result == "#000000" {
        get_color(context, fallback, variant)
    } else {
        result
    }
}

/// Generate a full KDE Plasma color scheme file from matugen colors
#[allow(unused_variables)]
fn generate_kde_colorscheme(context: &serde_json::Value, scheme_name: &str) -> String {
    let v = "default";

    let primary = get_color(context, "primary", v);
    let on_primary = get_color(context, "on_primary", v);
    let primary_container = get_color(context, "primary_container", v);
    let on_primary_container = get_color(context, "on_primary_container", v);

    let secondary = get_color(context, "secondary", v);
    let on_secondary = get_color(context, "on_secondary", v);
    let secondary_container = get_color(context, "secondary_container", v);
    let on_secondary_container = get_color(context, "on_secondary_container", v);

    let tertiary = get_color(context, "tertiary", v);
    let on_tertiary = get_color(context, "on_tertiary", v);
    let tertiary_container = get_color(context, "tertiary_container", v);

    let surface = get_color(context, "surface", v);
    let on_surface = get_color(context, "on_surface", v);
    let surface_variant = get_color(context, "surface_variant", v);
    let on_surface_variant = get_color(context, "on_surface_variant", v);

    let error = get_color(context, "error", v);
    let on_error = get_color(context, "on_error", v);
    let error_container = get_color(context, "error_container", v);

    let outline = get_color(context, "outline", v);
    let outline_variant = get_color(context, "outline_variant", v);

    let inverse_surface = get_color_or(context, "inverse_surface", "surface_variant", v);
    let inverse_on_surface = get_color_or(context, "inverse_on_surface", "on_surface", v);

    let surface_container = get_color_or(context, "surface_container", "surface", v);
    let surface_container_high =
        get_color_or(context, "surface_container_high", "surface_variant", v);
    let surface_container_highest =
        get_color_or(context, "surface_container_highest", "surface_variant", v);
    let surface_container_low = get_color_or(context, "surface_container_low", "surface", v);
    let surface_dim = get_color_or(context, "surface_dim", "surface", v);
    let surface_bright = get_color_or(context, "surface_bright", "surface_variant", v);

    let kde_link = get_color_or(context, "link", "primary", v);
    let kde_negative = get_color_or(context, "negative", "error", v);
    let kde_neutral = get_color_or(context, "neutral", "tertiary", v);
    let kde_positive = get_color_or(context, "positive", "secondary", v);
    let kde_visited = get_color_or(context, "visited", "secondary", v);

    let format_str = format!(
        r#"[ColorEffects:Disabled]
Color={on_surface_variant_rgb}
ColorAmount=0.55
ColorEffect=3
ContrastAmount=0.65
ContrastEffect=1
IntensityAmount=0.1
IntensityEffect=2

[ColorEffects:Inactive]
ChangeSelectionColor=true
Color={surface_rgb}
ColorAmount=0.025
ColorEffect=2
ContrastAmount=0.1
ContrastEffect=2
Enable=false
IntensityAmount=0
IntensityEffect=0

[Colors:Button]
BackgroundAlternate={surface_container_high_rgb}
BackgroundNormal={surface_container_rgb}
DecorationFocus={primary_rgb}
DecorationHover={primary_rgb}
ForegroundActive={primary_rgb}
ForegroundInactive={on_surface_variant_rgb}
ForegroundLink={primary_rgb}
ForegroundNegative={error_rgb}
ForegroundNeutral={tertiary_rgb}
ForegroundNormal={on_surface_rgb}
ForegroundPositive={primary_rgb}
ForegroundVisited={secondary_rgb}

[Colors:Complementary]
BackgroundAlternate={surface_container_high_rgb}
BackgroundNormal={surface_rgb}
DecorationFocus={primary_rgb}
DecorationHover={primary_rgb}
ForegroundActive={primary_rgb}
ForegroundInactive={on_surface_variant_rgb}
ForegroundLink={primary_rgb}
ForegroundNegative={error_rgb}
ForegroundNeutral={tertiary_rgb}
ForegroundNormal={on_surface_rgb}
ForegroundPositive={primary_rgb}
ForegroundVisited={secondary_rgb}

[Colors:Header]
BackgroundAlternate={surface_container_rgb}
BackgroundNormal={surface_rgb}
DecorationFocus={primary_rgb}
DecorationHover={primary_rgb}
ForegroundActive={primary_rgb}
ForegroundInactive={on_surface_variant_rgb}
ForegroundLink={primary_rgb}
ForegroundNegative={error_rgb}
ForegroundNeutral={tertiary_rgb}
ForegroundNormal={on_surface_rgb}
ForegroundPositive={primary_rgb}
ForegroundVisited={secondary_rgb}

[Colors:Header][Inactive]
BackgroundAlternate={surface_container_rgb}
BackgroundNormal={surface_rgb}
DecorationFocus={primary_rgb}
DecorationHover={primary_rgb}
ForegroundActive={primary_rgb}
ForegroundInactive={on_surface_variant_rgb}
ForegroundLink={primary_rgb}
ForegroundNegative={error_rgb}
ForegroundNeutral={tertiary_rgb}
ForegroundNormal={on_surface_rgb}
ForegroundPositive={primary_rgb}
ForegroundVisited={secondary_rgb}

[Colors:Selection]
BackgroundAlternate={primary_rgb}
BackgroundNormal={primary_rgb}
DecorationFocus={primary_rgb}
DecorationHover={primary_rgb}
ForegroundActive={on_primary_rgb}
ForegroundInactive={on_primary_rgb}
ForegroundLink={link_rgb}
ForegroundNegative={negative_rgb}
ForegroundNeutral={neutral_rgb}
ForegroundNormal={on_primary_rgb}
ForegroundPositive={positive_rgb}
ForegroundVisited={visited_rgb}

[Colors:Tooltip]
BackgroundAlternate={surface_container_highest_rgb}
BackgroundNormal={surface_container_high_rgb}
DecorationFocus={primary_rgb}
DecorationHover={primary_rgb}
ForegroundActive={primary_rgb}
ForegroundInactive={on_surface_variant_rgb}
ForegroundLink={primary_rgb}
ForegroundNegative={error_rgb}
ForegroundNeutral={tertiary_rgb}
ForegroundNormal={on_surface_rgb}
ForegroundPositive={primary_rgb}
ForegroundVisited={secondary_rgb}

[Colors:View]
BackgroundAlternate={surface_container_low_rgb}
BackgroundNormal={surface_rgb}
DecorationFocus={primary_rgb}
DecorationHover={primary_rgb}
ForegroundActive={primary_rgb}
ForegroundInactive={on_surface_variant_rgb}
ForegroundLink={primary_rgb}
ForegroundNegative={error_rgb}
ForegroundNeutral={tertiary_rgb}
ForegroundNormal={on_surface_rgb}
ForegroundPositive={primary_rgb}
ForegroundVisited={secondary_rgb}

[Colors:Window]
BackgroundAlternate={surface_container_rgb}
BackgroundNormal={surface_container_low_rgb}
DecorationFocus={primary_rgb}
DecorationHover={primary_rgb}
ForegroundActive={primary_rgb}
ForegroundInactive={on_surface_variant_rgb}
ForegroundLink={primary_rgb}
ForegroundNegative={error_rgb}
ForegroundNeutral={tertiary_rgb}
ForegroundNormal={on_surface_rgb}
ForegroundPositive={primary_rgb}
ForegroundVisited={secondary_rgb}

[General]
ColorScheme={scheme_name}
Name={scheme_name}
shadeSortColumn=true

[KDE]
contrast=4

[WM]
activeBackground={surface_container_rgb}
activeBlend={primary_rgb}
activeForeground={on_surface_rgb}
inactiveBackground={surface_container_low_rgb}
inactiveBlend={surface_dim_rgb}
inactiveForeground={on_surface_variant_rgb}
frame={surface_container_low_rgb}
"#,
        // Surface / base colors
        surface_rgb = rgb_str(&surface),
        surface_container_rgb = rgb_str(&surface_container),
        surface_container_high_rgb = rgb_str(&surface_container_high),
        surface_container_highest_rgb = rgb_str(&surface_container_highest),
        surface_container_low_rgb = rgb_str(&surface_container_low),
        surface_dim_rgb = rgb_str(&surface_dim),
        on_surface_rgb = rgb_str(&on_surface),
        on_surface_variant_rgb = rgb_str(&on_surface_variant),
        // Primary
        primary_rgb = rgb_str(&primary),
        on_primary_rgb = rgb_str(&on_primary),
        // Secondary
        secondary_rgb = rgb_str(&secondary),
        // Tertiary
        tertiary_rgb = rgb_str(&tertiary),
        // Error
        error_rgb = rgb_str(&error),
        // Scheme name
        scheme_name = scheme_name,
        // Fallbacks
        link_rgb = rgb_str(&kde_link),
        negative_rgb = rgb_str(&kde_negative),
        neutral_rgb = rgb_str(&kde_neutral),
        positive_rgb = rgb_str(&kde_positive),
        visited_rgb = rgb_str(&kde_visited),
    );

    format_str
}

#[tauri::command]
pub fn generate_kde_colorscheme_cmd(context: serde_json::Value) -> Result<String, String> {
    let scheme_name = "MatugenStudio";
    let content = generate_kde_colorscheme(&context, scheme_name);

    // Write to KDE color-schemes directory
    let color_schemes_dir = dirs::home_dir()
        .ok_or("Could not find home directory")?
        .join(".local/share/color-schemes");

    fs::create_dir_all(&color_schemes_dir).map_err(|e| e.to_string())?;

    let file_path = color_schemes_dir.join(format!("{}.colors", scheme_name));
    fs::write(&file_path, &content).map_err(|e| e.to_string())?;

    Ok(file_path.to_string_lossy().to_string())
}

#[tauri::command]
pub async fn apply_kde_colorscheme(context: serde_json::Value) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || apply_kde_colorscheme_blocking(context))
        .await
        .map_err(|e| e.to_string())?
}

pub fn apply_kde_colorscheme_blocking(context: serde_json::Value) -> Result<(), String> {
    let color_schemes_dir = dirs::home_dir()
        .ok_or("Could not find home directory")?
        .join(".local/share/color-schemes");

    fs::create_dir_all(&color_schemes_dir).map_err(|e| e.to_string())?;

    // Generate both scheme files immediately
    let scheme_name = "MatugenStudio";
    let swap_name = "MatugenStudioSwap";

    let content = generate_kde_colorscheme(&context, scheme_name);
    let swap_content = generate_kde_colorscheme(&context, swap_name);

    let file_path = color_schemes_dir.join(format!("{}.colors", scheme_name));
    let swap_path = color_schemes_dir.join(format!("{}.colors", swap_name));

    fs::write(&file_path, &content).map_err(|e| e.to_string())?;
    fs::write(&swap_path, &swap_content).map_err(|e| e.to_string())?;

    let mut cmd1 = shell(&format!("plasma-apply-colorscheme {}", swap_name));
    let output1 = cmd1
        .execute_output()
        .map_err(|e| format!("Failed to run plasma-apply-colorscheme: {}", e))?;
    if !output1.status.success() {
        fs::remove_file(&swap_path).ok();
        return Err(String::from_utf8_lossy(&output1.stderr).to_string());
    }

    std::thread::sleep(Duration::from_millis(200));

    let mut cmd2 = shell(&format!("plasma-apply-colorscheme {}", scheme_name));
    let output2 = cmd2
        .execute_output()
        .map_err(|e| format!("Failed to run plasma-apply-colorscheme: {}", e))?;
    fs::remove_file(&swap_path).ok();

    if !output2.status.success() {
        return Err(String::from_utf8_lossy(&output2.stderr).to_string());
    }

    Ok(())
}

#[tauri::command]
pub fn get_kde_color_values() -> Result<Vec<KdeColorValue>, String> {
    let path = kde_colorscheme_path("MatugenStudio")?;
    if !path.exists() {
        return Ok(vec![]);
    }

    let content = fs::read_to_string(path).map_err(|e| e.to_string())?;
    let mappings = kde_color_mappings();
    let mut values = Vec::new();
    for mapping in mappings {
        if let Some(rgb) = read_ini_value(&content, mapping.section, mapping.property) {
            values.push(KdeColorValue {
                key: mapping.key.to_string(),
                hex: rgb_to_hex(&rgb),
            });
        }
    }
    Ok(values)
}

#[tauri::command]
pub fn apply_kde_color_values(values: Vec<KdeColorValue>) -> Result<(), String> {
    let path = kde_colorscheme_path("MatugenStudio")?;
    if !path.exists() {
        return Err("MatugenStudio.colors has not been generated yet.".to_string());
    }

    let mut content = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    for value in values {
        let Some(mapping) = kde_color_mappings()
            .into_iter()
            .find(|mapping| mapping.key == value.key)
        else {
            continue;
        };
        let rgb = hex_to_rgb_string(&value.hex)?;
        content = upsert_ini_value(&content, mapping.section, mapping.property, &rgb);
    }
    fs::write(&path, content).map_err(|e| e.to_string())?;
    apply_existing_kde_colorscheme()
}

struct KdeColorMapping {
    key: &'static str,
    section: &'static str,
    property: &'static str,
}

fn kde_color_mappings() -> Vec<KdeColorMapping> {
    vec![
        KdeColorMapping {
            key: "windowBackground",
            section: "Colors:Window",
            property: "BackgroundNormal",
        },
        KdeColorMapping {
            key: "viewBackground",
            section: "Colors:View",
            property: "BackgroundNormal",
        },
        KdeColorMapping {
            key: "button",
            section: "Colors:Button",
            property: "BackgroundNormal",
        },
        KdeColorMapping {
            key: "selection",
            section: "Colors:Selection",
            property: "BackgroundNormal",
        },
        KdeColorMapping {
            key: "foreground",
            section: "Colors:Window",
            property: "ForegroundNormal",
        },
        KdeColorMapping {
            key: "link",
            section: "Colors:Window",
            property: "ForegroundLink",
        },
        KdeColorMapping {
            key: "negative",
            section: "Colors:Window",
            property: "ForegroundNegative",
        },
        KdeColorMapping {
            key: "neutral",
            section: "Colors:Window",
            property: "ForegroundNeutral",
        },
        KdeColorMapping {
            key: "visited",
            section: "Colors:Window",
            property: "ForegroundVisited",
        },
    ]
}

fn kde_colorscheme_path(name: &str) -> Result<PathBuf, String> {
    Ok(dirs::home_dir()
        .ok_or("Could not find home directory")?
        .join(".local/share/color-schemes")
        .join(format!("{}.colors", name)))
}

fn read_ini_value(content: &str, section: &str, property: &str) -> Option<String> {
    let mut in_section = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            in_section = trimmed == format!("[{}]", section);
            continue;
        }
        if in_section {
            if let Some((key, value)) = trimmed.split_once('=') {
                if key == property {
                    return Some(value.to_string());
                }
            }
        }
    }
    None
}

fn upsert_ini_value(content: &str, section: &str, property: &str, value: &str) -> String {
    let mut output = Vec::new();
    let mut in_section = false;
    let mut wrote_property = false;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            if in_section && !wrote_property {
                output.push(format!("{}={}", property, value));
                wrote_property = true;
            }
            in_section = trimmed == format!("[{}]", section);
        }

        if in_section && trimmed.starts_with(&format!("{}=", property)) {
            output.push(format!("{}={}", property, value));
            wrote_property = true;
        } else {
            output.push(line.to_string());
        }
    }

    if in_section && !wrote_property {
        output.push(format!("{}={}", property, value));
    }

    output.join("\n") + "\n"
}

fn rgb_to_hex(rgb: &str) -> String {
    let parts = rgb
        .split(',')
        .filter_map(|part| part.trim().parse::<u8>().ok())
        .collect::<Vec<_>>();
    if parts.len() != 3 {
        return "#000000".to_string();
    }
    format!("#{:02X}{:02X}{:02X}", parts[0], parts[1], parts[2])
}

fn hex_to_rgb_string(hex: &str) -> Result<String, String> {
    let value = hex.trim();
    if value.len() != 7 || !value.starts_with('#') {
        return Err("Color must use #RRGGBB format".to_string());
    }
    let (r, g, b) = hex_to_rgb(value);
    Ok(format!("{},{},{}", r, g, b))
}

fn apply_existing_kde_colorscheme() -> Result<(), String> {
    let scheme_path = kde_colorscheme_path("MatugenStudio")?;
    let swap_path = kde_colorscheme_path("MatugenStudioSwap")?;
    let content = fs::read_to_string(&scheme_path).map_err(|e| e.to_string())?;
    let swap_content = content
        .replace("ColorScheme=MatugenStudio", "ColorScheme=MatugenStudioSwap")
        .replace("Name=MatugenStudio", "Name=MatugenStudioSwap");
    fs::write(&swap_path, swap_content).map_err(|e| e.to_string())?;

    let mut cmd1 = shell("plasma-apply-colorscheme MatugenStudioSwap");
    let output1 = cmd1
        .execute_output()
        .map_err(|e| format!("Failed to run plasma-apply-colorscheme: {}", e))?;
    if !output1.status.success() {
        fs::remove_file(&swap_path).ok();
        return Err(String::from_utf8_lossy(&output1.stderr).to_string());
    }

    std::thread::sleep(Duration::from_millis(120));

    let mut cmd2 = shell("plasma-apply-colorscheme MatugenStudio");
    let output2 = cmd2
        .execute_output()
        .map_err(|e| format!("Failed to run plasma-apply-colorscheme: {}", e))?;
    fs::remove_file(&swap_path).ok();

    if !output2.status.success() {
        return Err(String::from_utf8_lossy(&output2.stderr).to_string());
    }

    Ok(())
}

/// Get the current KDE wallpaper path from Plasma config
#[tauri::command]
pub async fn get_kde_current_wallpaper() -> Result<Option<String>, String> {
    get_current_kde_wallpaper().await
}

async fn get_current_kde_wallpaper() -> Result<Option<String>, String> {
    match get_current_kde_wallpaper_zbus(0).await {
        Ok(path) if path.is_some() => Ok(path),
        Ok(_) | Err(_) => tauri::async_runtime::spawn_blocking(get_current_kde_wallpaper_qdbus)
            .await
            .map_err(|e| e.to_string())?,
    }
}

async fn get_current_kde_wallpaper_zbus(screen: u32) -> Result<Option<String>, String> {
    let connection = zbus::Connection::session()
        .await
        .map_err(|e| e.to_string())?;
    let proxy = zbus::Proxy::new(
        &connection,
        "org.kde.plasmashell",
        "/PlasmaShell",
        "org.kde.PlasmaShell",
    )
    .await
    .map_err(|e| e.to_string())?;

    let message = proxy
        .call_method("wallpaper", &screen)
        .await
        .map_err(|e| e.to_string())?;
    let body = message.body();

    if let Ok(path) = body.deserialize::<String>() {
        return Ok(normalize_wallpaper_path(&path));
    }

    if let Ok(map) = body.deserialize::<HashMap<String, OwnedValue>>() {
        for key in ["Image", "WallpaperSource", "currentWallpaper", "path"] {
            if let Some(value) = map.get(key) {
                if let Ok(raw) = value.try_clone().and_then(String::try_from) {
                    if let Some(path) = normalize_wallpaper_path(&raw) {
                        return Ok(Some(path));
                    }
                }
            }
        }
    }

    let debug_body = format!("{:?}", body);
    Ok(normalize_wallpaper_path(&debug_body))
}

fn get_current_kde_wallpaper_qdbus() -> Result<Option<String>, String> {
    for program in ["qdbus6", "qdbus"] {
        if let Ok(output) = Command::new(program)
            .arg("org.kde.plasmashell")
            .arg("/PlasmaShell")
            .arg("org.kde.PlasmaShell.wallpaper")
            .arg("0")
            .output()
        {
            if output.status.success() {
                let raw = String::from_utf8_lossy(&output.stdout);
                if let Some(path) = normalize_wallpaper_path(&raw) {
                    return Ok(Some(path));
                }
            }
        }
    }

    let script = "var allDesktops = desktops(); var d = allDesktops[0]; d.currentConfigGroup = ['Wallpaper', d.wallpaperPlugin, 'General']; print(d.readConfig('Image'));";
    for program in ["qdbus6", "qdbus"] {
        if let Ok(output) = Command::new(program)
            .arg("org.kde.plasmashell")
            .arg("/PlasmaShell")
            .arg("org.kde.PlasmaShell.evaluateScript")
            .arg(script)
            .output()
        {
            if output.status.success() {
                let raw = String::from_utf8_lossy(&output.stdout);
                if let Some(path) = normalize_wallpaper_path(&raw) {
                    return Ok(Some(path));
                }
            }
        }
    }

    Ok(None)
}

fn normalize_wallpaper_path(raw: &str) -> Option<String> {
    let mut value = raw.trim().trim_matches('"').trim_matches('\'').to_string();
    if value.is_empty() || value == "null" || value == "undefined" {
        return None;
    }

    if let Some(index) = value.find("file:") {
        value = value[index..].to_string();
    }

    if let Some(rest) = value.strip_prefix("Image:") {
        value = rest.trim().to_string();
    }

    if let Some(index) = value.find('\n') {
        value = value[..index].trim().to_string();
    }

    if let Some(index) = value.find("+video") {
        value = value[..index].to_string();
    }

    let path = if let Some(rest) = value.strip_prefix("file://") {
        rest
    } else if let Some(rest) = value.strip_prefix("file:") {
        rest
    } else {
        value.as_str()
    };

    if path.is_empty() || !path.starts_with('/') {
        return None;
    }

    Some(percent_decode_str(path).decode_utf8_lossy().to_string())
}

#[tauri::command]
pub async fn start_kde_wallpaper_watcher(
    app: tauri::AppHandle,
    options: KdeWatcherOptions,
) -> Result<KdeWatcherSnapshot, String> {
    start_kde_wallpaper_watcher_inner(app, options, None).await
}

#[tauri::command]
pub async fn stop_kde_wallpaper_watcher(
    app: tauri::AppHandle,
) -> Result<KdeWatcherSnapshot, String> {
    let state = app.state::<KdeWallpaperWatcher>();
    let snapshot = {
        let mut inner = state.inner.lock().await;
        if let Some(runtime) = inner.runtime.take() {
            runtime.handle.abort();
        }

        inner.snapshot.enabled = false;
        inner.snapshot.is_processing = false;
        inner.snapshot.status_message = "Service stopped.".to_string();
        inner.snapshot.clone()
    };

    persist_service_status(&KdeServiceStatus {
        enabled: false,
        run_in_background: snapshot.run_in_background,
        current_wallpaper: snapshot.last_handled_wallpaper.clone(),
        poll_interval_secs: snapshot.poll_interval_secs,
        scheme_type: snapshot.scheme_type.clone(),
        gtk_enabled: snapshot.gtk_enabled,
        gtk_dark: snapshot.gtk_dark,
    })?;
    emit_watcher_snapshot(&app, &snapshot);
    Ok(snapshot)
}

#[tauri::command]
pub async fn get_kde_wallpaper_watcher_status(
    app: tauri::AppHandle,
) -> Result<KdeWatcherSnapshot, String> {
    let state = app.state::<KdeWallpaperWatcher>();
    let snapshot = {
        let inner = state.inner.lock().await;
        inner.snapshot.clone()
    };

    if snapshot.enabled || snapshot.status_message != "Service stopped." {
        return Ok(snapshot);
    }

    let persisted = read_service_status()?;
    let mut next = snapshot;
    next.poll_interval_secs = persisted.poll_interval_secs;
    next.scheme_type = persisted.scheme_type;
    next.run_in_background = persisted.run_in_background;
    next.last_handled_wallpaper = persisted.current_wallpaper;
    next.gtk_enabled = persisted.gtk_enabled;
    next.gtk_dark = persisted.gtk_dark;
    Ok(next)
}

#[tauri::command]
pub async fn mark_kde_wallpaper_handled(
    app: tauri::AppHandle,
    wallpaper_path: String,
) -> Result<KdeWatcherSnapshot, String> {
    update_watcher_snapshot(&app, |snapshot| {
        snapshot.current_wallpaper = Some(wallpaper_path.clone());
        snapshot.last_handled_wallpaper = Some(wallpaper_path.clone());
        snapshot.is_processing = false;
        snapshot.status_message = if snapshot.enabled {
            format!(
                "Wallpaper handled by Matugen Studio: {}.",
                wallpaper_file_name(&wallpaper_path)
            )
        } else {
            "Service stopped.".to_string()
        };
        snapshot.last_error = None;
    })
    .await;

    let snapshot = current_watcher_snapshot(&app).await;
    persist_service_status(&KdeServiceStatus {
        enabled: snapshot.enabled,
        run_in_background: snapshot.run_in_background,
        current_wallpaper: snapshot.last_handled_wallpaper.clone(),
        poll_interval_secs: snapshot.poll_interval_secs,
        scheme_type: snapshot.scheme_type.clone(),
        gtk_enabled: snapshot.gtk_enabled,
        gtk_dark: snapshot.gtk_dark,
    })?;
    Ok(snapshot)
}

pub fn restore_kde_wallpaper_watcher(app: tauri::AppHandle) {
    tauri::async_runtime::spawn(async move {
        let Ok(status) = read_service_status() else {
            return;
        };

        if status.enabled {
            let options = KdeWatcherOptions {
                poll_interval_secs: status.poll_interval_secs,
                scheme_type: status.scheme_type.clone(),
                gtk_enabled: status.gtk_enabled,
                gtk_dark: status.gtk_dark,
            };
            let _ = start_kde_wallpaper_watcher_inner(app, options, status.current_wallpaper).await;
        }
    });
}

#[tauri::command]
pub async fn set_kde_run_in_background(
    app: tauri::AppHandle,
    enabled: bool,
) -> Result<KdeWatcherSnapshot, String> {
    update_watcher_snapshot(&app, |snapshot| {
        snapshot.run_in_background = enabled;
    })
    .await;

    let snapshot = current_watcher_snapshot(&app).await;
    persist_service_status(&KdeServiceStatus {
        enabled: snapshot.enabled,
        run_in_background: snapshot.run_in_background,
        current_wallpaper: snapshot.last_handled_wallpaper.clone(),
        poll_interval_secs: snapshot.poll_interval_secs,
        scheme_type: snapshot.scheme_type.clone(),
        gtk_enabled: snapshot.gtk_enabled,
        gtk_dark: snapshot.gtk_dark,
    })?;
    Ok(snapshot)
}

pub fn should_run_in_background() -> bool {
    read_service_status()
        .map(|status| status.run_in_background)
        .unwrap_or(false)
}

async fn start_kde_wallpaper_watcher_inner(
    app: tauri::AppHandle,
    options: KdeWatcherOptions,
    last_handled_wallpaper: Option<String>,
) -> Result<KdeWatcherSnapshot, String> {
    let poll_interval_secs = options.poll_interval_secs.clamp(5, 3600);
    let scheme_type = if options.scheme_type.trim().is_empty() {
        default_scheme_type()
    } else {
        options.scheme_type
    };
    let gtk_enabled = options.gtk_enabled;
    let gtk_dark = options.gtk_dark;
    let last_handled_wallpaper = last_handled_wallpaper.or_else(|| {
        read_service_status()
            .ok()
            .and_then(|status| status.current_wallpaper)
    });
    let state = app.state::<KdeWallpaperWatcher>();

    let snapshot = {
        let mut inner = state.inner.lock().await;
        if let Some(runtime) = inner.runtime.take() {
            runtime.handle.abort();
        }

        inner.snapshot.enabled = true;
        inner.snapshot.run_in_background = should_run_in_background();
        inner.snapshot.is_processing = false;
        inner.snapshot.poll_interval_secs = poll_interval_secs;
        inner.snapshot.scheme_type = scheme_type.clone();
        inner.snapshot.gtk_enabled = gtk_enabled;
        inner.snapshot.gtk_dark = gtk_dark;
        inner.snapshot.last_handled_wallpaper = last_handled_wallpaper.clone();
        inner.snapshot.status_message = "Monitoring KDE wallpaper changes.".to_string();
        inner.snapshot.last_error = None;

        let app_for_task = app.clone();
        let handle = tauri::async_runtime::spawn(async move {
            kde_wallpaper_watcher_loop(
                app_for_task,
                poll_interval_secs,
                scheme_type,
                gtk_enabled,
                gtk_dark,
            )
            .await;
        });
        inner.runtime = Some(KdeWatcherRuntime { handle });
        inner.snapshot.clone()
    };

    persist_service_status(&KdeServiceStatus {
        enabled: true,
        run_in_background: snapshot.run_in_background,
        current_wallpaper: snapshot.last_handled_wallpaper.clone(),
        poll_interval_secs: snapshot.poll_interval_secs,
        scheme_type: snapshot.scheme_type.clone(),
        gtk_enabled: snapshot.gtk_enabled,
        gtk_dark: snapshot.gtk_dark,
    })?;
    emit_watcher_snapshot(&app, &snapshot);
    Ok(snapshot)
}

async fn kde_wallpaper_watcher_loop(
    app: tauri::AppHandle,
    poll_interval_secs: u64,
    scheme_type: String,
    gtk_enabled: bool,
    gtk_dark: bool,
) {
    loop {
        match get_current_kde_wallpaper().await {
            Ok(Some(wallpaper)) => {
                update_watcher_snapshot(&app, |snapshot| {
                    snapshot.current_wallpaper = Some(wallpaper.clone());
                    snapshot.last_error = None;
                })
                .await;

                if should_handle_wallpaper(&app, &wallpaper).await {
                    sleep(Duration::from_millis(600)).await;
                    let stable_wallpaper = match get_current_kde_wallpaper().await {
                        Ok(Some(path)) if path == wallpaper => path,
                        Ok(_) => {
                            sleep(Duration::from_secs(poll_interval_secs)).await;
                            continue;
                        }
                        Err(error) => {
                            record_watcher_error(&app, error).await;
                            sleep(Duration::from_secs(poll_interval_secs)).await;
                            continue;
                        }
                    };

                    process_wallpaper_change(
                        &app,
                        stable_wallpaper,
                        scheme_type.clone(),
                        gtk_enabled,
                        gtk_dark,
                    )
                    .await;
                }
            }
            Ok(None) => {
                update_watcher_snapshot(&app, |snapshot| {
                    snapshot.current_wallpaper = None;
                    snapshot.status_message = "KDE wallpaper not detected.".to_string();
                })
                .await;
            }
            Err(error) => {
                record_watcher_error(&app, error).await;
            }
        }

        sleep(Duration::from_secs(poll_interval_secs)).await;
    }
}

async fn should_handle_wallpaper(app: &tauri::AppHandle, wallpaper: &str) -> bool {
    let state = app.state::<KdeWallpaperWatcher>();
    let inner = state.inner.lock().await;
    inner.snapshot.enabled
        && !inner.snapshot.is_processing
        && inner.snapshot.last_handled_wallpaper.as_deref() != Some(wallpaper)
}

async fn process_wallpaper_change(
    app: &tauri::AppHandle,
    wallpaper: String,
    scheme_type: String,
    gtk_enabled: bool,
    gtk_dark: bool,
) {
    update_watcher_snapshot(app, |snapshot| {
        snapshot.is_processing = true;
        snapshot.status_message = format!(
            "Wallpaper changed. Generating theme from {}.",
            wallpaper_file_name(&wallpaper)
        );
        snapshot.last_error = None;
    })
    .await;

    let wallpaper_for_task = wallpaper.clone();
    let scheme_for_task = scheme_type.clone();
    let app_for_task = app.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        let context = crate::commands::color::generate_scheme_from_image_blocking(
            wallpaper_for_task,
            scheme_for_task,
        )?;
        crate::commands::template::apply_theme_blocking(context.clone())?;
        apply_kde_colorscheme_blocking(context.clone())?;
        if gtk_enabled {
            crate::commands::gtk::apply_gtk_theme_blocking(&app_for_task, &context, gtk_dark)?;
        }
        Ok::<(), String>(())
    })
    .await
    .map_err(|e| e.to_string())
    .and_then(|result| result);

    match result {
        Ok(()) => {
            update_watcher_snapshot(app, |snapshot| {
                snapshot.is_processing = false;
                snapshot.last_handled_wallpaper = Some(wallpaper.clone());
                snapshot.status_message =
                    format!("Theme applied from {}.", wallpaper_file_name(&wallpaper));
                snapshot.last_error = None;
            })
            .await;
        }
        Err(error) => {
            update_watcher_snapshot(app, |snapshot| {
                snapshot.is_processing = false;
                snapshot.last_handled_wallpaper = Some(wallpaper.clone());
                snapshot.status_message = format!(
                    "Theme generation failed for {}.",
                    wallpaper_file_name(&wallpaper)
                );
                snapshot.last_error = Some(error);
            })
            .await;
        }
    }

    let snapshot = current_watcher_snapshot(app).await;
    let _ = persist_service_status(&KdeServiceStatus {
        enabled: snapshot.enabled,
        run_in_background: snapshot.run_in_background,
        current_wallpaper: snapshot.last_handled_wallpaper,
        poll_interval_secs: snapshot.poll_interval_secs,
        scheme_type: snapshot.scheme_type,
        gtk_enabled: snapshot.gtk_enabled,
        gtk_dark: snapshot.gtk_dark,
    });
}

async fn record_watcher_error(app: &tauri::AppHandle, error: String) {
    update_watcher_snapshot(app, |snapshot| {
        snapshot.is_processing = false;
        snapshot.last_error = Some(error);
        snapshot.status_message = "Wallpaper monitor could not query KDE.".to_string();
    })
    .await;
}

async fn current_watcher_snapshot(app: &tauri::AppHandle) -> KdeWatcherSnapshot {
    let state = app.state::<KdeWallpaperWatcher>();
    let inner = state.inner.lock().await;
    inner.snapshot.clone()
}

async fn update_watcher_snapshot<F>(app: &tauri::AppHandle, update: F)
where
    F: FnOnce(&mut KdeWatcherSnapshot),
{
    let snapshot = {
        let state = app.state::<KdeWallpaperWatcher>();
        let mut inner = state.inner.lock().await;
        update(&mut inner.snapshot);
        inner.snapshot.clone()
    };

    emit_watcher_snapshot(app, &snapshot);
}

fn emit_watcher_snapshot(app: &tauri::AppHandle, snapshot: &KdeWatcherSnapshot) {
    let _ = app.emit("kde-wallpaper-watcher-status", snapshot.clone());
}

fn wallpaper_file_name(path: &str) -> String {
    PathBuf::from(path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(path)
        .to_string()
}

/// Get/Set service status from a config file
fn get_service_config_path() -> Result<PathBuf, String> {
    let config_dir = dirs::config_dir()
        .ok_or("Could not find config directory")?
        .join("matugen");
    fs::create_dir_all(&config_dir).map_err(|e| e.to_string())?;
    Ok(config_dir.join("service.json"))
}

#[tauri::command]
pub fn get_kde_service_status() -> Result<KdeServiceStatus, String> {
    read_service_status()
}

fn read_service_status() -> Result<KdeServiceStatus, String> {
    let path = get_service_config_path()?;
    if path.exists() {
        let content = fs::read_to_string(&path).map_err(|e| e.to_string())?;
        serde_json::from_str(&content).map_err(|e| e.to_string())
    } else {
        Ok(KdeServiceStatus::default())
    }
}

#[tauri::command]
pub fn set_kde_service_status(status: KdeServiceStatus) -> Result<(), String> {
    persist_service_status(&status)
}

fn persist_service_status(status: &KdeServiceStatus) -> Result<(), String> {
    let path = get_service_config_path()?;
    let content = serde_json::to_string_pretty(&status).map_err(|e| e.to_string())?;
    fs::write(&path, content).map_err(|e| e.to_string())?;
    Ok(())
}
