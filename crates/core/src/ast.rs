/// Colour of a `==highlight==`. A leading coloured dot (`==🔴text==`) selects one
/// of Obsidian's highlight colours; without a dot the default highlight is used.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HighlightColor {
    #[default]
    Default,
    Red,
    Orange,
    Yellow,
    Green,
    Blue,
    Purple,
}

impl HighlightColor {
    /// Maps the coloured dot that starts a highlight to its colour.
    pub fn from_dot(c: char) -> Option<Self> {
        Some(match c {
            '🔴' => Self::Red,
            '🟠' => Self::Orange,
            '🟡' => Self::Yellow,
            '🟢' => Self::Green,
            '🔵' => Self::Blue,
            '🟣' => Self::Purple,
            _ => return None,
        })
    }

    /// Background colour, sampled from Obsidian's highlight colours.
    pub fn fill_hex(self) -> &'static str {
        match self {
            Self::Default => "#fdf1a5",
            Self::Red => "#efbfbb",
            Self::Orange => "#fae0c3",
            Self::Yellow => "#f2e6bd",
            Self::Green => "#d0efcc",
            Self::Blue => "#b8ccf2",
            Self::Purple => "#e3ccf3",
        }
    }
}

#[derive(Debug, Clone)]
pub enum Inline {
    Text(String),
    Bold(Vec<Inline>),
    Italic(Vec<Inline>),
    Strikethrough(Vec<Inline>),
    Highlight { color: HighlightColor, children: Vec<Inline> },
    InlineMath(String),
    DisplayMath(String),
    Footnote(Vec<Inline>),
    Superscript(Vec<Inline>),
    Subscript(Vec<Inline>),
    Code(String),
    Link { text: Vec<Inline>, url: String },
    /// Obsidian wikilink `[[target#fragment|alias]]`. `text` is the display
    /// text (the alias, or `Note`, `Heading` / `Note > Heading` when there is
    /// none). `target` is empty for same-note links (`[[#Heading]]`).
    WikiLink { target: String, fragment: Option<String>, text: Vec<Inline>, aliased: bool },
    Image { alt: String, src: String, width: Option<u32> },
    SoftBreak,
    HardBreak,
}

#[derive(Debug, Clone)]
pub struct TableRow {
    pub cells: Vec<Vec<Inline>>,
}

#[derive(Debug, Clone)]
pub enum ColAlign { Left, Center, Right }

/// A single item inside a bullet or ordered list.
/// `checkbox` holds the character between `[` and `]` when the item
/// starts with Obsidian task syntax (`- [ ]`, `- [x]`, `- [/]`, …).
#[derive(Debug, Clone)]
pub struct ListItem {
    pub checkbox: Option<char>,
    pub blocks: Vec<Block>,
}

#[derive(Debug, Clone)]
pub enum Block {
    Heading { level: u8, inlines: Vec<Inline> },
    Paragraph(Vec<Inline>),
    CodeBlock { lang: String, code: String },
    Table { header: TableRow, rows: Vec<TableRow>, alignments: Vec<ColAlign> },
    Callout { kind: String, title: String, body: Vec<Block> },
    BulletList(Vec<ListItem>),
    OrderedList(Vec<ListItem>),
    BlockQuote(Vec<Block>),
    ThematicBreak,
}

/// A frontmatter value parsed from YAML.
///
/// `Inlines` — a string value parsed as inline Markdown; the renderer emits it
///   as a Typst string literal when it contains only plain text (preserving
///   backward-compat for values used in string comparisons, e.g. `language`),
///   or as a content block `[…]` when it contains bold, italic, etc.
///
/// `Raw` — an already-formatted Typst literal (inline arrays `(…)`).
///
/// `Multiline` — a YAML `|` block scalar; each inner `Vec<Inline>` is one line.
/// Always rendered as a Typst content block with `\` line breaks.
#[derive(Debug)]
pub enum FrontmatterValue {
    Inlines(Vec<Inline>),
    Raw(String),
    Multiline(Vec<Vec<Inline>>),
}

#[derive(Debug)]
pub struct Document {
    pub frontmatter: Vec<(String, FrontmatterValue)>,
    pub blocks: Vec<Block>,
}
