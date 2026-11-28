mod pdf;
mod world;

use omd2typst_core::{
    appendix_unmatched, collect_unsupported_embeds, numbering_unmatched, parse_markdown, reading_guide_unmatched,
    render_typst, take_math_failures, MathMode, PdfEmbedMode, RenderOptions, WikilinkMode,
    BUILTIN_TEMPLATE,
};

use clap::{Parser, ValueEnum};
use anyhow::{Context, Result};
use std::fs;

#[derive(Parser)]
#[command(
    name = "omd2typst",
    version,
    about = "Convert Obsidian Markdown to a Typst file or PDF",
    after_help = "EXAMPLES:
    omd2typst notes.md output.typ
    omd2typst notes.md output.pdf
    omd2typst notes.md output.pdf --template my-template.typ
    omd2typst notes.md output.pdf --wikilinks link --obsidian-vault \"My Vault\"
    omd2typst notes.md output.pdf --math typst
    omd2typst --export-template my-template.typ"
)]
struct Cli {
    /// Input Markdown file
    #[arg(required_unless_present = "export_template")]
    input: Option<String>,

    /// Output file (.typ or .pdf)
    #[arg(required_unless_present = "export_template")]
    output: Option<String>,

    /// Output format — inferred from the output file extension if omitted
    #[arg(short, long)]
    format: Option<Format>,

    /// Typst template file for styling. Must export `template` and `callout`.
    /// Use --export-template to get a starting point.
    #[arg(short, long)]
    template: Option<String>,

    /// How [[wikilinks]] are rendered: `text` (display text only), `bold`,
    /// `link` (internal links to headings; links to other notes need
    /// --obsidian-vault) or `keep` (leave the brackets as typed).
    #[arg(long, value_enum, default_value_t = Wikilinks::Text)]
    wikilinks: Wikilinks,

    /// Obsidian vault name. With `--wikilinks link`, links to other notes become
    /// obsidian://open links (the vault name is embedded in the PDF).
    #[arg(long, value_name = "NAME")]
    obsidian_vault: Option<String>,

    /// Syntax of `$…$` / `$$…$$` formulas: `latex` (as Obsidian renders it;
    /// converted to Typst math) or `typst` (Typst math, passed through).
    #[arg(long, value_enum, default_value_t = Math::Latex)]
    math: Math,

    /// Width in pixels that counts as the full text width when converting
    /// image widths (`![[img|400]]`). 400 with the default 700 (Obsidian's
    /// readable line length) is 57% of the text width. Use 0 to read the numbers
    /// as points instead.
    #[arg(long, value_name = "PX", default_value_t = 700)]
    image_reference_width: u32,

    /// Use the image file name as caption for images without alt text.
    /// Alt text, when present, always wins.
    #[arg(long)]
    filename_captions: bool,

    /// How PDF embeds (`![[annex.pdf]]`, `![[annex.pdf#page=3]]`) are included:
    /// `fit` (one page per PDF page inside the document's margins, with header
    /// and footer), `full` (full pages without margins, header and footer),
    /// `skip` (left out silently) or `off` (a placeholder note). Needs the PDF
    /// file to be readable from the current directory, like images.
    #[arg(long, value_enum, default_value_t = PdfEmbeds::Fit)]
    pdf_embeds: PdfEmbeds,

    /// Write the built-in styling to FILE as a Typst template and exit.
    /// Edit the file, then pass it back with --template.
    #[arg(long, value_name = "FILE")]
    export_template: Option<String>,
}

#[derive(ValueEnum, Clone, Copy)]
enum Math {
    Latex,
    Typst,
}

impl From<Math> for MathMode {
    fn from(m: Math) -> Self {
        match m {
            Math::Latex => MathMode::Latex,
            Math::Typst => MathMode::Typst,
        }
    }
}

#[derive(ValueEnum, Clone, Copy)]
enum PdfEmbeds {
    Off,
    Skip,
    Fit,
    Full,
}

impl From<PdfEmbeds> for PdfEmbedMode {
    fn from(p: PdfEmbeds) -> Self {
        match p {
            PdfEmbeds::Off => PdfEmbedMode::Off,
            PdfEmbeds::Skip => PdfEmbedMode::Skip,
            PdfEmbeds::Fit => PdfEmbedMode::Fit,
            PdfEmbeds::Full => PdfEmbedMode::Full,
        }
    }
}

#[derive(ValueEnum, Clone, Copy)]
enum Wikilinks {
    Text,
    Bold,
    Link,
    Keep,
}

impl From<Wikilinks> for WikilinkMode {
    fn from(w: Wikilinks) -> Self {
        match w {
            Wikilinks::Text => WikilinkMode::Text,
            Wikilinks::Bold => WikilinkMode::Bold,
            Wikilinks::Link => WikilinkMode::Link,
            Wikilinks::Keep => WikilinkMode::Keep,
        }
    }
}

#[derive(ValueEnum, Clone)]
enum Format {
    Typst,
    Pdf,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // --export-template writes the built-in template to disk and exits.
    if let Some(path) = &cli.export_template {
        fs::write(path, BUILTIN_TEMPLATE)
            .with_context(|| format!("Cannot write template to: {}", path))?;
        println!("Template written to: {}", path);
        println!("Edit it and use it with:  omd2typst input.md output.pdf --template {}", path);
        return Ok(());
    }

    let input_path = cli.input.as_deref()
        .ok_or_else(|| anyhow::anyhow!("input file is required"))?;
    let requested_output = cli.output.as_deref()
        .ok_or_else(|| anyhow::anyhow!("output file is required"))?;

    let format = cli.format.clone().unwrap_or_else(|| {
        if requested_output.ends_with(".pdf") { Format::Pdf } else { Format::Typst }
    });

    // For .typ output: detect collision with the template and rename with a warning.
    let output_path: String = match &format {
        Format::Typst => {
            if let Some(tmpl) = cli.template.as_deref() {
                if paths_are_same(requested_output, tmpl) {
                    let renamed = prefix_filename(requested_output, "output-");
                    eprintln!(
                        "Warning: output file '{}' has the same name as the template '{}'. \
                         A Typst file cannot import itself. \
                         Output renamed to '{}'.",
                        requested_output, tmpl, renamed
                    );
                    renamed
                } else {
                    requested_output.to_string()
                }
            } else {
                requested_output.to_string()
            }
        }
        Format::Pdf => requested_output.to_string(),
    };

    let input = fs::read_to_string(input_path)
        .with_context(|| format!("Cannot read input file: {}", input_path))?;

    let doc = parse_markdown(&input);

    // PDF embeds: Typst cannot count the pages of a PDF, so read the counts here.
    let pdf_mode: PdfEmbedMode = cli.pdf_embeds.into();
    let root = std::env::current_dir().context("Cannot determine current directory")?;
    let embeds = collect_unsupported_embeds(&doc);
    let pdf_pages = pdf::page_counts(&embeds, pdf_mode, &root, |w| eprintln!("warning: {w}"));

    // Warn about embeds that cannot be rendered (PDFs that are switched off,
    // note transclusions, audio/video, Obsidian Bases — the latter only by the
    // Obsidian plugin); the document shows an inline placeholder for each.
    for src in &embeds {
        let path = src.split_once('#').map_or(src.as_str(), |(p, _)| p).trim_end().to_ascii_lowercase();
        if path.ends_with(".pdf") {
            if pdf_mode == PdfEmbedMode::Off {
                eprintln!(
                    "warning: PDF embed '{src}' is switched off; inserted a placeholder note instead \
                     (use --pdf-embeds fit or full to include it)."
                );
            }
            // Other modes: included, or already reported when the file cannot be read.
        } else if path.ends_with(".base") {
            eprintln!(
                "warning: embed '{src}' is not supported; inserted a placeholder note instead \
                 (Obsidian Bases embeds are only rendered by the Omd2Typst Obsidian plugin)."
            );
        } else {
            eprintln!("warning: embed '{src}' is not supported; inserted a placeholder note instead.");
        }
    }

    // Warn when `appendix-from` names a chapter that does not exist.
    if let Some(name) = appendix_unmatched(&doc) {
        eprintln!(
            "warning: appendix-from: '{}' did not match any top-level chapter; \
             appendix numbering was not applied.",
            name
        );
    }

    if cli.obsidian_vault.is_some() && !matches!(cli.wikilinks, Wikilinks::Link) {
        eprintln!("warning: --obsidian-vault has no effect without --wikilinks link.");
    }

    // Warn when `numbering-from` names a chapter that does not exist.
    if let Some(name) = numbering_unmatched(&doc) {
        eprintln!(
            "warning: numbering-from: '{}' did not match any top-level chapter; \
             numbering starts from the beginning.",
            name
        );
    }

    // Warn when `reading-guide` names an H2 section that does not exist.
    if let Some(name) = reading_guide_unmatched(&doc) {
        eprintln!(
            "warning: reading-guide: '{}' did not match any '## ' section; \
             no reading-guide page was created.",
            name
        );
    }

    let typst_src = render_typst(
        &doc,
        cli.template.as_deref(),
        &RenderOptions {
            math: cli.math.into(),
            filename_captions: cli.filename_captions,
            image_reference_width: Some(cli.image_reference_width),
            wikilinks: cli.wikilinks.into(),
            obsidian_vault: cli.obsidian_vault.clone(),
            pdf_embeds: pdf_mode,
            pdf_pages,
        },
    );

    // Formulas the LaTeX converter could not handle are passed through as typed.
    for failure in take_math_failures() {
        eprintln!(
            "warning: math: could not convert LaTeX formula ({}); \
             passed through as typed. Use --math typst for Typst math syntax.",
            failure
        );
    }

    match format {
        Format::Typst => {
            fs::write(&output_path, &typst_src)
                .with_context(|| format!("Cannot write: {}", output_path))?;
        }
        Format::Pdf => {
            let world = world::OmdWorld::new(root, typst_src);
            let result = typst::compile::<typst_layout::PagedDocument>(&world);

            for warning in &result.warnings {
                eprintln!("warning: {}", warning.message);
            }

            let document = result.output.map_err(|errors| {
                let msg = errors
                    .iter()
                    .map(|e| {
                        let file = e.span.id()
                            .map(|id| id.vpath().get_with_slash().to_string())
                            .unwrap_or_else(|| "?".to_string());
                        format!("  {file}: {}", e.message)
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                anyhow::anyhow!("Typst compilation failed:\n{msg}")
            })?;

            let pdf_bytes = typst_pdf::pdf(&document, &typst_pdf::PdfOptions::default())
                .map_err(|errors| {
                    let msg = errors
                        .iter()
                        .map(|e| format!("  {}", e.message))
                        .collect::<Vec<_>>()
                        .join("\n");
                    anyhow::anyhow!("PDF generation failed:\n{msg}")
                })?;

            fs::write(&output_path, pdf_bytes)
                .with_context(|| format!("Cannot write: {}", output_path))?;
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Path helpers
// ---------------------------------------------------------------------------

/// Returns true if `a` and `b` resolve to the same filesystem path.
fn paths_are_same(a: &str, b: &str) -> bool {
    use std::path::Path;
    let abs = |p: &str| {
        Path::new(p).canonicalize().ok()
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_default().join(p))
    };
    abs(a) == abs(b)
}

/// Prepends `prefix` to the filename component of `path`.
/// E.g. prefix_filename("dir/foo.typ", "output") → "dir/outputfoo.typ"
fn prefix_filename(path: &str, prefix: &str) -> String {
    use std::path::Path;
    let p = Path::new(path);
    let filename = p.file_name().and_then(|n| n.to_str()).unwrap_or("output.typ");
    p.with_file_name(format!("{}{}", prefix, filename))
        .to_string_lossy()
        .into_owned()
}

