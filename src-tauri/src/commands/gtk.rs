use crate::commands::proc;
use regex::Regex;
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Duration;
use tauri::Manager;

/// Upper bound for the small `gsettings`/reload commands issued while
/// applying a GTK theme.
const GTK_HOOK_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GtkThemeResult {
    theme_name: String,
    theme_path: String,
    warnings: Vec<String>,
}

struct GtkPalette {
    primary: String,
    on_primary: String,
    secondary: String,
    on_secondary: String,
    tertiary: String,
    on_tertiary: String,
    error: String,
    on_error: String,
    surface: String,
    on_surface: String,
    surface_container: String,
    surface_container_high: String,
    surface_container_low: String,
}

#[tauri::command]
pub async fn apply_gtk_theme(
    app: tauri::AppHandle,
    context: serde_json::Value,
    dark: bool,
) -> Result<GtkThemeResult, String> {
    tauri::async_runtime::spawn_blocking(move || apply_gtk_theme_blocking(&app, &context, dark))
        .await
        .map_err(|e| e.to_string())?
}

pub fn apply_gtk_theme_blocking(
    app: &tauri::AppHandle,
    context: &serde_json::Value,
    dark: bool,
) -> Result<GtkThemeResult, String> {
    let source_name = if dark { "adw-gtk3-dark" } else { "adw-gtk3" };
    let theme_name = if dark {
        "Matugen-Adw-Dark"
    } else {
        "Matugen-Adw"
    };
    let source_dir = bundled_gtk_theme_dir(app)?.join(source_name);
    if !source_dir.exists() {
        return Err(format!(
            "Bundled GTK theme base not found at {}",
            source_dir.display()
        ));
    }

    let themes_dir = dirs::home_dir()
        .ok_or("Could not find home directory")?
        .join(".local/share/themes");
    let target_dir = themes_dir.join(theme_name);
    if target_dir.exists() {
        fs::remove_dir_all(&target_dir).map_err(|e| e.to_string())?;
    }
    copy_dir_all(&source_dir, &target_dir)?;

    write_index_theme(&target_dir, theme_name, dark)?;
    recolor_theme_css(&target_dir, &gtk_palette(context)?)?;
    install_gtk4_user_css(&target_dir)?;
    let warnings = apply_gtk_settings(theme_name, dark)?;

    Ok(GtkThemeResult {
        theme_name: theme_name.to_string(),
        theme_path: target_dir.to_string_lossy().to_string(),
        warnings,
    })
}

fn bundled_gtk_theme_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let resource_path = app
        .path()
        .resolve("resources/gtk-themes", tauri::path::BaseDirectory::Resource)
        .map_err(|e| format!("Failed to resolve bundled GTK theme path: {}", e))?;

    if resource_path.exists() {
        return Ok(resource_path);
    }

    let dev_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("resources")
        .join("gtk-themes");

    if dev_path.exists() {
        return Ok(dev_path);
    }

    Err(format!(
        "Bundled GTK theme directory not found at {}",
        resource_path.display()
    ))
}

fn copy_dir_all(source: &Path, target: &Path) -> Result<(), String> {
    fs::create_dir_all(target).map_err(|e| e.to_string())?;
    for entry in fs::read_dir(source).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let source_path = entry.path();
        let target_path = target.join(entry.file_name());
        if source_path.is_dir() {
            copy_dir_all(&source_path, &target_path)?;
        } else {
            fs::copy(&source_path, &target_path).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

fn write_index_theme(target_dir: &Path, theme_name: &str, dark: bool) -> Result<(), String> {
    let comment = if dark {
        "Matugen generated dark adw-gtk3 theme"
    } else {
        "Matugen generated light adw-gtk3 theme"
    };
    let content = format!(
        "[X-GNOME-Metatheme]\nName={theme_name}\nType=X-GNOME-Metatheme\nComment={comment}\nEncoding=UTF-8\nGtkTheme={theme_name}\n"
    );
    fs::write(target_dir.join("index.theme"), content).map_err(|e| e.to_string())
}

fn recolor_theme_css(target_dir: &Path, palette: &GtkPalette) -> Result<(), String> {
    for entry in walk_files(target_dir)? {
        if entry.extension().and_then(|ext| ext.to_str()) != Some("css") {
            continue;
        }
        let content = match fs::read_to_string(&entry) {
            Ok(content) => content,
            Err(_) => continue,
        };
        let recolored = rewrite_css_colors(&content, palette);
        fs::write(entry, recolored).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn walk_files(dir: &Path) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();
    for entry in fs::read_dir(dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if path.is_dir() {
            files.extend(walk_files(&path)?);
        } else {
            files.push(path);
        }
    }
    Ok(files)
}

fn rewrite_css_colors(content: &str, palette: &GtkPalette) -> String {
    let define_map = [
        ("accent_bg_color", palette.primary.as_str()),
        ("accent_fg_color", palette.on_primary.as_str()),
        ("destructive_bg_color", palette.error.as_str()),
        ("destructive_fg_color", palette.on_error.as_str()),
        ("success_bg_color", palette.secondary.as_str()),
        ("success_fg_color", palette.on_secondary.as_str()),
        ("warning_bg_color", palette.tertiary.as_str()),
        ("warning_fg_color", palette.on_tertiary.as_str()),
        ("error_bg_color", palette.error.as_str()),
        ("error_fg_color", palette.on_error.as_str()),
        ("window_bg_color", palette.surface.as_str()),
        ("window_fg_color", palette.on_surface.as_str()),
        ("view_bg_color", palette.surface_container_low.as_str()),
        ("view_fg_color", palette.on_surface.as_str()),
        ("headerbar_bg_color", palette.surface_container.as_str()),
        ("headerbar_fg_color", palette.on_surface.as_str()),
        ("headerbar_backdrop_color", palette.surface.as_str()),
        ("sidebar_bg_color", palette.surface_container.as_str()),
        ("sidebar_fg_color", palette.on_surface.as_str()),
        ("sidebar_backdrop_color", palette.surface.as_str()),
        (
            "secondary_sidebar_bg_color",
            palette.surface_container.as_str(),
        ),
        ("secondary_sidebar_fg_color", palette.on_surface.as_str()),
        ("secondary_sidebar_backdrop_color", palette.surface.as_str()),
        ("card_bg_color", palette.surface_container_high.as_str()),
        ("card_fg_color", palette.on_surface.as_str()),
        ("dialog_bg_color", palette.surface_container_high.as_str()),
        ("dialog_fg_color", palette.on_surface.as_str()),
        ("popover_bg_color", palette.surface_container_high.as_str()),
        ("popover_fg_color", palette.on_surface.as_str()),
        (
            "thumbnail_bg_color",
            palette.surface_container_high.as_str(),
        ),
        ("thumbnail_fg_color", palette.on_surface.as_str()),
        ("panel_bg_color", palette.surface.as_str()),
        ("panel_fg_color", palette.on_surface.as_str()),
    ];

    let mut output = Vec::new();
    for line in content.lines() {
        let trimmed = line.trim_start();
        if let Some(rest) = trimmed.strip_prefix("@define-color ") {
            let name = rest.split_whitespace().next().unwrap_or_default();
            if let Some((_, hex)) = define_map.iter().find(|(key, _)| *key == name) {
                let indent_len = line.len() - trimmed.len();
                output.push(format!(
                    "{}@define-color {} {};",
                    &line[..indent_len],
                    name,
                    hex
                ));
                continue;
            }
        }
        output.push(line.to_string());
    }

    let mut content = output.join("\n");
    let css_vars = [
        ("accent-blue", palette.primary.as_str()),
        ("accent-bg-color", palette.primary.as_str()),
        ("accent-fg-color", palette.on_primary.as_str()),
        ("destructive-bg-color", palette.error.as_str()),
        ("destructive-fg-color", palette.on_error.as_str()),
        ("success-bg-color", palette.secondary.as_str()),
        ("success-fg-color", palette.on_secondary.as_str()),
        ("warning-bg-color", palette.tertiary.as_str()),
        ("warning-fg-color", palette.on_tertiary.as_str()),
        ("error-bg-color", palette.error.as_str()),
        ("error-fg-color", palette.on_error.as_str()),
        ("window-bg-color", palette.surface.as_str()),
        ("window-fg-color", palette.on_surface.as_str()),
        ("view-bg-color", palette.surface_container_low.as_str()),
        ("view-fg-color", palette.on_surface.as_str()),
        ("headerbar-bg-color", palette.surface_container.as_str()),
        ("headerbar-fg-color", palette.on_surface.as_str()),
        ("sidebar-bg-color", palette.surface_container.as_str()),
        ("sidebar-fg-color", palette.on_surface.as_str()),
        ("card-bg-color", palette.surface_container_high.as_str()),
        ("card-fg-color", palette.on_surface.as_str()),
        ("dialog-bg-color", palette.surface_container_high.as_str()),
        ("dialog-fg-color", palette.on_surface.as_str()),
        ("popover-bg-color", palette.surface_container_high.as_str()),
        ("popover-fg-color", palette.on_surface.as_str()),
    ];

    for (name, hex) in css_vars {
        content = css_var_regex(name)
            .replace_all(&content, format!("--{}: {};", name, hex))
            .to_string();
    }

    content
}

fn css_var_regex(name: &str) -> Regex {
    Regex::new(&format!(
        r"--{}:\s*(?:#[0-9A-Fa-f]{{6}}|@[A-Za-z0-9_-]+|var\([^)]+\)|RGB\([^)]+\)|rgb\([^)]+\));",
        regex::escape(name)
    ))
    .unwrap()
}

fn gtk_palette(context: &serde_json::Value) -> Result<GtkPalette, String> {
    Ok(GtkPalette {
        primary: required_color(context, "primary", "default")?,
        on_primary: required_color(context, "on_primary", "default")?,
        secondary: required_color(context, "secondary", "default")?,
        on_secondary: required_color(context, "on_secondary", "default")?,
        tertiary: required_color(context, "tertiary", "default")?,
        on_tertiary: required_color(context, "on_tertiary", "default")?,
        error: required_color(context, "error", "default")?,
        on_error: required_color(context, "on_error", "default")?,
        surface: required_color(context, "surface", "default")?,
        on_surface: required_color(context, "on_surface", "default")?,
        surface_container: required_color_or(context, "surface_container", "surface", "default")?,
        surface_container_high: required_color_or(
            context,
            "surface_container_high",
            "surface_variant",
            "default",
        )?,
        surface_container_low: required_color_or(
            context,
            "surface_container_low",
            "surface",
            "default",
        )?,
    })
}

fn get_color(context: &serde_json::Value, name: &str, variant: &str) -> Option<String> {
    let colors = context.get("colors")?;
    let group = colors.get(name)?;
    for variant_name in [variant, "default", "dark", "light"] {
        if let Some(color) = group.get(variant_name) {
            for key in ["hex", "color"] {
                if let Some(value) = color.get(key).and_then(|value| value.as_str()) {
                    if let Some(hex) = normalize_hex(value) {
                        return Some(hex);
                    }
                }
            }
        }
    }
    None
}

fn required_color(context: &serde_json::Value, name: &str, variant: &str) -> Result<String, String> {
    get_color(context, name, variant)
        .ok_or_else(|| format!("Required GTK color '{}' is missing", name))
}

fn required_color_or(
    context: &serde_json::Value,
    name: &str,
    fallback: &str,
    variant: &str,
) -> Result<String, String> {
    get_color(context, name, variant)
        .or_else(|| get_color(context, fallback, variant))
        .ok_or_else(|| format!("Required GTK colors '{}' and '{}' are missing", name, fallback))
}

fn normalize_hex(value: &str) -> Option<String> {
    static HEX_RE: OnceLock<Regex> = OnceLock::new();
    let value = value.trim();
    let re = HEX_RE.get_or_init(|| Regex::new(r"(?i)^#[0-9a-f]{6}([0-9a-f]{2})?$").unwrap());
    if !re.is_match(value) {
        return None;
    }
    Some(format!("#{}", &value[1..7]).to_uppercase())
}

fn apply_gtk_settings(theme_name: &str, dark: bool) -> Result<Vec<String>, String> {
    let mut warnings = Vec::new();
    write_gtk_settings_ini(theme_name)?;

    if command_exists("gsettings") {
        run_optional_command(
            &format!(
                "gsettings set org.gnome.desktop.interface gtk-theme '{}'",
                theme_name
            ),
            &mut warnings,
        );
        run_optional_command(
            &format!(
                "gsettings set org.gnome.desktop.interface color-scheme '{}'",
                if dark { "prefer-dark" } else { "default" }
            ),
            &mut warnings,
        );
    } else {
        warnings.push("gsettings was not found; settings.ini was written instead.".to_string());
    }

    Ok(warnings)
}

fn write_gtk_settings_ini(theme_name: &str) -> Result<(), String> {
    let config_dir = dirs::config_dir().ok_or("Could not find config directory")?;
    for version in ["gtk-3.0", "gtk-4.0"] {
        let dir = config_dir.join(version);
        fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let path = dir.join("settings.ini");
        let mut content = if path.exists() {
            fs::read_to_string(&path).unwrap_or_else(|_| "[Settings]\n".to_string())
        } else {
            "[Settings]\n".to_string()
        };
        content = upsert_ini_setting(&content, "gtk-theme-name", theme_name);
        fs::write(path, content).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn install_gtk4_user_css(theme_dir: &Path) -> Result<(), String> {
    let source_dir = theme_dir.join("gtk-4.0");
    if !source_dir.exists() {
        return Ok(());
    }

    let config_dir = dirs::config_dir()
        .ok_or("Could not find config directory")?
        .join("gtk-4.0");
    fs::create_dir_all(&config_dir).map_err(|e| e.to_string())?;

    for entry in fs::read_dir(&source_dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let source_path = entry.path();
        let target_path = config_dir.join(entry.file_name());

        if source_path.is_dir() {
            if target_path.exists() {
                fs::remove_dir_all(&target_path).map_err(|e| e.to_string())?;
            }
            copy_dir_all(&source_path, &target_path)?;
        } else {
            backup_user_css_file(&target_path)?;
            fs::copy(&source_path, &target_path).map_err(|e| e.to_string())?;
            if target_path.extension().and_then(|ext| ext.to_str()) == Some("css") {
                prepend_generated_marker(&target_path)?;
            }
        }
    }

    Ok(())
}

fn backup_user_css_file(path: &Path) -> Result<(), String> {
    if !path.exists() {
        return Ok(());
    }

    let current = fs::read_to_string(path).unwrap_or_default();
    if current.contains("Generated by Matugen Studio") {
        return Ok(());
    }

    let backup = path.with_extension(format!(
        "{}.matugen-studio.bak",
        path.extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("bak")
    ));
    if !backup.exists() {
        fs::copy(path, backup).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn prepend_generated_marker(path: &Path) -> Result<(), String> {
    let content = fs::read_to_string(path).map_err(|e| e.to_string())?;
    if content.contains("Generated by Matugen Studio") {
        return Ok(());
    }
    fs::write(
        path,
        format!(
            "/* Generated by Matugen Studio. Previous user CSS is backed up next to this file. */\n{}",
            content
        ),
    )
    .map_err(|e| e.to_string())
}

fn upsert_ini_setting(content: &str, key: &str, value: &str) -> String {
    let mut output = Vec::new();
    let mut in_settings = false;
    let mut saw_settings = false;
    let mut wrote_key = false;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            if in_settings && !wrote_key {
                output.push(format!("{}={}", key, value));
                wrote_key = true;
            }
            in_settings = trimmed == "[Settings]";
            saw_settings |= in_settings;
        }

        if in_settings && trimmed.starts_with(&format!("{}=", key)) {
            output.push(format!("{}={}", key, value));
            wrote_key = true;
        } else {
            output.push(line.to_string());
        }
    }

    if !saw_settings {
        if !output.is_empty() {
            output.push(String::new());
        }
        output.push("[Settings]".to_string());
        output.push(format!("{}={}", key, value));
    } else if in_settings && !wrote_key {
        output.push(format!("{}={}", key, value));
    }

    output.join("\n") + "\n"
}

fn command_exists(command: &str) -> bool {
    std::process::Command::new("sh")
        .arg("-c")
        .arg(format!("command -v {} >/dev/null 2>&1", command))
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn run_optional_command(command: &str, warnings: &mut Vec<String>) {
    match proc::run_with_timeout(command, GTK_HOOK_TIMEOUT) {
        Ok(output) if output.status.success() => {}
        Ok(output) => warnings.push(String::from_utf8_lossy(&output.stderr).trim().to_string()),
        Err(error) => warnings.push(error.to_string()),
    }
}
