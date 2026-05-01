use matugen_core::{
    color::color::{get_source_color, Source},
    scheme::{get_schemes, get_custom_color_schemes, SchemeTypes, SchemesEnum},
    color::base16::{generate_base16_schemes, Backend},
    helpers::merge_json_source,
    util::arguments::FilterType,
};
use material_colors::theme::ThemeBuilder;

#[tauri::command]
pub fn generate_scheme_from_image(image_path: String, scheme_type: String) -> Result<serde_json::Value, String> {
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
        &source_color_index
    ).map_err(|e| format!("Error getting source color: {}", e))?;

    let theme = ThemeBuilder::with_source(source_color).build();
    let contrast = Some(0.0);
    
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
    schemes.light.insert("source_color".to_owned(), source_color);

    let base16 = generate_base16_schemes(&source, Backend::Wal)
        .map_err(|e| e.to_string())?;

    let json = merge_json_source(
        serde_json::json!({
            "source_color_hex": source_color.to_hex_with_pound(),
        }),
        &Some(schemes),
        &Some(base16),
        &Some(theme),
        SchemesEnum::Dark,
    ).map_err(|e| e.to_string())?;

    Ok(json)
}
