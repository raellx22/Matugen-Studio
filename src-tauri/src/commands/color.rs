use material_colors::theme::ThemeBuilder;
use matugen_core::{
    color::base16::{generate_base16_schemes, Backend},
    color::color::{get_luminance, get_source_color, Source},
    helpers::merge_json_source,
    scheme::{get_custom_color_schemes, get_schemes, SchemeTypes, SchemesEnum},
    util::arguments::FilterType,
};
use serde_json::Value;

struct WallpaperAnalysis {
    luminance: f64,
    grayscale_score: f64,
}

struct SchemeRequest {
    requested: String,
    base: String,
    tinted: bool,
}

#[tauri::command]
pub async fn generate_scheme_from_image(
    image_path: String,
    scheme_type: String,
) -> Result<serde_json::Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        generate_scheme_from_image_blocking(image_path, scheme_type)
    })
    .await
    .map_err(|e| e.to_string())?
}

pub fn generate_scheme_from_image_blocking(
    image_path: String,
    scheme_type: String,
) -> Result<serde_json::Value, String> {
    let wallpaper_analysis = analyse_wallpaper(&image_path).ok();
    let source = Source::Image { path: image_path };
    let resize_filter = Some(FilterType::Triangle);
    let fallback = None;
    let prefer = None;
    let source_color_index = Some(0);

    let source_color = get_source_color(
        &source,
        &resize_filter,
        fallback,
        &prefer,
        &source_color_index,
    )
    .map_err(|e| format!("Error getting source color: {}", e))?;

    let smart_monochrome = wallpaper_analysis
        .as_ref()
        .map(|analysis| analysis.grayscale_score <= 0.035)
        .unwrap_or(false);

    let scheme_request = parse_scheme_request(&scheme_type);
    let effective_scheme_type = if scheme_request.base == "Smart" && smart_monochrome {
        "Monochrome"
    } else if scheme_request.base == "Smart" {
        "Content"
    } else {
        scheme_request.base.as_str()
    };

    let scheme_type_enum = match effective_scheme_type {
        "Expressive" => SchemeTypes::SchemeExpressive,
        "Fidelity" => SchemeTypes::SchemeFidelity,
        "Fruit Salad" => SchemeTypes::SchemeFruitSalad,
        "Monochrome" => SchemeTypes::SchemeMonochrome,
        "Neutral" => SchemeTypes::SchemeNeutral,
        "Rainbow" => SchemeTypes::SchemeRainbow,
        "Tonal Spot" => SchemeTypes::SchemeTonalSpot,
        "Vibrant" => SchemeTypes::SchemeVibrant,
        _ => SchemeTypes::SchemeContent,
    };
    let theme = ThemeBuilder::with_source(source_color).build();
    let contrast = Some(0.0); // Resetting static contrast to allow natural scheme behavior
    let scheme_type_opt = Some(scheme_type_enum);

    let (scheme_dark, scheme_light) = get_schemes(source_color, &scheme_type_opt, &contrast);
    let mut schemes = get_custom_color_schemes(
        source_color,
        scheme_dark,
        scheme_light,
        &None,
        &scheme_type_opt,
        &contrast,
        &None,
        &None,
    )
    .map_err(|error| error.to_string())?;
    schemes.dark.insert("source_color".to_owned(), source_color);
    schemes
        .light
        .insert("source_color".to_owned(), source_color);

    let base16 = generate_base16_schemes(&source, Backend::Wal).map_err(|e| e.to_string())?;

    let mut json = merge_json_source(
        serde_json::json!({
            "source_color_hex": source_color.to_hex_with_pound(),
            "source_color_luminance": get_luminance(&source_color),
            "wallpaper_luminance": wallpaper_analysis.as_ref().map(|analysis| analysis.luminance),
            "wallpaper_grayscale_score": wallpaper_analysis.as_ref().map(|analysis| analysis.grayscale_score),
            "requested_scheme_type": scheme_request.requested.clone(),
            "effective_scheme_type": effective_scheme_type,
            "surface_style": if scheme_request.tinted { "tinted" } else { "classic" },
        }),
        &Some(schemes),
        &Some(base16),
        &Some(theme),
        if wallpaper_analysis
            .as_ref()
            .map(|analysis| analysis.luminance)
            .unwrap_or_else(|| get_luminance(&source_color))
            > 0.5
        {
            SchemesEnum::Light
        } else {
            SchemesEnum::Dark
        },
    )
    .map_err(|e| e.to_string())?;

    if scheme_request.tinted {
        apply_surface_tint(&mut json, effective_scheme_type);
    }

    Ok(json)
}

fn parse_scheme_request(scheme_type: &str) -> SchemeRequest {
    if let Some(base) = scheme_type.strip_prefix("Tinted ") {
        return SchemeRequest {
            requested: scheme_type.to_string(),
            base: base.to_string(),
            tinted: true,
        };
    }

    if let Some(base) = scheme_type.strip_prefix("Classic ") {
        return SchemeRequest {
            requested: scheme_type.to_string(),
            base: base.to_string(),
            tinted: false,
        };
    }

    SchemeRequest {
        requested: scheme_type.to_string(),
        base: scheme_type.to_string(),
        tinted: false,
    }
}

fn surface_tint_strength(scheme_type: &str) -> f64 {
    match scheme_type {
        "Monochrome" => 0.0,
        "Neutral" => 0.025,
        "Tonal Spot" => 0.045,
        "Content" => 0.075,
        "Fidelity" => 0.09,
        "Expressive" => 0.105,
        "Fruit Salad" | "Rainbow" => 0.12,
        "Vibrant" => 0.14,
        _ => 0.075,
    }
}

fn apply_surface_tint(context: &mut Value, scheme_type: &str) {
    let strength = surface_tint_strength(scheme_type);
    if strength <= 0.0 {
        return;
    }

    let variants = ["default", "dark", "light"];
    let primary_by_variant: Vec<(&str, String)> = variants
        .iter()
        .filter_map(|variant| {
            read_scheme_hex(context, "primary", variant).map(|hex| (*variant, hex))
        })
        .collect();
    if primary_by_variant.is_empty() {
        return;
    }

    let Some(colors) = context.get_mut("colors").and_then(Value::as_object_mut) else {
        return;
    };

    let surface_keys = [
        ("background", 0.55),
        ("surface", 0.72),
        ("surface_dim", 0.64),
        ("surface_bright", 0.68),
        ("surface_container_lowest", 0.56),
        ("surface_container_low", 0.76),
        ("surface_container", 0.92),
        ("surface_container_high", 1.05),
        ("surface_container_highest", 1.16),
        ("surface_variant", 0.48),
    ];

    for (key, multiplier) in surface_keys {
        let Some(group) = colors.get_mut(key).and_then(Value::as_object_mut) else {
            continue;
        };

        for (variant, primary_hex) in &primary_by_variant {
            let Some(color_obj) = group.get_mut(*variant).and_then(Value::as_object_mut) else {
                continue;
            };
            let Some(base_hex) = read_hex_from_object(color_obj) else {
                continue;
            };
            let amount = (strength * multiplier).clamp(0.0, 0.18);
            if let Some(tinted) = mix_hex(&base_hex, primary_hex, amount) {
                write_color_object(color_obj, &tinted);
            }
        }
    }
}

fn read_scheme_hex(context: &Value, key: &str, variant: &str) -> Option<String> {
    context
        .get("colors")?
        .get(key)?
        .get(variant)
        .and_then(Value::as_object)
        .and_then(read_hex_from_object)
}

fn read_hex_from_object(object: &serde_json::Map<String, Value>) -> Option<String> {
    for key in ["hex", "color"] {
        let Some(value) = object.get(key).and_then(Value::as_str).map(str::trim) else {
            continue;
        };
        if value.len() == 9 && value.starts_with('#') {
            return Some(format!("#{}", &value[1..7]));
        }
        if value.len() == 7 && value.starts_with('#') {
            return Some(value.to_string());
        }
    }
    None
}

fn mix_hex(base: &str, tint: &str, amount: f64) -> Option<String> {
    let (br, bg, bb) = parse_hex_rgb(base)?;
    let (tr, tg, tb) = parse_hex_rgb(tint)?;
    let mix = |a: u8, b: u8| ((a as f64 * (1.0 - amount)) + (b as f64 * amount)).round() as u8;
    Some(format!(
        "#{:02X}{:02X}{:02X}",
        mix(br, tr),
        mix(bg, tg),
        mix(bb, tb)
    ))
}

fn parse_hex_rgb(hex: &str) -> Option<(u8, u8, u8)> {
    let value = hex.trim().trim_start_matches('#');
    if value.len() < 6 {
        return None;
    }
    Some((
        u8::from_str_radix(&value[0..2], 16).ok()?,
        u8::from_str_radix(&value[2..4], 16).ok()?,
        u8::from_str_radix(&value[4..6], 16).ok()?,
    ))
}

fn write_color_object(object: &mut serde_json::Map<String, Value>, hex: &str) {
    let Some((r, g, b)) = parse_hex_rgb(hex) else {
        return;
    };
    let stripped = hex.trim_start_matches('#');
    object.insert("color".to_string(), Value::String(hex.to_string()));
    object.insert("hex".to_string(), Value::String(hex.to_string()));
    object.insert(
        "hex_stripped".to_string(),
        Value::String(stripped.to_string()),
    );
    object.insert("hex_alpha".to_string(), Value::String(format!("{hex}ff")));
    object.insert(
        "hex_alpha_stripped".to_string(),
        Value::String(format!("{stripped}ff")),
    );
    object.insert(
        "rgb".to_string(),
        Value::String(format!("rgb({r}, {g}, {b})")),
    );
    object.insert(
        "rgba".to_string(),
        Value::String(format!("rgba({r}, {g}, {b}, 255)")),
    );
    object.insert("red".to_string(), Value::String(r.to_string()));
    object.insert("green".to_string(), Value::String(g.to_string()));
    object.insert("blue".to_string(), Value::String(b.to_string()));
}

fn analyse_wallpaper(image_path: &str) -> Result<WallpaperAnalysis, String> {
    let image = image::io::Reader::open(image_path)
        .map_err(|e| e.to_string())?
        .with_guessed_format()
        .map_err(|e| e.to_string())?
        .decode()
        .map_err(|e| e.to_string())?
        .resize(128, 128, image::imageops::FilterType::Triangle)
        .to_rgba8();

    let mut total_luminance = 0.0;
    let mut total_grayscale_delta = 0.0;
    let mut count = 0.0;

    for pixel in image.pixels() {
        if pixel[3] == 0 {
            continue;
        }

        let r = pixel[0] as f64 / 255.0;
        let g = pixel[1] as f64 / 255.0;
        let b = pixel[2] as f64 / 255.0;
        total_luminance += (0.299 * r * r + 0.587 * g * g + 0.114 * b * b).sqrt();
        total_grayscale_delta += (r.max(g).max(b) - r.min(g).min(b)).abs();
        count += 1.0;
    }

    if count == 0.0 {
        return Ok(WallpaperAnalysis {
            luminance: 0.0,
            grayscale_score: 0.0,
        });
    }

    Ok(WallpaperAnalysis {
        luminance: total_luminance / count,
        grayscale_score: total_grayscale_delta / count,
    })
}
