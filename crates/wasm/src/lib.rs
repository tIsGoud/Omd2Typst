use wasm_bindgen::prelude::*;
use omd2typst_core::{parse_markdown, render_typst, MathMode, RenderOptions, WikilinkMode, BUILTIN_TEMPLATE};

#[wasm_bindgen]
pub fn render_to_typst(
    markdown: &str,
    template_src: Option<String>,
    wikilinks: Option<String>,
    obsidian_vault: Option<String>,
    math: Option<String>,
    filename_captions: Option<bool>,
    image_reference_width: Option<u32>,
) -> String {
    let doc = parse_markdown(markdown);
    let options = RenderOptions {
        // Unknown / missing mode falls back to the default (text).
        wikilinks: wikilinks.as_deref().and_then(WikilinkMode::parse).unwrap_or_default(),
        obsidian_vault,
        // Unknown / missing syntax falls back to the default (latex).
        math: math.as_deref().and_then(MathMode::parse).unwrap_or_default(),
        filename_captions: filename_captions.unwrap_or(false),
        image_reference_width,
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
        let result = render_to_typst("# Hello", None, None, None, None, None, None);
        assert!(!result.is_empty());
    }

    #[test]
    fn render_to_typst_honours_wikilink_mode() {
        let md = "# T\n\n[[Note|alias]]\n";
        assert!(render_to_typst(md, None, Some("bold".into()), None, None, None, None).contains("#strong[alias]"));
        assert!(render_to_typst(md, None, Some("nonsense".into()), None, None, None, None).contains("alias"));
        assert!(render_to_typst(md, None, Some("keep".into()), None, None, None, None).contains("Note|alias"));
    }

    #[test]
    fn render_to_typst_honours_math_syntax() {
        let md = "# T\n\n$\\log$ and $log(x)$\n";
        assert!(render_to_typst(md, None, None, None, None, None, None).contains("$log$"));
        assert!(render_to_typst(md, None, None, None, Some("typst".into()), None, None).contains("$\\log$"));
        assert!(render_to_typst(md, None, None, None, Some("nonsense".into()), None, None).contains("$log$"));
    }

    #[test]
    fn render_to_typst_honours_filename_captions() {
        let md = "# T\n\n![[_assets/laptop.png]]\n";
        assert!(!render_to_typst(md, None, None, None, None, None, None).contains("caption: [laptop]"));
        assert!(render_to_typst(md, None, None, None, None, Some(true), None).contains("caption: [laptop]"));
    }

    #[test]
    fn render_to_typst_converts_image_widths() {
        let md = "# T\n\n![[a.png|350]]\n";
        assert!(render_to_typst(md, None, None, None, None, None, None).contains("width: 50%"));
        assert!(render_to_typst(md, None, None, None, None, None, Some(0)).contains("width: 350pt"));
    }

    #[test]
    fn get_builtin_template_returns_non_empty() {
        assert!(!get_builtin_template().is_empty());
    }
}
