use embedded_graphics::{
    geometry::Size,
    image::ImageRaw,
    mono_font::{mapping::StrGlyphMapping, DecorationDimensions, MonoFont},
};

const CHARS_PER_ROW: u32 = 32;

/// Character ranges for all fonts.
///
/// This consists of two character ranges - ASCII from ' ' to '~', then ISO 8859-1 from `&nbsp;`
/// (HTML notation) to `ÿ`. Unknown characters fall back to `?`.
const GLYPH_MAPPING: StrGlyphMapping =
    StrGlyphMapping::new("\0 ~\0\u{00A0}ÿ", '?' as usize - ' ' as usize);

/// The 40 point size with a character size of 26x50 pixels.
pub const FIRA_CODE_40: MonoFont = MonoFont {
    image: ImageRaw::new_binary(include_bytes!("../FiraCode40.raw"), CHARS_PER_ROW * 26),

    glyph_mapping: &GLYPH_MAPPING,
    character_size: Size::new(26, 50),
    character_spacing: 0,
    baseline: 6,
    underline: DecorationDimensions::new(18, 1),
    strikethrough: DecorationDimensions::new(13, 1),
};

/// The 30 point size with a character size of 20x38 pixels.
pub const FIRA_CODE_30: MonoFont = MonoFont {
    image: ImageRaw::new_binary(include_bytes!("../FiraCode30.raw"), CHARS_PER_ROW * 20),

    glyph_mapping: &GLYPH_MAPPING,
    character_size: Size::new(20, 38),
    character_spacing: 0,
    baseline: 6,
    underline: DecorationDimensions::new(15, 1),
    strikethrough: DecorationDimensions::new(10, 1),
};
