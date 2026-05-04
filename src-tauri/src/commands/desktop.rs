use execute::{shell, Execute};
use percent_encoding::{utf8_percent_encode, AsciiSet, CONTROLS};
use serde::Serialize;
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::BufWriter;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;
use tauri::Manager;

const THUMBNAIL_SIZE: u32 = 256;
const THUMBNAIL_DIRS: [&str; 4] = ["large", "x-large", "xx-large", "normal"];
const PATH_ENCODE_SET: &AsciiSet = &CONTROLS
    .add(b' ')
    .add(b'"')
    .add(b'#')
    .add(b'%')
    .add(b'<')
    .add(b'>')
    .add(b'?')
    .add(b'`')
    .add(b'{')
    .add(b'}');

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThumbnailResult {
    image_path: String,
    thumbnail_path: Option<String>,
    error: Option<String>,
}

#[tauri::command]
pub async fn list_wallpapers(
    app: tauri::AppHandle,
    folder_path: String,
) -> Result<Vec<String>, String> {
    allow_asset_directory(&app, Path::new(&folder_path), true);
    allow_thumbnail_cache(&app);

    tauri::async_runtime::spawn_blocking(move || list_wallpapers_blocking(folder_path))
        .await
        .map_err(|e| e.to_string())?
}

fn list_wallpapers_blocking(folder_path: String) -> Result<Vec<String>, String> {
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
                    if ext_lower == "jpg"
                        || ext_lower == "jpeg"
                        || ext_lower == "png"
                        || ext_lower == "webp"
                    {
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
            }
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
                        }
                        Err(e) => Err(format!("qdbus execution failed: {}", e)),
                    }
                }
            }
            Err(e) => Err(format!("Failed to execute dbus command: {}", e)),
        }
    }
}

#[tauri::command]
pub async fn generate_thumbnail(
    app: tauri::AppHandle,
    image_path: String,
) -> Result<String, String> {
    allow_thumbnail_cache(&app);

    let thumbnail_path =
        tauri::async_runtime::spawn_blocking(move || resolve_or_generate_thumbnail(&image_path))
            .await
            .map_err(|e| e.to_string())??;

    allow_asset_file(&app, Path::new(&thumbnail_path));
    Ok(thumbnail_path)
}

#[tauri::command]
pub async fn generate_thumbnails(
    app: tauri::AppHandle,
    image_paths: Vec<String>,
) -> Vec<ThumbnailResult> {
    allow_thumbnail_cache(&app);

    let results: Vec<ThumbnailResult> = tauri::async_runtime::spawn_blocking(move || {
        image_paths
            .into_iter()
            .map(
                |image_path| match resolve_or_generate_thumbnail(&image_path) {
                    Ok(thumbnail_path) => ThumbnailResult {
                        image_path,
                        thumbnail_path: Some(thumbnail_path),
                        error: None,
                    },
                    Err(error) => ThumbnailResult {
                        image_path,
                        thumbnail_path: None,
                        error: Some(error),
                    },
                },
            )
            .collect()
    })
    .await
    .unwrap_or_default();

    for result in &results {
        if let Some(thumbnail_path) = &result.thumbnail_path {
            allow_asset_file(&app, Path::new(thumbnail_path));
        }
    }

    results
}

fn resolve_or_generate_thumbnail(image_path: &str) -> Result<String, String> {
    let path = PathBuf::from(&image_path);
    if !path.exists() {
        return Err("File does not exist".into());
    }

    let cache_dir = dirs::cache_dir().ok_or("No cache dir")?.join("thumbnails");
    let uri = file_uri(&path)?;
    let meta = fs::metadata(&path).map_err(|e| e.to_string())?;
    let modified = meta
        .modified()
        .unwrap_or(UNIX_EPOCH)
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let size = meta.len();
    let hash = format!("{:x}", md5::compute(uri.as_bytes()));

    for dir_name in THUMBNAIL_DIRS {
        let candidate = cache_dir.join(dir_name).join(format!("{}.png", hash));
        if is_current_thumbnail(&candidate, &uri, modified) {
            return Ok(candidate.to_string_lossy().to_string());
        }
    }

    let thumb_path = cache_dir.join("large").join(format!("{}.png", hash));
    fs::create_dir_all(thumb_path.parent().ok_or("Invalid thumbnail path")?)
        .map_err(|e| e.to_string())?;

    let img = image::open(&path).map_err(|e| e.to_string())?;
    let thumb = img.thumbnail(THUMBNAIL_SIZE, THUMBNAIL_SIZE).to_rgba8();
    write_freedesktop_thumbnail(&thumb_path, &thumb, &uri, modified, size)?;

    Ok(thumb_path.to_string_lossy().to_string())
}

fn file_uri(path: &Path) -> Result<String, String> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|e| e.to_string())?
            .join(path)
    };
    let path_string = absolute.to_string_lossy();
    Ok(format!(
        "file://{}",
        utf8_percent_encode(&path_string, PATH_ENCODE_SET)
    ))
}

fn is_current_thumbnail(path: &Path, uri: &str, modified: u64) -> bool {
    if !path.exists() {
        return false;
    }

    match read_thumbnail_metadata(path) {
        Ok(metadata) => {
            let thumb_uri = metadata.get("Thumb::URI");
            let thumb_mtime = metadata
                .get("Thumb::MTime")
                .and_then(|mtime| mtime.parse::<u64>().ok());

            if thumb_uri.map(String::as_str) == Some(uri) && thumb_mtime == Some(modified) {
                return true;
            }
        }
        Err(_) => return false,
    }

    false
}

fn read_thumbnail_metadata(path: &Path) -> Result<HashMap<String, String>, String> {
    let file = File::open(path).map_err(|e| e.to_string())?;
    let decoder = png::Decoder::new(file);
    let reader = decoder.read_info().map_err(|e| e.to_string())?;
    let info = reader.info();
    let mut metadata = HashMap::new();

    for text_chunk in &info.uncompressed_latin1_text {
        metadata.insert(text_chunk.keyword.clone(), text_chunk.text.clone());
    }

    for text_chunk in &info.compressed_latin1_text {
        if let Ok(text) = text_chunk.get_text() {
            metadata.insert(text_chunk.keyword.clone(), text);
        }
    }

    for text_chunk in &info.utf8_text {
        if let Ok(text) = text_chunk.get_text() {
            metadata.insert(text_chunk.keyword.clone(), text);
        }
    }

    Ok(metadata)
}

fn write_freedesktop_thumbnail(
    thumb_path: &Path,
    thumb: &image::RgbaImage,
    uri: &str,
    modified: u64,
    size: u64,
) -> Result<(), String> {
    let tmp_path = thumb_path.with_extension("png.tmp");
    let file = File::create(&tmp_path).map_err(|e| e.to_string())?;
    let writer = BufWriter::new(file);
    let mut encoder = png::Encoder::new(writer, thumb.width(), thumb.height());
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .add_text_chunk("Thumb::URI".to_string(), uri.to_string())
        .map_err(|e| e.to_string())?;
    encoder
        .add_text_chunk("Thumb::MTime".to_string(), modified.to_string())
        .map_err(|e| e.to_string())?;
    encoder
        .add_text_chunk("Thumb::Size".to_string(), size.to_string())
        .map_err(|e| e.to_string())?;
    encoder
        .add_text_chunk("Software".to_string(), "Matugen Studio".to_string())
        .map_err(|e| e.to_string())?;

    let mut png_writer = encoder.write_header().map_err(|e| e.to_string())?;
    png_writer
        .write_image_data(thumb.as_raw())
        .map_err(|e| e.to_string())?;
    drop(png_writer);

    fs::rename(tmp_path, thumb_path).map_err(|e| e.to_string())
}

fn allow_thumbnail_cache(app: &tauri::AppHandle) {
    if let Some(cache_dir) = dirs::cache_dir() {
        allow_asset_directory(app, &cache_dir.join("thumbnails"), true);
    }
}

fn allow_asset_directory(app: &tauri::AppHandle, path: &Path, recursive: bool) {
    let _ = app.asset_protocol_scope().allow_directory(path, recursive);
}

fn allow_asset_file(app: &tauri::AppHandle, path: &Path) {
    let _ = app.asset_protocol_scope().allow_file(path);
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
