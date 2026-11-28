//! Page counts of embedded PDFs. Typst cannot count the pages of a PDF, but the
//! engine needs the count to emit one image per page (`![[annex.pdf]]`).

use omd2typst_core::PdfEmbedMode;
use std::collections::HashMap;
use std::path::Path;

/// Number of pages in the PDF at `path`, or `None` if it cannot be read or parsed.
pub fn count_pages(path: &Path) -> Option<u32> {
    let data = std::fs::read(path).ok()?;
    let pdf = hayro_syntax::Pdf::new(data).ok()?;
    u32::try_from(pdf.pages().len()).ok()
}

/// The PDF paths in embeds such as `_assets/a.pdf#page=3` (path without fragment, once each).
pub fn pdf_paths(embeds: &[String]) -> Vec<String> {
    let mut paths: Vec<String> = Vec::new();
    for src in embeds {
        let path = src.split_once('#').map_or(src.as_str(), |(p, _)| p).trim_end();
        if path.to_ascii_lowercase().ends_with(".pdf") && !paths.iter().any(|p| p == path) {
            paths.push(path.to_string());
        }
    }
    paths
}

/// Page counts for the PDFs in `embeds`, read relative to `root` (the folder Typst
/// resolves paths against; a leading `/` is relative to it too). Only needed for
/// `fit` and `full`. PDFs that cannot be read are reported through `warn` and left out.
pub fn page_counts(
    embeds: &[String],
    mode: PdfEmbedMode,
    root: &Path,
    mut warn: impl FnMut(String),
) -> HashMap<String, u32> {
    let mut counts = HashMap::new();
    if !matches!(mode, PdfEmbedMode::Fit | PdfEmbedMode::Full) {
        return counts;
    }
    for path in pdf_paths(embeds) {
        match count_pages(&root.join(path.trim_start_matches('/'))) {
            Some(n) => {
                counts.insert(path, n);
            }
            None => warn(format!("cannot read PDF '{path}' to count its pages; embed left as a placeholder.")),
        }
    }
    counts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_the_pages_of_the_example_pdf() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../input");
        assert_eq!(count_pages(&root.join("_assets/example.pdf")), Some(22));
        assert_eq!(count_pages(&root.join("_assets/missing.pdf")), None);
        assert_eq!(count_pages(&root.join("input-en.md")), None);
    }

    #[test]
    fn finds_each_pdf_once_without_fragment() {
        let embeds: Vec<String> = ["a.pdf#page=3", "a.pdf", "b.PDF ", "c.mp4"].map(String::from).to_vec();
        assert_eq!(pdf_paths(&embeds), ["a.pdf", "b.PDF"]);
    }

    #[test]
    fn only_fit_and_full_read_the_files() {
        let embeds = vec!["_assets/example.pdf".to_string()];
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../input");
        assert!(page_counts(&embeds, PdfEmbedMode::Off, &root, |_| {}).is_empty());
        assert!(page_counts(&embeds, PdfEmbedMode::Skip, &root, |_| {}).is_empty());
        assert_eq!(page_counts(&embeds, PdfEmbedMode::Fit, &root, |_| {})["_assets/example.pdf"], 22);
        let mut warned = Vec::new();
        page_counts(&["nope.pdf".to_string()], PdfEmbedMode::Full, &root, |w| warned.push(w));
        assert_eq!(warned.len(), 1);
    }
}
