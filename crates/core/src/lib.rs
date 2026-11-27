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

/// How `![[annex.pdf]]` embeds are included. `Fit` and `Full` need Typst 0.14 or newer to compile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PdfEmbedMode {
    /// Not included: the "Embed not supported" placeholder (default).
    #[default]
    Off,
    /// Left out silently: nothing is shown for the embed.
    Skip,
    /// One page per PDF page, inside the document's margins, keeping the
    /// document's header and footer.
    Fit,
    /// One full page per PDF page, without margins, header and footer.
    Full,
}

impl PdfEmbedMode {
    /// Parse `off`, `skip`, `fit` or `full` (case-insensitive).
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "off" => Some(Self::Off),
            "skip" => Some(Self::Skip),
            "fit" => Some(Self::Fit),
            "full" => Some(Self::Full),
            _ => None,
        }
    }
}

/// Rendering options.
#[derive(Default, Clone, Debug)]
pub struct RenderOptions {
    /// Syntax of math formulas. Default: [`MathMode::Latex`].
    pub math: MathMode,
    /// How PDF embeds are included. Default: [`PdfEmbedMode::Off`].
    pub pdf_embeds: PdfEmbedMode,
    /// Page count of each embedded PDF, keyed by the path as it appears in the
    /// document (without a `#page=` fragment). Typst cannot count the pages of
    /// a PDF, so the caller supplies them; an embed without `#page=` and
    /// without an entry here stays a placeholder.
    pub pdf_pages: std::collections::HashMap<String, u32>,
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
