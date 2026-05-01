// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

pub mod commands;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            greet,
            commands::color::generate_scheme_from_image,
            commands::template::list_available_templates,
            commands::template::preview_template,
            commands::template::install_template,
            commands::template::apply_theme,
            commands::template::get_installed_templates,
            commands::template::uninstall_template,
            commands::desktop::list_wallpapers,
            commands::desktop::apply_wallpaper,
            commands::desktop::get_monitor_count,
            commands::desktop::generate_thumbnail,
            commands::desktop::dir_exists,
            commands::preset::save_preset,
            commands::preset::get_presets,
            commands::preset::delete_preset,
            commands::preset::export_preset,
            commands::preset::import_preset,
            commands::kde::generate_kde_colorscheme_cmd,
            commands::kde::apply_kde_colorscheme,
            commands::kde::get_kde_current_wallpaper,
            commands::kde::get_kde_service_status,
            commands::kde::set_kde_service_status
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
