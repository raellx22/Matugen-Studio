use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use matugen_core::parser::Engine;
use matugen_core::State;
use matugen_core::util::config::ConfigFile;
use execute::{shell, Execute};

#[derive(Serialize, Deserialize)]
pub struct TemplateInfo {
    name: String,
    path: String,
    size: u64,
}

#[tauri::command]
pub fn list_available_templates(themes_dir: String) -> Result<Vec<TemplateInfo>, String> {
    let mut templates = Vec::new();
    let path = PathBuf::from(themes_dir).join("templates");

    if !path.exists() {
        return Err("Templates directory not found".to_string());
    }

    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            if let Ok(meta) = entry.metadata() {
                if meta.is_file() {
                    templates.push(TemplateInfo {
                        name: entry.file_name().to_string_lossy().to_string(),
                        path: entry.path().to_string_lossy().to_string(),
                        size: meta.len(),
                    });
                }
            }
        }
    }

    templates.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(templates)
}

#[tauri::command]
pub fn preview_template(template_path: String, context: serde_json::Value) -> Result<String, String> {
    let mut engine = Engine::new();
    
    // Register filters
    State::add_engine_filters(&mut engine);
    
    // Read the template file
    let source = fs::read_to_string(&template_path)
        .map_err(|e| format!("Failed to read template: {}", e))?;
        
    engine.add_template("preview".to_string(), source);
    engine.add_context(context);
    
    let result = engine.render("preview")
        .map_err(|errs| {
            let mut err_msg = String::new();
            for err in errs {
                err_msg.push_str(&format!("{:?}\n", err));
            }
            err_msg
        })?;
        
    Ok(result)
}

#[tauri::command]
pub fn install_template(template_path: String, template_name: String, output_path: String, post_hook: Option<String>) -> Result<(), String> {
    let config_dir = dirs::config_dir()
        .ok_or("Could not find config directory")?
        .join("matugen");
        
    let templates_dir = config_dir.join("templates");
    fs::create_dir_all(&templates_dir).map_err(|e| e.to_string())?;

    let dest_path = templates_dir.join(&template_name);
    fs::copy(&template_path, &dest_path).map_err(|e| e.to_string())?;

    let config_path = config_dir.join("config.toml");
    let mut config_content = String::new();
    if config_path.exists() {
        config_content = fs::read_to_string(&config_path).unwrap_or_default();
    }
    
    if !config_content.contains("[config]") {
        config_content = "[config]\n".to_string() + &config_content;
    }

    let name_key = template_name.replace(".", "_").replace("-", "_");

    if config_content.contains(&format!("[templates.{}]", name_key)) {
        return Err("Template already installed. Please check your ~/.config/matugen/config.toml".to_string());
    }

    config_content.push_str(&format!("\n[templates.{}]\n", name_key));
    config_content.push_str(&format!("input_path = \"{}\"\n", dest_path.to_string_lossy()));
    config_content.push_str(&format!("output_path = \"{}\"\n", output_path));
    
    if let Some(hook) = post_hook {
        if !hook.trim().is_empty() {
            config_content.push_str(&format!("post_hook = \"{}\"\n", hook.trim()));
        }
    }

    fs::write(&config_path, config_content).map_err(|e| e.to_string())?;

    Ok(())
}

fn expand_tilde(path: &PathBuf) -> PathBuf {
    let s = path.to_string_lossy();
    if s.starts_with("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(&s[2..]);
        }
    }
    path.clone()
}

#[tauri::command]
pub fn apply_theme(context: serde_json::Value) -> Result<(), String> {
    let config_dir = dirs::config_dir()
        .ok_or("Could not find config directory")?
        .join("matugen");
        
    let config_path = config_dir.join("config.toml");
    let mut config_content = fs::read_to_string(&config_path)
        .map_err(|e| format!("Failed to read config.toml: {}", e))?;
        
    if !config_content.contains("[config]") {
        config_content = "[config]\n".to_string() + &config_content;
    }
        
    let config_file: ConfigFile = toml::from_str(&config_content)
        .map_err(|e| format!("Invalid config.toml: {}", e))?;

    let mut engine = Engine::new();
    State::add_engine_filters(&mut engine);

    engine.add_context(context.clone());

    for (name, template) in config_file.templates.iter() {
        let input_path = &template.input_path;
        let input_abs = if input_path.is_absolute() {
            input_path.clone()
        } else {
            config_path.parent().unwrap().join(input_path)
        };
        
        let input_abs = expand_tilde(&input_abs);

        if !input_abs.exists() {
            continue;
        }

        let source = match fs::read_to_string(&input_abs) {
            Ok(s) => s,
            Err(_) => continue,
        };
            
        engine.add_template(name.clone(), source);
        
        let result = match engine.render(name) {
            Ok(r) => r,
            Err(_) => continue,
        };

        if let Some(matugen_core::template::OutputPath::Single(out_path)) = &template.output_path {
            let out_abs = expand_tilde(&out_path);
            
            if let Some(parent) = out_abs.parent() {
                fs::create_dir_all(parent).ok();
            }
            
            if fs::write(&out_abs, result).is_ok() {
                if let Some(hook) = &template.post_hook {
                    let mut cmd = shell(hook);
                    cmd.execute().ok();
                }
            }
        }
    }

    Ok(())
}

#[tauri::command]
pub fn get_installed_templates() -> Result<Vec<String>, String> {
    let config_dir = dirs::config_dir()
        .ok_or("Could not find config directory")?
        .join("matugen");
        
    let config_path = config_dir.join("config.toml");
    if !config_path.exists() {
        return Ok(vec![]);
    }
    
    let config_content = fs::read_to_string(&config_path).unwrap_or_default();
    let mut installed = Vec::new();
    
    for line in config_content.lines() {
        let line = line.trim();
        if line.starts_with("[templates.") && line.ends_with(']') {
            let name_key = &line[11..line.len()-1];
            installed.push(name_key.to_string());
        }
    }
    
    Ok(installed)
}

#[tauri::command]
pub fn uninstall_template(template_name: String) -> Result<(), String> {
    let config_dir = dirs::config_dir()
        .ok_or("Could not find config directory")?
        .join("matugen");
        
    let name_key = template_name.replace(".", "_").replace("-", "_");
    let dest_path = config_dir.join("templates").join(&template_name);
    
    if dest_path.exists() {
        fs::remove_file(&dest_path).ok();
    }
    
    let config_path = config_dir.join("config.toml");
    if config_path.exists() {
        let config_content = fs::read_to_string(&config_path).unwrap_or_default();
        let mut new_lines = Vec::new();
        let mut skip = false;
        
        for line in config_content.lines() {
            let t_line = line.trim();
            if t_line.starts_with("[templates.") {
                skip = t_line == format!("[templates.{}]", name_key);
            } else if t_line.starts_with('[') {
                skip = false;
            }
            if !skip {
                new_lines.push(line);
            }
        }
        fs::write(&config_path, new_lines.join("\n")).map_err(|e| e.to_string())?;
    }
    
    Ok(())
}
