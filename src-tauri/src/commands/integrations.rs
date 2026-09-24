//! Optional Plasma integrations. Only known color/opacity keys are patched.
use super::{
    proc,
    settings::{KdeIntegrationSettings, OutlineSource},
};
use serde::Serialize;
use serde_json::Value;
use std::{
    fs,
    path::Path,
    sync::{LazyLock, Mutex},
    time::Duration,
};
const TIMEOUT: Duration = Duration::from_secs(10);
const ROUNDED_ID: &str = "kwin4_effect_shapecorners";
static APPLY_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

#[derive(Debug, Clone, Default, Serialize)]
pub struct IntegrationDetection {
    pub detected: bool,
    pub supported: bool,
    pub version: Option<String>,
    pub active: bool,
    pub message: String,
}
#[derive(Debug, Default, Serialize)]
pub struct KdeIntegrationDetection {
    pub klassy: IntegrationDetection,
    pub rounded_corners: IntegrationDetection,
}
fn run(command: &str) -> Result<String, String> {
    let output = proc::run_with_timeout(command, TIMEOUT)?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}
fn klassy_version(text: &str) -> Option<(String, bool)> {
    let re = regex::Regex::new(r"\b(\d+)\.(\d+)\.(\d+)\b").ok()?;
    let version = re.captures(text)?;
    let major: u32 = version[1].parse().ok()?;
    let minor: u32 = version[2].parse().ok()?;
    // The installed 6.7 and upstream 6.5+ use WindowOutline*, not ThinWindowOutline*.
    Some((version[0].to_string(), major == 6 && minor >= 5))
}
fn has_effect(output: &str) -> bool {
    output
        .split(['\'', '"', ',', ' ', '\n', '[', ']', '(', ')'])
        .any(|s| s == ROUNDED_ID)
}
fn detect() -> KdeIntegrationDetection {
    let mut result = KdeIntegrationDetection::default();
    match run("QT_QPA_PLATFORM=offscreen klassy-settings --version") {
        Ok(output) => {
            if let Some((version, supported)) = klassy_version(&output) {
                result.klassy = IntegrationDetection {
                    detected: true,
                    supported,
                    version: Some(version),
                    active: false,
                    message: if supported {
                        "Detected"
                    } else {
                        "This Klassy version is not supported"
                    }
                    .into(),
                };
                if let Some(config) =
                    dirs::config_dir().and_then(|d| fs::read_to_string(d.join("kwinrc")).ok())
                {
                    result.klassy.active = config
                        .lines()
                        .any(|line| line.trim() == "library=org.kde.klassy");
                }
                if supported && !result.klassy.active {
                    result.klassy.message =
                        "Detected; select Klassy in KDE Window Decorations to see changes".into();
                }
            } else {
                result.klassy.message = "Klassy detected; version could not be verified".into();
            }
        }
        Err(_) => result.klassy.message = "Klassy not detected".into(),
    }
    let available = run("gdbus call --session --dest org.kde.KWin --object-path /Effects --method org.freedesktop.DBus.Properties.Get org.kde.kwin.Effects listOfEffects");
    match available {
        Ok(output) if has_effect(&output) => {
            result.rounded_corners = IntegrationDetection {
                detected: true,
                supported: true,
                active: false,
                version: None,
                message: "Detected".into(),
            };
            result.rounded_corners.active = run("gdbus call --session --dest org.kde.KWin --object-path /Effects --method org.kde.kwin.Effects.isEffectLoaded kwin4_effect_shapecorners").map(|s| s.contains("true")).unwrap_or(false);
            if !result.rounded_corners.active {
                result.rounded_corners.message =
                    "Detected; enable Rounded Corners in KDE Desktop Effects to see changes".into();
            }
        }
        Ok(_) => result.rounded_corners.message = "KDE Rounded Corners not detected".into(),
        Err(_) => {
            result.rounded_corners.message = "KDE Rounded Corners detection unavailable".into()
        }
    }
    result
}
#[tauri::command]
pub async fn detect_kde_integrations() -> Result<KdeIntegrationDetection, String> {
    tauri::async_runtime::spawn_blocking(detect)
        .await
        .map_err(|e| e.to_string())
}
fn role(context: &Value, name: &str) -> Result<String, String> {
    let color = &context["colors"][name]["default"];
    let hex = color["hex"]
        .as_str()
        .or_else(|| color["color"].as_str())
        .ok_or_else(|| format!("Missing Material role: {name}"))?;
    if ![7, 9].contains(&hex.len())
        || !hex.starts_with('#')
        || !hex[1..].bytes().all(|c| c.is_ascii_hexdigit())
    {
        return Err(format!("Invalid Material color: {name}"));
    }
    Ok(hex[..7].to_string())
}
fn outline_role(source: OutlineSource) -> &'static str {
    match source {
        OutlineSource::Automatic | OutlineSource::Outline => "outline",
        OutlineSource::Primary => "primary",
        OutlineSource::OutlineVariant => "outline_variant",
    }
}
type Patch = (&'static str, &'static str, String);
fn klassy_patches(
    context: &Value,
    settings: &KdeIntegrationSettings,
) -> Result<Vec<Patch>, String> {
    let mut patches = vec![];
    if !settings.klassy.enabled {
        return Ok(patches);
    }
    if settings.klassy.outline_sync {
        patches.extend([
            (
                "WindowOutlineStyle",
                "WindowOutlineStyleActive",
                "WindowOutlineCustomColor".into(),
            ),
            (
                "WindowOutlineStyle",
                "WindowOutlineCustomColorActive",
                role(context, "primary")?,
            ),
            (
                "WindowOutlineStyle",
                "WindowOutlineCustomColorOpacityActive",
                "100".into(),
            ),
        ]);
    }
    if let Some(opacity) = settings.klassy.titlebar_opacity {
        patches.push((
            "TitleBarOpacity",
            "TitleBarOpacityActive",
            opacity.to_string(),
        ));
        patches.push((
            "TitleBarOpacity",
            "OverrideTitleBarOpacityActive",
            "true".into(),
        ));
    }
    Ok(patches)
}
fn rounded_patches(
    context: &Value,
    settings: &KdeIntegrationSettings,
) -> Result<Vec<Patch>, String> {
    if !settings.rounded_corners.enabled || !settings.rounded_corners.outline_sync {
        return Ok(vec![]);
    }
    let hex = role(
        context,
        outline_role(settings.rounded_corners.outline_source),
    )?;
    let rgb = [1, 3, 5]
        .map(|i| {
            u8::from_str_radix(&hex[i..i + 2], 16)
                .expect("validated hex")
                .to_string()
        })
        .join(",");
    Ok(vec![
        ("Round-Corners", "OutlineColor", rgb.clone()),
        ("Round-Corners", "InactiveOutlineColor", rgb),
        ("Round-Corners", "ActiveOutlineUsePalette", "false".into()),
        ("Round-Corners", "InactiveOutlineUsePalette", "false".into()),
        ("Round-Corners", "ActiveOutlineUseCustom", "true".into()),
        ("Round-Corners", "InactiveOutlineUseCustom", "true".into()),
    ])
}
/// Preserve unrelated keys, comments and geometry. Add missing groups as needed.
fn patch_ini(content: &str, patches: &[Patch]) -> Result<String, String> {
    let mut output = content.to_string();
    for (group, key, value) in patches {
        if output
            .lines()
            .any(|l| l.trim() == format!("[{group}][$i]") || l.trim() == "[$i]")
        {
            return Err(format!("KDE configuration group {group} is immutable"));
        }
        let header = format!("[{group}]");
        let mut lines: Vec<String> = output.lines().map(str::to_string).collect();
        let start = if let Some(i) = lines.iter().position(|l| l.trim() == header) {
            i + 1
        } else {
            lines.push(header);
            lines.len()
        };
        let end = (start..lines.len())
            .find(|i| lines[*i].trim().starts_with('['))
            .unwrap_or(lines.len());
        let mut found = false;
        for line in &mut lines[start..end] {
            if let Some((k, _)) = line.split_once('=') {
                if k.trim() == format!("{key}[$i]") {
                    return Err(format!("KDE configuration key {key} is immutable"));
                }
                if k.trim() == *key {
                    *line = format!("{key}={value}");
                    found = true;
                }
            }
        }
        if !found {
            lines.insert(end, format!("{key}={value}"));
        }
        output = lines.join("\n") + "\n";
    }
    Ok(output)
}
fn write_patches(path: &Path, patches: &[Patch]) -> Result<bool, String> {
    if patches.is_empty() {
        return Ok(false);
    }
    let content = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(e.to_string()),
    };
    let next = patch_ini(&content, patches)?;
    if next == content {
        return Ok(false);
    }
    fs::create_dir_all(path.parent().ok_or("Invalid KDE config path")?)
        .map_err(|e| e.to_string())?;
    // Keep a first-use backup. Subsequent writes preserve all unrelated keys.
    let backup = path.with_extension("matugen-backup");
    if path.exists() && !backup.exists() {
        fs::copy(path, backup).map_err(|e| e.to_string())?;
    }
    let temp = path.with_extension("matugen-tmp");
    fs::write(&temp, next).map_err(|e| e.to_string())?;
    if let Ok(meta) = fs::metadata(path) {
        fs::set_permissions(&temp, meta.permissions()).map_err(|e| e.to_string())?;
    }
    fs::rename(temp, path).map_err(|e| e.to_string())?;
    Ok(true)
}
#[tauri::command]
pub async fn apply_kde_integrations(context: Value) -> Result<Vec<String>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let settings = super::settings::get_studio_settings()?;
        apply(&context, &settings.integrations)
    })
    .await
    .map_err(|e| e.to_string())?
}
pub fn apply(context: &Value, settings: &KdeIntegrationSettings) -> Result<Vec<String>, String> {
    settings.validate()?;
    if !settings.klassy.enabled && !settings.rounded_corners.enabled {
        return Ok(vec![]);
    }
    let _guard = APPLY_LOCK.lock().map_err(|e| e.to_string())?;
    let detection = detect();
    let config = dirs::config_dir().ok_or("Could not find config directory")?;
    let mut warnings = vec![];
    if settings.klassy.enabled {
        if !detection.klassy.supported {
            warnings.push(detection.klassy.message);
        } else {
            let result = (|| {
                if write_patches(
                    &config.join("klassy/klassyrc"),
                    &klassy_patches(context, settings)?,
                )? {
                    run("gdbus emit --session --object-path /KlassyDecoration --signal org.kde.Klassy.Style.updateDecorationColorCache")?;
                    run("gdbus emit --session --object-path /KWin --signal org.kde.KWin.reloadConfig")?;
                }
                Ok::<(), String>(())
            })();
            if let Err(e) = result {
                warnings.push(format!("Klassy: {e}"));
            }
        }
    }
    if settings.rounded_corners.enabled {
        if !detection.rounded_corners.supported {
            warnings.push(detection.rounded_corners.message);
        } else {
            let result = (|| {
                if write_patches(&config.join("kwinrc"), &rounded_patches(context, settings)?)?
                    && detection.rounded_corners.active
                {
                    run("gdbus call --session --dest org.kde.KWin --object-path /Effects --method org.kde.kwin.Effects.reconfigureEffect kwin4_effect_shapecorners")?;
                }
                Ok::<(), String>(())
            })();
            if let Err(e) = result {
                warnings.push(format!("Rounded Corners: {e}"));
            }
        }
    }
    Ok(warnings)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn version_detection_is_conservative() {
        assert_eq!(
            klassy_version("klassy-settings 6.7.3"),
            Some(("6.7.3".into(), true))
        );
        assert!(!klassy_version("klassy-settings 6.4.0").unwrap().1);
        assert!(klassy_version("").is_none());
        assert!(has_effect("(['blur', 'kwin4_effect_shapecorners'],)"));
        assert!(!has_effect("(['blur', 'kwin4_effect_shapecorners_fork'],)"));
    }
    #[test]
    fn disabled_integrations_never_need_colors_or_modify_config() {
        let settings = KdeIntegrationSettings::default();
        assert!(klassy_patches(&Value::Null, &settings).unwrap().is_empty());
        assert!(rounded_patches(&Value::Null, &settings).unwrap().is_empty());
        assert!(apply(&Value::Null, &settings).unwrap().is_empty());
    }
    #[test]
    fn patch_is_idempotent_and_preserves_geometry() {
        let original = "# comment\n[Round-Corners]\nRadius=12\nShadowSize=6\nOutlineColor = 1,2,3\n[Other]\nA=b\n";
        let patches = vec![("Round-Corners", "OutlineColor", "10,20,30".into())];
        let result = patch_ini(original, &patches).unwrap();
        assert!(result.contains("Radius=12\nShadowSize=6"));
        assert!(result.contains("[Other]\nA=b"));
        assert_eq!(patch_ini(&result, &patches).unwrap(), result);
        assert!(patch_ini("[Round-Corners][$i]\n", &patches).is_err());
        assert_eq!(
            patch_ini("", &patches).unwrap(),
            "[Round-Corners]\nOutlineColor=10,20,30\n"
        );
    }
    #[test]
    fn color_mapping_and_opacity_only_touch_requested_keys() {
        let context = serde_json::json!({"colors":{"primary":{"default":{"hex":"#123456"}},"outline_variant":{"default":{"hex":"#abcdef"}}}});
        let mut settings = KdeIntegrationSettings::default();
        settings.klassy.enabled = true;
        settings.klassy.outline_sync = true;
        settings.klassy.titlebar_opacity = Some(80);
        let patches = klassy_patches(&context, &settings).unwrap();
        let text = patch_ini("", &patches).unwrap();
        assert!(text.contains("WindowOutlineCustomColorActive=#123456"));
        assert!(text.contains("TitleBarOpacityActive=80"));
        assert!(!text.contains("Inactive"));
        settings.rounded_corners.enabled = true;
        settings.rounded_corners.outline_sync = true;
        settings.rounded_corners.outline_source = OutlineSource::OutlineVariant;
        assert!(
            patch_ini("", &rounded_patches(&context, &settings).unwrap())
                .unwrap()
                .contains("OutlineColor=171,205,239")
        );
        assert_eq!(outline_role(OutlineSource::Automatic), "outline");
        assert_eq!(outline_role(OutlineSource::Primary), "primary");
        assert_eq!(outline_role(OutlineSource::Outline), "outline");
    }
}
