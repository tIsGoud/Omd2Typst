pub mod ast;
mod parser;
mod renderer;

pub use ast::{
    Block, ColAlign, Document, FrontmatterValue,
    Inline, ListItem, TableRow,
};
pub use parser::parse_markdown;
pub use renderer::{render_typst, BUILTIN_TEMPLATE};

/// Rendering options — currently empty, extended as consumers reveal needs.
/// Present now so the render_typst() signature is stable when options arrive.
#[derive(Default)]
pub struct RenderOptions {}
