use wasm_bindgen::prelude::*;
use omd2typst_core::{parse_markdown, render_typst, RenderOptions, WikilinkMode, BUILTIN_TEMPLATE};

#[wasm_bindgen]
pub fn render_to_typst(
    markdown: &str,
    template_src: Option<String>,
    wikilinks: Option<String>,
    obsidian_vault: Option<String>,
) -> String {
    let doc = parse_markdown(markdown);
    let options = RenderOptions {
        // Unknown / missing mode falls back to the default (text).
        wikilinks: wikilinks.as_deref().and_then(WikilinkMode::parse).unwrap_or_default(),
        obsidian_vault,
    };
    render_typst(&doc, template_src.as_deref(), &options)
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
        let result = render_to_typst("# Hello", None, None, None);
        assert!(!result.is_empty());
    }

    #[test]
    fn render_to_typst_honours_wikilink_mode() {
        let md = "# T\n\n[[Note|alias]]\n";
        assert!(render_to_typst(md, None, Some("bold".into()), None).contains("#strong[alias]"));
        assert!(render_to_typst(md, None, Some("nonsense".into()), None).contains("alias"));
        assert!(render_to_typst(md, None, Some("keep".into()), None).contains("Note|alias"));
    }

    #[test]
    fn get_builtin_template_returns_non_empty() {
        assert!(!get_builtin_template().is_empty());
    }
}
