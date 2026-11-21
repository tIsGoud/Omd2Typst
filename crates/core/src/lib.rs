pub mod ast;
mod math;
mod parser;
mod renderer;

pub use ast::{
    Block, ColAlign, Document, FrontmatterValue,
    HighlightColor, Inline, ListItem, TableRow,
};
pub use parser::parse_markdown;
pub use renderer::{
    appendix_unmatched, collect_unsupported_embeds, numbering_unmatched, reading_guide_unmatched, render_typst,
    take_math_failures,
    BUILTIN_TEMPLATE,
};

/// How Obsidian wikilinks (`[[Note]]`, `[[Note|alias]]`, `[[#Heading]]`) are rendered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WikilinkMode {
    /// Display text only, no brackets (default).
    #[default]
    Text,
    /// Display text in bold.
    Bold,
    /// Real links: `[[#Heading]]` becomes an internal link to that heading,
    /// links to other notes become `obsidian://` links when a vault name is
    /// given (otherwise plain text).
    Link,
    /// Leave the wikilink as typed, brackets included.
    Keep,
}

impl WikilinkMode {
    /// Parse `text`, `bold`, `link` or `keep` (case-insensitive).
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "text" => Some(Self::Text),
            "bold" => Some(Self::Bold),
            "link" => Some(Self::Link),
            "keep" => Some(Self::Keep),
            _ => None,
        }
    }
}

/// Syntax of the `$…$` and `$$…$$` formulas in the Markdown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MathMode {
    /// LaTeX, as Obsidian renders it (default); converted to Typst math.
    #[default]
    Latex,
    /// Typst math syntax, passed through unchanged.
    Typst,
}

impl MathMode {
    /// Parse `latex` or `typst` (case-insensitive).
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "latex" => Some(Self::Latex),
            "typst" => Some(Self::Typst),
            _ => None,
        }
    }
}

/// Default for [`RenderOptions::image_reference_width`]: Obsidian's default
/// readable line length in pixels.
pub const DEFAULT_IMAGE_REFERENCE_WIDTH: u32 = 700;

/// Rendering options.
#[derive(Default, Clone, Debug)]
pub struct RenderOptions {
    /// Syntax of math formulas. Default: [`MathMode::Latex`].
    pub math: MathMode,
    /// Width, in Obsidian pixels, that counts as the full text width when
    /// image widths (`![[img|400]]`) are converted. A width of 400 with the
    /// default 700 (Obsidian's readable line length) becomes 57% of the text
    /// width. `None` means [`DEFAULT_IMAGE_REFERENCE_WIDTH`]; `Some(0)` treats
    /// the numbers as points instead.
    pub image_reference_width: Option<u32>,
    /// Use the image's file name as caption when it has no alt text.
    /// Default: off (images without alt text get no caption).
    pub filename_captions: bool,
    /// How wikilinks are rendered. Default: [`WikilinkMode::Text`].
    pub wikilinks: WikilinkMode,
    /// Obsidian vault name used to build `obsidian://open` links to other notes
    /// in [`WikilinkMode::Link`]. Without it such links fall back to plain text.
    pub obsidian_vault: Option<String>,
}
