use wasm_bindgen::prelude::*;
use omd2typst_core::{parse_markdown, render_typst, MathMode, PdfEmbedMode, RenderOptions, WikilinkMode, BUILTIN_TEMPLATE};
use std::collections::HashMap;

#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn render_to_typst(
    markdown: &str,
    template_src: Option<String>,
    wikilinks: Option<String>,
    obsidian_vault: Option<String>,
    math: Option<String>,
    filename_captions: Option<bool>,
    image_reference_width: Option<u32>,
    pdf_embeds: Option<String>,
    pdf_pages: Option<String>,
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
        // Unknown / missing mode falls back to the default (off: placeholder).
        pdf_embeds: pdf_embeds.as_deref().and_then(PdfEmbedMode::parse).unwrap_or_default(),
        pdf_pages: parse_pdf_pages(pdf_pages.as_deref().unwrap_or_default()),
    };
    render_typst(&doc, template_src.as_deref(), &options)
}

/// Page counts as lines of `<count>\t<path>` (tab-separated, one PDF per line).
fn parse_pdf_pages(text: &str) -> HashMap<String, u32> {
    text.lines()
        .filter_map(|line| {
            let (count, path) = line.split_once('\t')?;
            Some((path.to_string(), count.trim().parse().ok()?))
        })
        .collect()
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
        let result = render_to_typst("# Hello", None, None, None, None, None, None, None, None);
        assert!(!result.is_empty());
    }

    #[test]
    fn render_to_typst_honours_wikilink_mode() {
        let md = "# T\n\n[[Note|alias]]\n";
        assert!(render_to_typst(md, None, Some("bold".into()), None, None, None, None, None, None).contains("#strong[alias]"));
        assert!(render_to_typst(md, None, Some("nonsense".into()), None, None, None, None, None, None).contains("alias"));
        assert!(render_to_typst(md, None, Some("keep".into()), None, None, None, None, None, None).contains("Note|alias"));
    }

    #[test]
    fn render_to_typst_honours_math_syntax() {
        let md = "# T\n\n$\\log$ and $log(x)$\n";
        assert!(render_to_typst(md, None, None, None, None, None, None, None, None).contains("$log$"));
        assert!(render_to_typst(md, None, None, None, Some("typst".into()), None, None, None, None).contains("$\\log$"));
        assert!(render_to_typst(md, None, None, None, Some("nonsense".into()), None, None, None, None).contains("$log$"));
    }

    #[test]
    fn render_to_typst_honours_filename_captions() {
        let md = "# T\n\n![[_assets/laptop.png]]\n";
        assert!(!render_to_typst(md, None, None, None, None, None, None, None, None).contains("caption: [laptop]"));
        assert!(render_to_typst(md, None, None, None, None, Some(true), None, None, None).contains("caption: [laptop]"));
    }

    #[test]
    fn render_to_typst_converts_image_widths() {
        let md = "# T\n\n![[a.png|350]]\n";
        assert!(render_to_typst(md, None, None, None, None, None, None, None, None).contains("width: 50%"));
        assert!(render_to_typst(md, None, None, None, None, None, Some(0), None, None).contains("width: 350pt"));
    }

    #[test]
    fn render_to_typst_includes_pdf_pages() {
        let md = "# T\n\n![[/A/annex.pdf]]\n";
        let pages = Some("2\t/A/annex.pdf\n".to_string());
        let fit = render_to_typst(md, None, None, None, None, None, None, Some("fit".into()), pages.clone());
        assert!(fit.contains("page: 2,") && !fit.contains("page: 3,"));
        assert!(render_to_typst(md, None, None, None, None, None, None, None, pages).contains("Embed not supported:"));
    }

    #[test]
    fn get_builtin_template_returns_non_empty() {
        assert!(!get_builtin_template().is_empty());
    }
}
