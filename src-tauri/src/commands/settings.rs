//! Persistent source of truth shared by the UI, presets and wallpaper watcher.
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs,
    path::PathBuf,
    sync::{LazyLock, Mutex},
};

static SETTINGS_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));
fn one() -> f64 {
    1.0
}
fn default_scheme() -> String {
    "Tinted Smart".into()
}
fn nullable_contrast<'de, D: serde::Deserializer<'de>>(d: D) -> Result<f64, D::Error> {
    Ok(Option::<f64>::deserialize(d)?.unwrap_or(0.0))
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum MaterialSpec {
    #[serde(rename = "2021")]
    V2021,
    #[default]
    #[serde(rename = "2025")]
    V2025,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GenerationSettings {
    pub seed_index: i64,
    #[serde(deserialize_with = "nullable_contrast")]
    pub contrast: f64,
    pub chroma: f64,
    pub tone: f64,
    pub material_spec: MaterialSpec,
}
impl Default for GenerationSettings {
    fn default() -> Self {
        Self {
            seed_index: 0,
            contrast: 0.0,
            chroma: one(),
            tone: one(),
            material_spec: MaterialSpec::default(),
        }
    }
}
impl GenerationSettings {
    pub fn validate(&self) -> Result<(), String> {
        // Contrast is MCU's range. Multipliers follow KDE Material You Colors;
        // HCT itself clamps the resulting tone to 0..100 and maps chroma to sRGB.
        for (name, value, min, max) in [
            ("Contrast", self.contrast, -1.0, 1.0),
            ("Chroma", self.chroma, 0.0, 10.0),
            ("Tone", self.tone, 0.0, 1.5),
        ] {
            if !value.is_finite() || !(min..=max).contains(&value) {
                return Err(format!("{name} must be between {min} and {max}"));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutlineSource {
    #[default]
    Automatic,
    Primary,
    Outline,
    OutlineVariant,
}
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct KlassySettings {
    pub enabled: bool,
    pub outline_sync: bool,
    pub titlebar_opacity: Option<u8>,
}
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RoundedCornersSettings {
    pub enabled: bool,
    pub outline_sync: bool,
    pub outline_source: OutlineSource,
}
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct KdeIntegrationSettings {
    pub klassy: KlassySettings,
    pub rounded_corners: RoundedCornersSettings,
}
impl KdeIntegrationSettings {
    pub fn validate(&self) -> Result<(), String> {
        if self.klassy.titlebar_opacity.is_some_and(|v| v > 100) {
            return Err("Titlebar opacity must be between 0 and 100".into());
        }
        Ok(())
    }
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ColorMode {
    Dark,
    Light,
    #[default]
    Auto,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct StudioSettings {
    pub generation: GenerationSettings,
    pub integrations: KdeIntegrationSettings,
    pub mode: ColorMode,
    pub scheme_type: String,
    pub kde_overrides: HashMap<String, String>,
    pub gtk_enabled: Option<bool>,
}
impl Default for StudioSettings {
    fn default() -> Self {
        Self {
            generation: GenerationSettings::default(),
            integrations: KdeIntegrationSettings::default(),
            mode: ColorMode::Auto,
            scheme_type: default_scheme(),
            kde_overrides: HashMap::new(),
            gtk_enabled: None,
        }
    }
}
impl StudioSettings {
    pub fn validate(&self) -> Result<(), String> {
        self.generation.validate()?;
        self.integrations.validate()?;
        for hex in self.kde_overrides.values() {
            if hex.len() != 7
                || !hex.starts_with('#')
                || !hex[1..].bytes().all(|c| c.is_ascii_hexdigit())
            {
                return Err("Invalid KDE override color".into());
            }
        }
        Ok(())
    }
}
fn settings_path() -> Result<PathBuf, String> {
    Ok(dirs::config_dir()
        .ok_or("Could not find config directory")?
        .join("matugen-studio/settings.json"))
}
#[tauri::command]
pub fn get_studio_settings() -> Result<StudioSettings, String> {
    let _guard = SETTINGS_LOCK.lock().map_err(|e| e.to_string())?;
    let path = settings_path()?;
    if !path.exists() {
        let service = super::kde::get_kde_service_status()?;
        return Ok(StudioSettings {
            scheme_type: service.scheme_type,
            ..Default::default()
        });
    }
    let settings: StudioSettings =
        serde_json::from_str(&fs::read_to_string(path).map_err(|e| e.to_string())?)
            .map_err(|e| format!("Invalid Studio settings: {e}"))?;
    settings.validate()?;
    Ok(settings)
}
#[tauri::command]
pub fn set_studio_settings(settings: StudioSettings) -> Result<StudioSettings, String> {
    settings.validate()?;
    let _guard = SETTINGS_LOCK.lock().map_err(|e| e.to_string())?;
    let path = settings_path()?;
    fs::create_dir_all(path.parent().ok_or("Invalid settings path")?).map_err(|e| e.to_string())?;
    let tmp = path.with_extension("json.tmp");
    fs::write(
        &tmp,
        serde_json::to_vec_pretty(&settings).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    fs::rename(tmp, path).map_err(|e| e.to_string())?;
    Ok(settings)
}
/// Set every template's default variant from the same mode decision used for GTK.
pub fn resolve_mode(context: &mut serde_json::Value, mode: ColorMode) -> bool {
    let dark = match mode {
        ColorMode::Dark => true,
        ColorMode::Light => false,
        ColorMode::Auto => {
            context
                .get("wallpaper_luminance")
                .and_then(|v| v.as_f64())
                .or_else(|| {
                    context
                        .get("source_color_luminance")
                        .and_then(|v| v.as_f64())
                })
                .unwrap_or(0.0)
                <= 0.5
        }
    };
    if let Some(colors) = context.get_mut("colors").and_then(|v| v.as_object_mut()) {
        for group in colors.values_mut() {
            if let Some(value) = group.get(if dark { "dark" } else { "light" }).cloned() {
                group["default"] = value;
            }
        }
    }
    dark
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn legacy_and_null_defaults() {
        let settings: GenerationSettings = serde_json::from_str(r#"{"contrast":null}"#).unwrap();
        assert_eq!(settings, GenerationSettings::default());
        assert!(serde_json::from_str::<GenerationSettings>(r#"{"material_spec":"2099"}"#).is_err());
    }
    #[test]
    fn ranges_reject_invalid_numbers() {
        for contrast in [-1.01, 1.01, f64::NAN, f64::INFINITY] {
            assert!(GenerationSettings {
                contrast,
                ..Default::default()
            }
            .validate()
            .is_err());
        }
        for chroma in [-0.1, 10.1, f64::NAN] {
            assert!(GenerationSettings {
                chroma,
                ..Default::default()
            }
            .validate()
            .is_err());
        }
        for tone in [-0.1, 1.51, f64::INFINITY] {
            assert!(GenerationSettings {
                tone,
                ..Default::default()
            }
            .validate()
            .is_err());
        }
        assert!(GenerationSettings::default().validate().is_ok());
    }
    #[test]
    fn mode_updates_default_and_gtk_together() {
        let mut ctx = serde_json::json!({"wallpaper_luminance":0.8,"colors":{"primary":{"dark":{"hex":"#000000"},"light":{"hex":"#ffffff"}}}});
        assert!(!resolve_mode(&mut ctx, ColorMode::Auto));
        assert_eq!(ctx["colors"]["primary"]["default"]["hex"], "#ffffff");
        assert!(resolve_mode(&mut ctx, ColorMode::Dark));
    }
}
