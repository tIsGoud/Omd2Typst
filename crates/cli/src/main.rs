mod world;

use omd2typst_core::{parse_markdown, render_typst, RenderOptions, BUILTIN_TEMPLATE};

use clap::{Parser, ValueEnum};
use anyhow::{Context, Result};
use std::fs;
use std::process::Command;

#[derive(Parser)]
#[command(
    name = "omd2typst",
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

    // Template path is resolved relative to the output directory.
    // For PDF output the intermediate .typ lives in the same directory,
    // so the relative path is identical — no second resolve needed.
    let template_rel = cli.template.as_deref()
        .map(|p| resolve_template_path(&output_path, p))
        .transpose()?;

    let doc = parse_markdown(&input);
    let typst_src = render_typst(&doc, template_rel.as_deref(), &RenderOptions::default());

    match format {
        Format::Typst => {
            fs::write(&output_path, &typst_src)
                .with_context(|| format!("Cannot write: {}", output_path))?;
        }
        Format::Pdf => {
            // Write to intermediate<timestamp>.typ in the same directory as the PDF.
            let intermediate = intermediate_typ_path(&output_path);
            fs::write(&intermediate, &typst_src)
                .with_context(|| format!("Cannot write intermediate file: {}", intermediate.display()))?;

            let status = Command::new("typst")
                .args(["compile"])
                .arg(&intermediate)
                .arg(&output_path)
                .status()
                .context("Failed to run `typst compile` — is typst installed?")?;

            fs::remove_file(&intermediate).ok();

            if !status.success() {
                anyhow::bail!("`typst compile` failed with status {}", status);
            }
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Path helpers
// ---------------------------------------------------------------------------

/// Returns the path for the intermediate .typ file:
/// same directory as `output`, filename = intermediate<YYYYMMDDHHmmss>.typ
fn intermediate_typ_path(output: &str) -> std::path::PathBuf {
    let dir = std::path::Path::new(output)
        .parent()
        .unwrap_or(std::path::Path::new("."));
    dir.join(format!("intermediate-{}.typ", current_timestamp()))
}

/// Returns the current UTC time formatted as YYYYMMDDHHmmss.
/// Uses Howard Hinnant's civil_from_days algorithm — no external crate needed.
fn current_timestamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};

    let total_secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0) as i64;

    let ss   = total_secs % 60;
    let mm   = (total_secs / 60) % 60;
    let hh   = (total_secs / 3600) % 24;
    let days = total_secs / 86400;

    // civil_from_days: converts days-since-epoch to (year, month, day)
    let z   = days + 719_468;
    let era = (if z >= 0 { z } else { z - 146_096 }) / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y   = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp  = (5 * doy + 2) / 153;
    let d   = doy - (153 * mp + 2) / 5 + 1;
    let m   = if mp < 10 { mp + 3 } else { mp - 9 };
    let y   = if m <= 2 { y + 1 } else { y };

    format!("{:04}{:02}{:02}{:02}{:02}{:02}", y, m, d, hh, mm, ss)
}

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

/// Returns a path to the template relative to the output .typ file's directory.
/// Typst resolves #import paths relative to the importing file, not the cwd.
fn resolve_template_path(output: &str, template: &str) -> Result<String> {
    use std::path::{Component, Path};

    let template_abs = Path::new(template)
        .canonicalize()
        .with_context(|| format!("Template file not found: {}", template))?;

    let output_dir = Path::new(output).parent().unwrap_or(Path::new("."));
    let output_dir_abs = output_dir
        .canonicalize()
        .unwrap_or_else(|_| std::env::current_dir().unwrap_or_default());

    let from: Vec<_> = output_dir_abs.components().collect();
    let to: Vec<_>   = template_abs.components().collect();
    let common = from.iter().zip(to.iter()).take_while(|(a, b)| a == b).count();

    let up = from.len() - common;
    let mut parts: Vec<String> = (0..up).map(|_| "..".into()).collect();
    parts.extend(to[common..].iter().filter_map(|c| match c {
        Component::Normal(s) => Some(s.to_string_lossy().into_owned()),
        _ => None,
    }));

    Ok(if parts.is_empty() { ".".into() } else { parts.join("/") })
}
