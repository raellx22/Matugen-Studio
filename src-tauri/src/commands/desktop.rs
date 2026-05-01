use std::fs;
use std::path::PathBuf;
use std::time::UNIX_EPOCH;
use sha2::{Sha256, Digest};
use execute::{shell, Execute};

#[tauri::command]
pub fn list_wallpapers(folder_path: String) -> Result<Vec<String>, String> {
    let mut wallpapers = Vec::new();
    let path = PathBuf::from(&folder_path);

    if !path.exists() || !path.is_dir() {
        return Err("Invalid directory".to_string());
    }

    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_file() {
                if let Some(ext) = p.extension().and_then(|e| e.to_str()) {
                    let ext_lower = ext.to_lowercase();
                    if ext_lower == "jpg" || ext_lower == "jpeg" || ext_lower == "png" || ext_lower == "webp" {
                        wallpapers.push(p.to_string_lossy().to_string());
                    }
                }
            }
        }
    }

    wallpapers.sort();
    Ok(wallpapers)
}

#[tauri::command]
pub fn get_monitor_count(window: tauri::Window) -> usize {
    window.available_monitors().map(|m| m.len()).unwrap_or(1)
}

#[tauri::command]
pub fn apply_wallpaper(image_path: String, screen_index: i32) -> Result<(), String> {
    if screen_index == -1 {
        // Global application using the official command
        let cmd_str = format!("plasma-apply-wallpaperimage \"{}\"", image_path);
        let mut cmd = shell(&cmd_str);
        
        match cmd.execute_output() {
            Ok(output) => {
                if output.status.success() {
                    Ok(())
                } else {
                    Err(String::from_utf8_lossy(&output.stderr).to_string())
                }
            },
            Err(e) => Err(format!("Failed to execute wallpaper command: {}", e)),
        }
    } else {
        // Per-monitor application via DBus
        let safe_path = image_path.replace("'", "\\'");
        let script = format!(
            "var allDesktops = desktops(); var target = allDesktops[{}]; if (target) {{ target.wallpaperPlugin = 'org.kde.image'; target.currentConfigGroup = ['Wallpaper', 'org.kde.image', 'General']; target.writeConfig('Image', 'file://{}'); }}",
            screen_index, safe_path
        );
        let mut cmd = std::process::Command::new("qdbus6");
        cmd.arg("org.kde.plasmashell")
           .arg("/PlasmaShell")
           .arg("org.kde.PlasmaShell.evaluateScript")
           .arg(&script);
           
        match cmd.execute_output() {
            Ok(output) => {
                if output.status.success() {
                    Ok(())
                } else {
                    // fallback to qdbus if qdbus6 is not found
                    let mut cmd2 = std::process::Command::new("qdbus");
                    cmd2.arg("org.kde.plasmashell")
                       .arg("/PlasmaShell")
                       .arg("org.kde.PlasmaShell.evaluateScript")
                       .arg(&script);
                    match cmd2.execute_output() {
                        Ok(output2) => {
                            if output2.status.success() {
                                Ok(())
                            } else {
                                let err1 = String::from_utf8_lossy(&output.stderr).to_string();
                                let err2 = String::from_utf8_lossy(&output2.stderr).to_string();
                                Err(format!("qdbus6 error: {} | qdbus error: {}", err1, err2))
                            }
                        },
                        Err(e) => Err(format!("qdbus execution failed: {}", e))
                    }
                }
            },
            Err(e) => Err(format!("Failed to execute dbus command: {}", e)),
        }
    }
}

#[tauri::command]
pub fn generate_thumbnail(image_path: String) -> Result<String, String> {
    let path = PathBuf::from(&image_path);
    if !path.exists() {
        return Err("File does not exist".into());
    }

    let cache_dir = dirs::cache_dir()
        .ok_or("No cache dir")?
        .join("matugen-gui")
        .join("thumbnails");
    
    fs::create_dir_all(&cache_dir).ok();

    let meta = fs::metadata(&path).map_err(|e| e.to_string())?;
    let modified = meta.modified()
        .unwrap_or(UNIX_EPOCH)
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let mut hasher = Sha256::new();
    hasher.update(image_path.as_bytes());
    hasher.update(modified.to_string().as_bytes());
    let hash = format!("{:x}", hasher.finalize());

    let thumb_path = cache_dir.join(format!("{}.jpg", hash));

    if thumb_path.exists() {
        return Ok(thumb_path.to_string_lossy().to_string());
    }

    let img = image::open(&path).map_err(|e| e.to_string())?;
    let thumb = img.thumbnail(400, 400);
    thumb.save(&thumb_path).map_err(|e| e.to_string())?;
    
    Ok(thumb_path.to_string_lossy().to_string())
}

#[tauri::command]
pub fn dir_exists(path: String) -> bool {
    let p = if path.starts_with("~/") {
        if let Some(home) = dirs::home_dir() {
            home.join(path.strip_prefix("~/").unwrap())
        } else {
            return false;
        }
    } else {
        PathBuf::from(path)
    };
    p.exists() && p.is_dir()
}
