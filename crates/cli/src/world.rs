use std::path::{Path, PathBuf};

use typst::foundations::Bytes;
use typst::syntax::{FileId, Source, VirtualPath};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Library, World};

pub struct OmdWorld {
    library: LazyHash<Library>,
    book: LazyHash<FontBook>,
    fonts: Vec<FontSlot>,
    root: PathBuf,
    main_id: FileId,
    main_source: Source,
}

struct FontSlot {
    /// Source path for diagnostic purposes. `None` for embedded fonts.
    #[allow(dead_code)]
    path: Option<PathBuf>,
    /// Font file bytes. Always `Some` — set at scan time for both embedded and system fonts.
    data: Bytes,
    /// Face index within the font file.
    index: u32,
}

impl OmdWorld {
    pub fn new(root: PathBuf, typ_source: String) -> Self {
        let mut book = FontBook::new();
        let mut fonts: Vec<FontSlot> = Vec::new();

        // Pass 1: embedded fonts from typst-assets (compiled into the binary).
        for data in typst_assets::fonts() {
            let bytes = Bytes::new(data);
            let mut index = 0u32;
            while let Some(font) = Font::new(bytes.clone(), index) {
                book.push(font.info().clone());
                fonts.push(FontSlot {
                    path: None,
                    data: bytes.clone(),
                    index,
                });
                index += 1;
            }
        }

        // Pass 2: system fonts (Verdana, Arial, DejaVu, etc.).
        for dir in system_font_dirs() {
            scan_font_dir(&dir, &mut book, &mut fonts);
        }

        let main_id = FileId::new(None, VirtualPath::new("main.typ"));
        let main_source = Source::new(main_id, typ_source);

        Self {
            library: LazyHash::new(Library::default()),
            book: LazyHash::new(book),
            fonts,
            root,
            main_id,
            main_source,
        }
    }
}

impl World for OmdWorld {
    fn library(&self) -> &LazyHash<Library> {
        &self.library
    }

    fn book(&self) -> &LazyHash<FontBook> {
        &self.book
    }

    fn main(&self) -> FileId {
        self.main_id
    }

    fn source(&self, id: FileId) -> typst::diag::FileResult<Source> {
        if id == self.main_id {
            return Ok(self.main_source.clone());
        }
        let path = self.root.join(id.vpath().as_rootless_path());
        let text = std::fs::read_to_string(&path)
            .map_err(|e| typst::diag::FileError::from_io(e, &path))?;
        Ok(Source::new(id, text))
    }

    fn file(&self, id: FileId) -> typst::diag::FileResult<Bytes> {
        let path = self.root.join(id.vpath().as_rootless_path());
        let data = std::fs::read(&path)
            .map_err(|e| typst::diag::FileError::from_io(e, &path))?;
        Ok(Bytes::new(data))
    }

    fn font(&self, index: usize) -> Option<Font> {
        let slot = self.fonts.get(index)?;
        Font::new(slot.data.clone(), slot.index)
    }

    fn today(&self, _offset: Option<i64>) -> Option<typst::foundations::Datetime> {
        // Returning None causes Typst to leave date fields blank.
        None
    }
}

/// Returns candidate directories for system fonts on macOS, Linux, and Windows.
pub(crate) fn system_font_dirs() -> Vec<PathBuf> {
    let mut dirs = vec![
        PathBuf::from("/usr/share/fonts"),
        PathBuf::from("/usr/local/share/fonts"),
        PathBuf::from("/Library/Fonts"),
        PathBuf::from("C:\\Windows\\Fonts"),
    ];
    if let Some(home) = std::env::var_os("HOME") {
        dirs.push(PathBuf::from(home).join("Library/Fonts"));
    }
    dirs
}

fn scan_font_dir(dir: &Path, book: &mut FontBook, fonts: &mut Vec<FontSlot>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            scan_font_dir(&path, book, fonts);
        } else if matches!(
            path.extension().and_then(|e| e.to_str()),
            Some("ttf" | "otf" | "ttc" | "otc")
        ) {
            let Ok(data) = std::fs::read(&path) else {
                continue;
            };
            let bytes = Bytes::new(data);
            let mut index = 0u32;
            while let Some(font) = Font::new(bytes.clone(), index) {
                book.push(font.info().clone());
                fonts.push(FontSlot {
                    path: Some(path.clone()),
                    data: bytes.clone(),
                    index,
                });
                index += 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn world_compiles_minimal_document() {
        let world = OmdWorld::new(
            std::env::current_dir().unwrap(),
            "#set page(width: 100pt, height: 100pt)\nHello".to_string(),
        );
        let result = typst::compile::<typst::layout::PagedDocument>(&world);
        assert!(
            result.output.is_ok(),
            "Typst compilation failed: {:?}",
            result.output.err()
        );
    }

    #[test]
    fn system_font_dirs_is_non_empty() {
        let dirs = system_font_dirs();
        assert!(!dirs.is_empty());
    }
}
