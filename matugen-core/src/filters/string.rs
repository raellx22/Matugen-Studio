use colorsys::Rgb;
use convert_case::{Case, Casing};

use crate::{
    expect_args,
    parser::{engine::format_color, Engine, FilterError, FilterReturnType, SpannedValue},
};

fn stringify_color(color: Rgb, keywords: &[&str]) -> Result<String, FilterError> {
    let format = keywords
        .last()
        .ok_or(FilterError::MissingColorFormat)?;
    format_color(color, format)
        .map(|value| value.to_string())
        .ok_or_else(|| FilterError::InvalidColorFormat {
            format: (*format).to_string(),
        })
}

pub(crate) fn replace(
    keywords: &[&str],
    args: &[SpannedValue],
    original: FilterReturnType,
    _engine: &Engine,
) -> Result<FilterReturnType, FilterError> {
    let (find, replace) = expect_args!(args, String, String);

    match original {
        FilterReturnType::String(s) => Ok(FilterReturnType::String(s.replace(&find, &replace))),
        FilterReturnType::Rgb(color) => {
            let modified = stringify_color(color, keywords)?.replace(&find, &replace);
            Ok(FilterReturnType::String(modified))
        }
        FilterReturnType::Hsl(color) => {
            let modified = stringify_color(color.into(), keywords)?.replace(&find, &replace);
            Ok(FilterReturnType::String(modified))
        }
        FilterReturnType::Bool(boolean) => match boolean {
            true => Ok(FilterReturnType::String("true".replace(&find, &replace))),
            false => Ok(FilterReturnType::String("false".replace(&find, &replace))),
        },
    }
}

pub(crate) fn lower_case(
    keywords: &[&str],
    _args: &[SpannedValue],
    original: FilterReturnType,
    _engine: &Engine,
) -> Result<FilterReturnType, FilterError> {
    match original {
        FilterReturnType::String(s) => Ok(FilterReturnType::String(s.to_case(Case::Lower))),
        FilterReturnType::Rgb(color) => {
            let string =
                stringify_color(color, keywords)?;
            Ok(FilterReturnType::String(
                string.to_string().to_case(Case::Lower),
            ))
        }
        FilterReturnType::Hsl(color) => {
            let string =
                stringify_color(color.into(), keywords)?;
            Ok(FilterReturnType::String(
                string.to_string().to_case(Case::Lower),
            ))
        }
        FilterReturnType::Bool(boolean) => match boolean {
            true => Ok(FilterReturnType::String(
                "true".to_string().to_case(Case::Lower),
            )),
            false => Ok(FilterReturnType::String(
                "false".to_string().to_case(Case::Lower),
            )),
        },
    }
}

pub(crate) fn camel_case(
    keywords: &[&str],
    _args: &[SpannedValue],
    original: FilterReturnType,
    _engine: &Engine,
) -> Result<FilterReturnType, FilterError> {
    match original {
        FilterReturnType::String(s) => Ok(FilterReturnType::String(s.to_case(Case::Camel))),
        FilterReturnType::Rgb(color) => {
            let string =
                stringify_color(color, keywords)?;
            Ok(FilterReturnType::String(
                string.to_string().to_case(Case::Camel),
            ))
        }
        FilterReturnType::Hsl(color) => {
            let string =
                stringify_color(color.into(), keywords)?;
            Ok(FilterReturnType::String(
                string.to_string().to_case(Case::Camel),
            ))
        }
        FilterReturnType::Bool(boolean) => match boolean {
            true => Ok(FilterReturnType::String(
                "true".to_string().to_case(Case::Camel),
            )),
            false => Ok(FilterReturnType::String(
                "false".to_string().to_case(Case::Camel),
            )),
        },
    }
}

pub(crate) fn pascal_case(
    keywords: &[&str],
    _args: &[SpannedValue],
    original: FilterReturnType,
    _engine: &Engine,
) -> Result<FilterReturnType, FilterError> {
    match original {
        FilterReturnType::String(s) => Ok(FilterReturnType::String(s.to_case(Case::Pascal))),
        FilterReturnType::Rgb(color) => {
            let string =
                stringify_color(color, keywords)?;
            Ok(FilterReturnType::String(
                string.to_string().to_case(Case::Pascal),
            ))
        }
        FilterReturnType::Hsl(color) => {
            let string =
                stringify_color(color.into(), keywords)?;
            Ok(FilterReturnType::String(
                string.to_string().to_case(Case::Pascal),
            ))
        }
        FilterReturnType::Bool(boolean) => match boolean {
            true => Ok(FilterReturnType::String(
                "true".to_string().to_case(Case::Pascal),
            )),
            false => Ok(FilterReturnType::String(
                "false".to_string().to_case(Case::Pascal),
            )),
        },
    }
}

pub(crate) fn snake_case(
    keywords: &[&str],
    _args: &[SpannedValue],
    original: FilterReturnType,
    _engine: &Engine,
) -> Result<FilterReturnType, FilterError> {
    match original {
        FilterReturnType::String(s) => Ok(FilterReturnType::String(s.to_case(Case::Snake))),
        FilterReturnType::Rgb(color) => {
            let string =
                stringify_color(color, keywords)?;
            Ok(FilterReturnType::String(
                string.to_string().to_case(Case::Snake),
            ))
        }
        FilterReturnType::Hsl(color) => {
            let string =
                stringify_color(color.into(), keywords)?;
            Ok(FilterReturnType::String(
                string.to_string().to_case(Case::Snake),
            ))
        }
        FilterReturnType::Bool(boolean) => match boolean {
            true => Ok(FilterReturnType::String(
                "true".to_string().to_case(Case::Snake),
            )),
            false => Ok(FilterReturnType::String(
                "false".to_string().to_case(Case::Snake),
            )),
        },
    }
}

pub(crate) fn kebab_case(
    keywords: &[&str],
    _args: &[SpannedValue],
    original: FilterReturnType,
    _engine: &Engine,
) -> Result<FilterReturnType, FilterError> {
    match original {
        FilterReturnType::String(s) => Ok(FilterReturnType::String(s.to_case(Case::Kebab))),
        FilterReturnType::Rgb(color) => {
            let string =
                stringify_color(color, keywords)?;
            Ok(FilterReturnType::String(
                string.to_string().to_case(Case::Kebab),
            ))
        }
        FilterReturnType::Hsl(color) => {
            let string =
                stringify_color(color.into(), keywords)?;
            Ok(FilterReturnType::String(
                string.to_string().to_case(Case::Kebab),
            ))
        }
        FilterReturnType::Bool(boolean) => match boolean {
            true => Ok(FilterReturnType::String(
                "true".to_string().to_case(Case::Kebab),
            )),
            false => Ok(FilterReturnType::String(
                "false".to_string().to_case(Case::Kebab),
            )),
        },
    }
}
