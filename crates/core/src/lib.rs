pub mod ast;
mod parser;
mod renderer;

pub use ast::{
    Block, ColAlign, Document, FrontmatterValue,
    Inline, ListItem, TableRow,
};
pub use parser::parse_markdown;
pub use renderer::{
    appendix_unmatched, collect_unsupported_embeds, numbering_unmatched, reading_guide_unmatched, render_typst,
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

/// Rendering options.
#[derive(Default, Clone, Debug)]
pub struct RenderOptions {
    /// How wikilinks are rendered. Default: [`WikilinkMode::Text`].
    pub wikilinks: WikilinkMode,
    /// Obsidian vault name used to build `obsidian://open` links to other notes
    /// in [`WikilinkMode::Link`]. Without it such links fall back to plain text.
    pub obsidian_vault: Option<String>,
}
