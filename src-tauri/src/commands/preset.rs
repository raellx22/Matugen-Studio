use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use base64::{Engine as B64Engine, engine::general_purpose};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Preset {
    pub name: String,
    pub wallpaper_path: Option<String>,
    pub scheme_type: String,
    pub scheme_data: serde_json::Value,
}

/// The shareable preset format — exported as .matugen files
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ExportablePreset {
    pub version: u32,
    pub name: String,
    pub scheme_type: String,
    pub scheme_data: serde_json::Value,
    pub installed_templates: Vec<String>,
    /// Wallpaper image encoded as base64 (PNG/JPG)
    pub wallpaper_base64: Option<String>,
    /// Original wallpaper filename for reconstruction
    pub wallpaper_filename: Option<String>,
}

fn get_presets_file_path() -> Result<PathBuf, String> {
    let config_dir = dirs::config_dir()
        .ok_or("Could not find config directory")?
        .join("matugen");

    if !config_dir.exists() {
        fs::create_dir_all(&config_dir).map_err(|e| format!("Failed to create config dir {}: {}", config_dir.display(), e))?;
    }

    Ok(config_dir.join("gui-presets.json"))
}

#[tauri::command]
pub fn save_preset(preset: Preset) -> Result<(), String> {
    println!("Received save_preset request for: {}", preset.name);
    let path = get_presets_file_path()?;

    let mut presets: Vec<Preset> = if path.exists() {
        let content = fs::read_to_string(&path).map_err(|e| format!("Failed to read presets: {}", e))?;
        serde_json::from_str(&content).unwrap_or_else(|_| {
            println!("Failed to parse existing presets, returning empty list");
            vec![]
        })
    } else {
        vec![]
    };

    // Replace if exists
    if let Some(pos) = presets.iter().position(|p| p.name == preset.name) {
        presets[pos] = preset;
    } else {
        presets.push(preset);
    }

    let content = serde_json::to_string_pretty(&presets).map_err(|e| format!("Failed to serialize: {}", e))?;
    fs::write(&path, content).map_err(|e| format!("Failed to write presets file: {}", e))?;
    
    println!("Saved preset successfully!");

    Ok(())
}
#[tauri::command]
pub fn get_presets() -> Result<Vec<Preset>, String> {
    let path = get_presets_file_path()?;
    
    if !path.exists() {
        return Ok(vec![]);
    }
    
    let content = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let presets: Vec<Preset> = serde_json::from_str(&content).unwrap_or_else(|_| vec![]);
    
    Ok(presets)
}

#[tauri::command]
pub fn delete_preset(name: String) -> Result<(), String> {
    let path = get_presets_file_path()?;
    
    if !path.exists() {
        return Ok(());
    }
    
    let content = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let mut presets: Vec<Preset> = serde_json::from_str(&content).unwrap_or_else(|_| vec![]);
    
    presets.retain(|p| p.name != name);
    
    let content = serde_json::to_string_pretty(&presets).map_err(|e| e.to_string())?;
    fs::write(&path, content).map_err(|e| e.to_string())?;
    
    Ok(())
}

/// Export a preset to a .matugen file at the given path
#[tauri::command]
pub fn export_preset(preset: Preset, installed_templates: Vec<String>, output_path: String) -> Result<(), String> {
    // Read and encode wallpaper as base64
    let (wallpaper_base64, wallpaper_filename) = if let Some(ref wp_path) = preset.wallpaper_path {
        let path = PathBuf::from(wp_path);
        if path.exists() {
            let data = fs::read(&path).map_err(|e| format!("Failed to read wallpaper: {}", e))?;
            let encoded = general_purpose::STANDARD.encode(&data);
            let filename = path.file_name()
                .map(|f| f.to_string_lossy().to_string())
                .unwrap_or_else(|| "wallpaper.png".to_string());
            (Some(encoded), Some(filename))
        } else {
            (None, None)
        }
    } else {
        (None, None)
    };

    let exportable = ExportablePreset {
        version: 1,
        name: preset.name,
        scheme_type: preset.scheme_type,
        scheme_data: preset.scheme_data,
        installed_templates,
        wallpaper_base64,
        wallpaper_filename,
    };

    let json = serde_json::to_string_pretty(&exportable)
        .map_err(|e| format!("Failed to serialize preset: {}", e))?;
    
    fs::write(&output_path, json)
        .map_err(|e| format!("Failed to write file: {}", e))?;

    Ok(())
}

/// Import a preset from a .matugen file
#[tauri::command]
pub fn import_preset(file_path: String) -> Result<Preset, String> {
    let content = fs::read_to_string(&file_path)
        .map_err(|e| format!("Failed to read file: {}", e))?;
    
    let exportable: ExportablePreset = serde_json::from_str(&content)
        .map_err(|e| format!("Invalid .matugen file: {}", e))?;

    // Extract wallpaper to local folder
    let wallpaper_path = if let (Some(b64), Some(filename)) = (&exportable.wallpaper_base64, &exportable.wallpaper_filename) {
        let wallpapers_dir = dirs::config_dir()
            .ok_or("Could not find config directory")?
            .join("matugen")
            .join("shared-wallpapers");
        
        fs::create_dir_all(&wallpapers_dir).map_err(|e| e.to_string())?;

        let dest = wallpapers_dir.join(filename);
        let decoded = general_purpose::STANDARD.decode(b64)
            .map_err(|e| format!("Failed to decode wallpaper: {}", e))?;
        
        fs::write(&dest, &decoded)
            .map_err(|e| format!("Failed to save wallpaper: {}", e))?;
        
        Some(dest.to_string_lossy().to_string())
    } else {
        None
    };

    // Build the preset
    let preset = Preset {
        name: exportable.name,
        wallpaper_path,
        scheme_type: exportable.scheme_type,
        scheme_data: exportable.scheme_data,
    };

    // Auto-save to local presets
    save_preset(preset.clone())?;

    Ok(preset)
}
