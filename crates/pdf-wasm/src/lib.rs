use std::collections::HashMap;

use serde_json;
use typst::foundations::Bytes;
use typst::syntax::{FileId, Source, VirtualPath};
use typst::text::{Font, FontBook, FontFamily, FontList, TextElem};
use typst::utils::LazyHash;
use typst::{Library, World};
use wasm_bindgen::prelude::*;

/// Version of the embedded Typst compiler.
pub const TYPST_VERSION: &str = "0.13.1";

struct WasmWorld {
    library: LazyHash<Library>,
    book: LazyHash<FontBook>,
    fonts: Vec<FontSlot>,
    main_id: FileId,
    main_source: Source,
    /// Vault-relative path (no leading slash) → file bytes.
    files: HashMap<String, Vec<u8>>,
}

struct FontSlot {
    data: Bytes,
    index: u32,
}

impl WasmWorld {
    fn new(typ_source: String, files: HashMap<String, Vec<u8>>) -> Self {
        let mut book = FontBook::new();
        let mut fonts: Vec<FontSlot> = Vec::new();

        // Liberation fonts — reliable sans/serif fallback on every platform.
        for (data, index) in liberation_fonts() {
            let bytes = Bytes::new(data);
            if let Some(font) = Font::new(bytes.clone(), index) {
                book.push(font.info().clone());
                fonts.push(FontSlot { data: bytes, index });
            }
        }

        // Fonts bundled with typst-assets (math, serif, monospace).
        for data in typst_assets::fonts() {
            let bytes = Bytes::new(data);
            let mut index = 0u32;
            while let Some(font) = Font::new(bytes.clone(), index) {
                book.push(font.info().clone());
                fonts.push(FontSlot { data: bytes.clone(), index });
                index += 1;
            }
        }

        let main_id = FileId::new(None, VirtualPath::new("main.typ"));
        let main_source = Source::new(main_id, typ_source);

        // Set Liberation Sans as the document default (instead of Libertinus Serif)
        // so documents with no explicit font setting render in a consistent sans-serif face.
        let mut library = Library::builder().build();
        library.styles.set(TextElem::set_font(FontList(vec![
            FontFamily::new("Liberation Sans"),
            FontFamily::new("Libertinus Serif"),
        ])));

        Self {
            library: LazyHash::new(library),
            book: LazyHash::new(book),
            fonts,
            main_id,
            main_source,
            files,
        }
    }
}

impl World for WasmWorld {
    fn library(&self) -> &LazyHash<Library> { &self.library }
    fn book(&self) -> &LazyHash<FontBook> { &self.book }
    fn main(&self) -> FileId { self.main_id }

    fn source(&self, id: FileId) -> typst::diag::FileResult<Source> {
        if id == self.main_id {
            return Ok(self.main_source.clone());
        }
        let key = id.vpath().as_rootless_path().to_string_lossy().into_owned();
        let data = self.files.get(&key).ok_or_else(|| {
            typst::diag::FileError::NotFound(id.vpath().as_rootless_path().into())
        })?;
        let text = String::from_utf8(data.clone())
            .map_err(|_| typst::diag::FileError::InvalidUtf8)?;
        Ok(Source::new(id, text))
    }

    fn file(&self, id: FileId) -> typst::diag::FileResult<Bytes> {
        let key = id.vpath().as_rootless_path().to_string_lossy().into_owned();
        let data = self.files.get(&key).ok_or_else(|| {
            typst::diag::FileError::NotFound(id.vpath().as_rootless_path().into())
        })?;
        Ok(Bytes::new(data.clone()))
    }

    fn font(&self, index: usize) -> Option<Font> {
        let slot = self.fonts.get(index)?;
        Font::new(slot.data.clone(), slot.index)
    }

    fn today(&self, _offset: Option<i64>) -> Option<typst::foundations::Datetime> {
        None
    }
}

fn liberation_fonts() -> impl Iterator<Item = (&'static [u8], u32)> {
    [
        (include_bytes!("../../cli/fonts/LiberationSans-Regular.ttf").as_slice(), 0u32),
        (include_bytes!("../../cli/fonts/LiberationSans-Bold.ttf").as_slice(), 0u32),
        (include_bytes!("../../cli/fonts/LiberationSans-Italic.ttf").as_slice(), 0u32),
        (include_bytes!("../../cli/fonts/LiberationSans-BoldItalic.ttf").as_slice(), 0u32),
        (include_bytes!("../../cli/fonts/LiberationSerif-Regular.ttf").as_slice(), 0u32),
        (include_bytes!("../../cli/fonts/LiberationSerif-Bold.ttf").as_slice(), 0u32),
        (include_bytes!("../../cli/fonts/LiberationSerif-Italic.ttf").as_slice(), 0u32),
        (include_bytes!("../../cli/fonts/LiberationSerif-BoldItalic.ttf").as_slice(), 0u32),
    ]
    .into_iter()
}

/// Return the Typst compiler version embedded in this module.
#[wasm_bindgen]
pub fn typst_version() -> String {
    TYPST_VERSION.to_string()
}

/// Compile Typst source to a PDF byte array.
///
/// `typ_source`        — full Typst source document (output of render_to_typst).
/// `files_json`        — JSON object mapping vault-relative paths to UTF-8 file
///                       contents (e.g. `{ "typst/my-template.typ": "..." }`).
///                       Required for any `#import` paths the document references.
/// `binary_files_json` — JSON object mapping vault-relative paths to standard
///                       base64-encoded binary content (e.g. images). Pass `"{}"`
///                       if there are no binary files.
#[wasm_bindgen]
pub fn render_to_pdf(typ_source: &str, files_json: &str, binary_files_json: &str) -> Result<Vec<u8>, JsValue> {
    let text_files: HashMap<String, String> = serde_json::from_str(files_json)
        .map_err(|e| JsValue::from_str(&format!("Invalid files JSON: {e}")))?;
    let mut files: HashMap<String, Vec<u8>> = text_files
        .into_iter()
        .map(|(k, v)| (k, v.into_bytes()))
        .collect();

    // Decode base64-encoded binary files (images, etc.) and merge into the map.
    if binary_files_json != "{}" && !binary_files_json.is_empty() {
        use base64::Engine;
        let binary_map: HashMap<String, String> = serde_json::from_str(binary_files_json)
            .map_err(|e| JsValue::from_str(&format!("Invalid binary_files JSON: {e}")))?;
        for (k, v) in binary_map {
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(&v)
                .map_err(|e| JsValue::from_str(&format!("Base64 decode error for '{k}': {e}")))?;
            files.insert(k, bytes);
        }
    }

    let world = WasmWorld::new(typ_source.to_string(), files);
    let result = typst::compile::<typst::layout::PagedDocument>(&world);

    match result.output {
        Ok(document) => {
            typst_pdf::pdf(&document, &typst_pdf::PdfOptions::default())
                .map_err(|e| JsValue::from_str(&format!("PDF export failed: {e:?}")))
        }
        Err(errors) => {
            let msg = errors
                .iter()
                .map(|e| e.message.to_string())
                .collect::<Vec<_>>()
                .join("; ");
            Err(JsValue::from_str(&format!("Typst compilation failed: {msg}")))
        }
    }
}
