use material_colors::theme::ThemeBuilder;
use matugen_core::{
    color::base16::{generate_base16_schemes, Backend},
    color::color::{get_source_color, Source},
    helpers::merge_json_source,
    scheme::{get_custom_color_schemes, get_schemes, SchemeTypes, SchemesEnum},
    util::arguments::FilterType,
};

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

    let scheme_type_enum = match scheme_type.as_str() {
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
    let contrast = Some(match scheme_type_enum {
        SchemeTypes::SchemeExpressive => 0.15,
        SchemeTypes::SchemeFidelity => 0.08,
        SchemeTypes::SchemeFruitSalad => 0.12,
        SchemeTypes::SchemeMonochrome => 0.18,
        SchemeTypes::SchemeNeutral => -0.04,
        SchemeTypes::SchemeRainbow => 0.14,
        SchemeTypes::SchemeTonalSpot => 0.04,
        SchemeTypes::SchemeVibrant => 0.22,
        SchemeTypes::SchemeContent => 0.0,
    });
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
    );
    schemes.dark.insert("source_color".to_owned(), source_color);
    schemes
        .light
        .insert("source_color".to_owned(), source_color);

    let base16 = generate_base16_schemes(&source, Backend::Wal).map_err(|e| e.to_string())?;

    let json = merge_json_source(
        serde_json::json!({
            "source_color_hex": source_color.to_hex_with_pound(),
        }),
        &Some(schemes),
        &Some(base16),
        &Some(theme),
        SchemesEnum::Dark,
    )
    .map_err(|e| e.to_string())?;

    Ok(json)
}
