//! Narrow adapter: 2021 stays on the vendored engine; 2025 uses a pinned MCU port.
use super::settings::{GenerationSettings, MaterialSpec};
use material_colors::{color::Argb, hct::Hct};
use material_colors_modern::{
    color::Rgb,
    dynamic_color::{
        color_spec::SpecVersion, color_spec_2025::ColorSpec2025, dynamic_scheme::Platform,
        DynamicScheme, Variant,
    },
    scheme::Scheme,
};
use matugen_core::scheme::{get_schemes, SchemeTypes, Schemes};

pub fn schemes(
    source: Argb,
    variant: SchemeTypes,
    settings: &GenerationSettings,
) -> Result<Schemes, String> {
    settings.validate()?;
    let (dark, light) = match settings.material_spec {
        MaterialSpec::V2021 => {
            let (dark, light) = get_schemes(source, &Some(variant), &Some(settings.contrast));
            (
                dark.into_iter().collect::<Vec<_>>(),
                light.into_iter().collect::<Vec<_>>(),
            )
        }
        MaterialSpec::V2025 => (
            modern(source, variant, true, settings.contrast),
            modern(source, variant, false, settings.contrast),
        ),
    };
    Ok(Schemes {
        dark: dark
            .into_iter()
            .map(|(name, color)| {
                let color = adjust(&name, color, settings, true);
                (name, color)
            })
            .collect(),
        light: light
            .into_iter()
            .map(|(name, color)| {
                let color = adjust(&name, color, settings, false);
                (name, color)
            })
            .collect(),
    })
}
fn modern(source: Argb, variant: SchemeTypes, dark: bool, contrast: f64) -> Vec<(String, Argb)> {
    let variant = match variant {
        SchemeTypes::SchemeContent => Variant::Content,
        SchemeTypes::SchemeExpressive => Variant::Expressive,
        SchemeTypes::SchemeFidelity => Variant::Fidelity,
        SchemeTypes::SchemeFruitSalad => Variant::FruitSalad,
        SchemeTypes::SchemeMonochrome => Variant::Monochrome,
        SchemeTypes::SchemeNeutral => Variant::Neutral,
        SchemeTypes::SchemeRainbow => Variant::Rainbow,
        SchemeTypes::SchemeTonalSpot => Variant::TonalSpot,
        SchemeTypes::SchemeVibrant => Variant::Vibrant,
    };
    let mut scheme = DynamicScheme::by_variant(
        Rgb::new(source.red, source.green, source.blue),
        &variant,
        dark,
        Some(contrast),
    )
    .with_spec_version(SpecVersion::Spec2025);
    // with_spec_version selects roles, but does not rebuild the source palettes.
    // Use the engine's 2025 palette constructors as well.
    let hct = scheme.source_color_hct;
    // 2025 changes these four palettes. Other variants retain their existing
    // constructors: upstream's new 2021 fallback helpers are not implemented.
    if matches!(
        variant,
        Variant::Neutral | Variant::TonalSpot | Variant::Expressive | Variant::Vibrant
    ) {
        scheme.primary_palette = ColorSpec2025::get_primary_palette(
            variant.clone(),
            hct,
            dark,
            Platform::Phone,
            contrast,
        );
        scheme.secondary_palette = ColorSpec2025::get_secondary_palette(
            variant.clone(),
            hct,
            dark,
            Platform::Phone,
            contrast,
        );
        scheme.tertiary_palette = ColorSpec2025::get_tertiary_palette(
            variant.clone(),
            hct,
            dark,
            Platform::Phone,
            contrast,
        );
        scheme.neutral_palette = ColorSpec2025::get_neutral_palette(
            variant.clone(),
            hct,
            dark,
            Platform::Phone,
            contrast,
        );
        scheme.neutral_variant_palette = ColorSpec2025::get_neutral_variant_palette(
            variant.clone(),
            hct,
            dark,
            Platform::Phone,
            contrast,
        );
        scheme.error_palette =
            ColorSpec2025::get_error_palette(variant.clone(), hct, dark, Platform::Phone, contrast);
    }
    Scheme::from(scheme)
        .into_iter()
        .map(|(name, rgb)| {
            (
                name.to_string(),
                Argb::new(255, rgb.red, rgb.green, rgb.blue),
            )
        })
        .collect()
}
fn adjust(name: &str, color: Argb, settings: &GenerationSettings, dark: bool) -> Argb {
    if settings.chroma == 1.0 && settings.tone == 1.0 {
        return color;
    }
    let hct = Hct::new(color);
    // Background roles only: leave foreground tone, outline, shadows and scrim alone.
    let background = !name.starts_with("on_")
        && (matches!(
            name,
            "background"
                | "surface"
                | "surface_dim"
                | "surface_bright"
                | "surface_variant"
                | "inverse_surface"
                | "primary"
                | "secondary"
                | "tertiary"
                | "error"
        ) || name.starts_with("surface_container")
            || name.ends_with("_container") && !name.starts_with("on_")
            || name.ends_with("_fixed")
            || name.ends_with("_fixed_dim"));
    let tone = if background {
        (hct.get_tone() * settings.tone.max(if dark { 0.0 } else { 0.5 })).clamp(0.0, 100.0)
    } else {
        hct.get_tone()
    };
    Argb::from(Hct::from(
        hct.get_hue(),
        hct.get_chroma() * settings.chroma,
        tone,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    const SOURCE: Argb = Argb::new(255, 66, 133, 244);
    #[test]
    fn legacy_defaults_are_exact() {
        let settings = GenerationSettings {
            material_spec: MaterialSpec::V2021,
            ..Default::default()
        };
        let actual = schemes(SOURCE, SchemeTypes::SchemeContent, &settings).unwrap();
        let (dark, light) = get_schemes(SOURCE, &Some(SchemeTypes::SchemeContent), &Some(0.0));
        assert_eq!(
            actual.dark.into_iter().collect::<Vec<_>>(),
            dark.into_iter().collect::<Vec<_>>()
        );
        assert_eq!(
            actual.light.into_iter().collect::<Vec<_>>(),
            light.into_iter().collect::<Vec<_>>()
        );
    }
    #[test]
    fn supported_specs_and_all_variants_generate_real_colors() {
        for spec in [MaterialSpec::V2021, MaterialSpec::V2025] {
            for variant in [
                SchemeTypes::SchemeContent,
                SchemeTypes::SchemeExpressive,
                SchemeTypes::SchemeFidelity,
                SchemeTypes::SchemeFruitSalad,
                SchemeTypes::SchemeMonochrome,
                SchemeTypes::SchemeNeutral,
                SchemeTypes::SchemeRainbow,
                SchemeTypes::SchemeTonalSpot,
                SchemeTypes::SchemeVibrant,
            ] {
                for contrast in [-1.0, 0.0, 1.0] {
                    let result = schemes(
                        SOURCE,
                        variant,
                        &GenerationSettings {
                            material_spec: spec,
                            contrast,
                            ..Default::default()
                        },
                    )
                    .unwrap();
                    assert!(result.dark.len() >= 40);
                    assert_ne!(result.dark["on_surface"], result.dark["surface"]);
                    assert_ne!(result.light["on_surface"], result.light["surface"]);
                }
            }
        }
    }
    #[test]
    fn controls_change_schemes() {
        let base = schemes(
            SOURCE,
            SchemeTypes::SchemeTonalSpot,
            &GenerationSettings::default(),
        )
        .unwrap();
        for settings in [
            GenerationSettings {
                contrast: 1.0,
                ..Default::default()
            },
            GenerationSettings {
                chroma: 0.2,
                ..Default::default()
            },
            GenerationSettings {
                tone: 0.7,
                ..Default::default()
            },
            GenerationSettings {
                material_spec: MaterialSpec::V2021,
                ..Default::default()
            },
        ] {
            let changed = schemes(SOURCE, SchemeTypes::SchemeTonalSpot, &settings).unwrap();
            assert_ne!(base.dark, changed.dark);
            assert_ne!(base.light, changed.light);
        }
    }
    #[test]
    fn multiplier_extremes_are_in_gamut() {
        for chroma in [0.0, 1.0, 10.0] {
            for tone in [0.0, 1.0, 1.5] {
                let result = schemes(
                    SOURCE,
                    SchemeTypes::SchemeVibrant,
                    &GenerationSettings {
                        chroma,
                        tone,
                        ..Default::default()
                    },
                )
                .unwrap();
                for color in result.dark.values().chain(result.light.values()) {
                    assert_eq!(color.alpha, 255);
                    assert!((-1e-9..=100.0 + 1e-9).contains(&Hct::new(*color).get_tone()));
                }
            }
        }
    }
}
