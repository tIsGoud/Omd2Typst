use wasm_bindgen::prelude::*;
use omd2typst_core::{parse_markdown, render_typst, RenderOptions, BUILTIN_TEMPLATE};

#[wasm_bindgen]
pub fn render_to_typst(markdown: &str, template_src: Option<String>) -> String {
    let doc = parse_markdown(markdown);
    render_typst(&doc, template_src.as_deref(), &RenderOptions::default())
}

#[wasm_bindgen]
pub fn get_builtin_template() -> String {
    BUILTIN_TEMPLATE.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_to_typst_returns_non_empty() {
        let result = render_to_typst("# Hello", None);
        assert!(!result.is_empty());
    }

    #[test]
    fn get_builtin_template_returns_non_empty() {
        assert!(!get_builtin_template().is_empty());
    }
}
