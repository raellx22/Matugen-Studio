// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager, WindowEvent,
};

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

pub mod commands;

#[derive(Default)]
struct AppExitState {
    quitting: AtomicBool,
}

fn show_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.set_skip_taskbar(false);
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

fn setup_tray(app: &mut tauri::App) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "show", "Abrir Matugen Studio", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Sair", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &quit])?;

    let mut tray = TrayIconBuilder::with_id("matugen-studio-tray")
        .menu(&menu)
        .tooltip("Matugen Studio")
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| match event.id() {
            id if id == "show" => show_main_window(app),
            id if id == "quit" => {
                app.state::<AppExitState>()
                    .quitting
                    .store(true, Ordering::SeqCst);
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| match event {
            TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            }
            | TrayIconEvent::DoubleClick {
                button: MouseButton::Left,
                ..
            } => show_main_window(tray.app_handle()),
            _ => {}
        });

    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }

    tray.build(app)?;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(AppExitState::default())
        .manage(commands::kde::KdeWallpaperWatcher::default())
        .manage(commands::wallhaven::WallhavenClient::new())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .on_window_event(|window, event| {
            if window.label() != "main" {
                return;
            }

            if let WindowEvent::CloseRequested { api, .. } = event {
                let app = window.app_handle();
                let is_quitting = app.state::<AppExitState>().quitting.load(Ordering::SeqCst);
                if !is_quitting && commands::kde::should_run_in_background() {
                    api.prevent_close();
                    let _ = window.set_skip_taskbar(true);
                    let _ = window.hide();
                }
            }
        })
        .setup(|app| {
            setup_tray(app)?;
            commands::kde::restore_kde_wallpaper_watcher(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            greet,
            commands::color::generate_scheme_from_image,
            commands::template::list_bundled_templates,
            commands::template::list_available_templates,
            commands::template::preview_template,
            commands::template::install_template,
            commands::template::apply_theme,
            commands::template::list_template_color_controls,
            commands::template::set_template_color_override,
            commands::template::reset_template_color_override,
            commands::template::apply_template_overrides_to_outputs,
            commands::template::get_installed_templates,
            commands::template::uninstall_template,
            commands::desktop::list_wallpapers,
            commands::desktop::apply_wallpaper,
            commands::desktop::get_monitor_count,
            commands::desktop::generate_thumbnail,
            commands::desktop::generate_thumbnails,
            commands::desktop::dir_exists,
            commands::gtk::apply_gtk_theme,
            commands::preset::save_preset,
            commands::preset::get_presets,
            commands::preset::delete_preset,
            commands::preset::export_preset,
            commands::preset::import_preset,
            commands::kde::generate_kde_colorscheme_cmd,
            commands::kde::apply_kde_colorscheme,
            commands::kde::get_kde_color_values,
            commands::kde::apply_kde_color_values,
            commands::kde::get_kde_current_wallpaper,
            commands::kde::start_kde_wallpaper_watcher,
            commands::kde::stop_kde_wallpaper_watcher,
            commands::kde::get_kde_wallpaper_watcher_status,
            commands::kde::mark_kde_wallpaper_handled,
            commands::kde::get_kde_service_status,
            commands::kde::set_kde_service_status,
            commands::kde::set_kde_run_in_background,
            commands::wallhaven::wallhaven_search,
            commands::wallhaven::wallhaven_download,
            commands::wallhaven::wallhaven_validate_key,
            commands::wallhaven::wallhaven_get_wallpaper
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
