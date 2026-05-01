use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use execute::{shell, Execute};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct KdeServiceStatus {
    pub enabled: bool,
    pub current_wallpaper: Option<String>,
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
    let surface_container_high = get_color_or(context, "surface_container_high", "surface_variant", v);
    let surface_container_highest = get_color_or(context, "surface_container_highest", "surface_variant", v);
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
pub fn apply_kde_colorscheme(context: serde_json::Value) -> Result<(), String> {
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

    // Spawn the plasma-apply commands in a background thread to avoid blocking UI
    let swap_path_clone = swap_path.clone();
    std::thread::spawn(move || {
        // Apply swap first to force plasma to notice the change
        let mut cmd1 = shell(&format!("plasma-apply-colorscheme {}", swap_name));
        cmd1.execute_output().ok();

        // Brief pause for plasma to register the change
        std::thread::sleep(std::time::Duration::from_millis(200));

        // Now apply the real scheme
        let mut cmd2 = shell(&format!("plasma-apply-colorscheme {}", scheme_name));
        cmd2.execute_output().ok();
        
        // Clean up swap file
        fs::remove_file(&swap_path_clone).ok();
    });

    Ok(())
}

/// Get the current KDE wallpaper path from Plasma config
#[tauri::command]
pub fn get_kde_current_wallpaper() -> Result<Option<String>, String> {
    // Try reading the current wallpaper from plasma config via qdbus
    let output = Command::new("qdbus6")
        .arg("org.kde.plasmashell")
        .arg("/PlasmaShell")
        .arg("org.kde.PlasmaShell.evaluateScript")
        .arg("var allDesktops = desktops(); var d = allDesktops[0]; d.currentConfigGroup = ['Wallpaper', 'org.kde.image', 'General']; print(d.readConfig('Image'));")
        .output();
    
    match output {
        Ok(o) if o.status.success() => {
            let path = String::from_utf8_lossy(&o.stdout).trim().to_string();
            let path = path.strip_prefix("file://").unwrap_or(&path).to_string();
            if path.is_empty() {
                Ok(None)
            } else {
                Ok(Some(path))
            }
        }
        _ => {
            // Fallback: try qdbus (without 6)
            let output2 = Command::new("qdbus")
                .arg("org.kde.plasmashell")
                .arg("/PlasmaShell")
                .arg("org.kde.PlasmaShell.evaluateScript")
                .arg("var allDesktops = desktops(); var d = allDesktops[0]; d.currentConfigGroup = ['Wallpaper', 'org.kde.image', 'General']; print(d.readConfig('Image'));")
                .output();

            match output2 {
                Ok(o) if o.status.success() => {
                    let path = String::from_utf8_lossy(&o.stdout).trim().to_string();
                    let path = path.strip_prefix("file://").unwrap_or(&path).to_string();
                    if path.is_empty() {
                        Ok(None)
                    } else {
                        Ok(Some(path))
                    }
                }
                _ => Ok(None),
            }
        }
    }
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
    let path = get_service_config_path()?;
    if path.exists() {
        let content = fs::read_to_string(&path).map_err(|e| e.to_string())?;
        serde_json::from_str(&content).map_err(|e| e.to_string())
    } else {
        Ok(KdeServiceStatus {
            enabled: false,
            current_wallpaper: None,
        })
    }
}

#[tauri::command]
pub fn set_kde_service_status(status: KdeServiceStatus) -> Result<(), String> {
    let path = get_service_config_path()?;
    let content = serde_json::to_string_pretty(&status).map_err(|e| e.to_string())?;
    fs::write(&path, content).map_err(|e| e.to_string())?;
    Ok(())
}
