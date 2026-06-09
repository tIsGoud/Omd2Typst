mod world;

use omd2typst_core::{parse_markdown, render_typst, RenderOptions, BUILTIN_TEMPLATE};

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

    /// Write the built-in styling to FILE as a Typst template and exit.
    /// Edit the file, then pass it back with --template.
    #[arg(long, value_name = "FILE")]
    export_template: Option<String>,
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
    let typst_src = render_typst(&doc, cli.template.as_deref(), &RenderOptions::default());

    match format {
        Format::Typst => {
            fs::write(&output_path, &typst_src)
                .with_context(|| format!("Cannot write: {}", output_path))?;
        }
        Format::Pdf => {
            let root = std::env::current_dir()
                .context("Cannot determine current directory")?;
            let world = world::OmdWorld::new(root, typst_src);
            let result = typst::compile::<typst::layout::PagedDocument>(&world);

            for warning in &result.warnings {
                eprintln!("warning: {}", warning.message);
            }

            let document = result.output.map_err(|errors| {
                let msg = errors
                    .iter()
                    .map(|e| {
                        let file = e.span.id()
                            .map(|id| id.vpath().as_rooted_path().display().to_string())
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

