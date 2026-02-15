use std::fmt::{Display, Formatter};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use typst::diag::{FileError, FileResult};
use typst::foundations::{Bytes, Datetime};
use typst::layout::PagedDocument;
use typst::syntax::{FileId, Source, VirtualPath};
use typst::text::{Font, FontBook, FontInfo};
use typst::utils::LazyHash;
use typst::{Library, World};

use typst_pdf::PdfOptions;

#[derive(Debug)]
pub enum CompileError {
    Io(std::io::Error),
    Typst(String),
}

impl Display for CompileError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(err) => write!(f, "I/O error: {err}"),
            Self::Typst(msg) => write!(f, "typst error: {msg}"),
        }
    }
}

impl std::error::Error for CompileError {}

pub fn compile_typst_to_pdf(source: &str, output_path: &Path) -> Result<Vec<u8>, CompileError> {
    let world = MdxportWorld::new(source);

    let warned = typst::compile::<PagedDocument>(&world);
    let document = warned.output.map_err(|diagnostics| {
        let messages: Vec<String> = diagnostics
            .iter()
            .map(|d| {
                let span_info = d
                    .span
                    .id()
                    .and_then(|id| world.source(id).ok())
                    .and_then(|src| {
                        let range = src.range(d.span)?;
                        let line = src.byte_to_line(range.start)?;
                        Some(format!(" (line {})", line + 1))
                    })
                    .unwrap_or_default();
                format!("{}{span_info}", d.message)
            })
            .collect();
        CompileError::Typst(messages.join("\n"))
    })?;

    let options = PdfOptions::default();
    let pdf_bytes = typst_pdf::pdf(&document, &options).map_err(|diagnostics| {
        let messages: Vec<String> = diagnostics.iter().map(|d| d.message.to_string()).collect();
        CompileError::Typst(messages.join("\n"))
    })?;

    if let Some(parent) = output_path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent).map_err(CompileError::Io)?;
    }
    fs::write(output_path, &pdf_bytes).map_err(CompileError::Io)?;

    Ok(pdf_bytes)
}

// ---------------------------------------------------------------------------
// World implementation
// ---------------------------------------------------------------------------

struct MdxportWorld {
    library: LazyHash<Library>,
    main_id: FileId,
    main_source: Source,
    font_storage: &'static FontStorage,
}

impl MdxportWorld {
    fn new(source: &str) -> Self {
        let main_id = FileId::new(None, VirtualPath::new("/main.typ"));
        let main_source = Source::new(main_id, source.to_string());

        Self {
            library: LazyHash::new(Library::default()),
            main_id,
            main_source,
            font_storage: FontStorage::global(),
        }
    }
}

impl World for MdxportWorld {
    fn library(&self) -> &LazyHash<Library> {
        &self.library
    }

    fn book(&self) -> &LazyHash<FontBook> {
        &self.font_storage.book
    }

    fn main(&self) -> FileId {
        self.main_id
    }

    fn source(&self, id: FileId) -> FileResult<Source> {
        if id == self.main_id {
            Ok(self.main_source.clone())
        } else {
            Err(FileError::NotFound(id.vpath().as_rootless_path().into()))
        }
    }

    fn file(&self, id: FileId) -> FileResult<Bytes> {
        Err(FileError::NotFound(id.vpath().as_rootless_path().into()))
    }

    fn font(&self, index: usize) -> Option<Font> {
        self.font_storage.fonts.get(index)?.get()
    }

    fn today(&self, _offset: Option<i64>) -> Option<Datetime> {
        None
    }
}

// ---------------------------------------------------------------------------
// Font loading (cached globally, loaded once)
// ---------------------------------------------------------------------------

struct FontSlot {
    data: Bytes,
    index: u32,
    font: OnceLock<Option<Font>>,
}

impl FontSlot {
    fn new(data: Bytes, index: u32) -> Self {
        Self {
            data,
            index,
            font: OnceLock::new(),
        }
    }

    fn get(&self) -> Option<Font> {
        self.font
            .get_or_init(|| Font::new(self.data.clone(), self.index))
            .clone()
    }
}

struct FontStorage {
    book: LazyHash<FontBook>,
    fonts: Vec<FontSlot>,
}

static FONT_STORAGE: OnceLock<FontStorage> = OnceLock::new();

impl FontStorage {
    fn global() -> &'static Self {
        FONT_STORAGE.get_or_init(|| {
            let mut book = FontBook::new();
            let mut fonts = Vec::new();

            // Bundled fonts from typst-assets (Libertinus Serif, New CM, DejaVu Sans Mono)
            for data in typst_assets::fonts() {
                let bytes = Bytes::new(data);
                add_font_data(&mut book, &mut fonts, bytes);
            }

            // System fonts
            for dir in system_font_dirs() {
                scan_font_dir(&mut book, &mut fonts, &dir);
            }

            FontStorage {
                book: LazyHash::new(book),
                fonts,
            }
        })
    }
}

fn add_font_data(book: &mut FontBook, fonts: &mut Vec<FontSlot>, data: Bytes) {
    for index in 0_u32.. {
        match FontInfo::new(data.as_slice(), index) {
            Some(info) => {
                book.push(info);
                fonts.push(FontSlot::new(data.clone(), index));
            }
            None => break,
        }
    }
}

fn scan_font_dir(book: &mut FontBook, fonts: &mut Vec<FontSlot>, dir: &Path) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };

    // Deduplicate by canonical path to avoid loading the same font twice
    let mut seen = std::collections::HashSet::new();

    for entry in entries.flatten() {
        let path = entry.path();

        if path.is_dir() {
            scan_font_dir(book, fonts, &path);
            continue;
        }

        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .map(str::to_lowercase);

        match ext.as_deref() {
            Some("ttf" | "otf" | "ttc" | "otc") => {}
            _ => continue,
        }

        if let Ok(canonical) = fs::canonicalize(&path)
            && !seen.insert(canonical)
        {
            continue;
        }

        if let Ok(data) = fs::read(&path) {
            add_font_data(book, fonts, Bytes::new(data));
        }
    }
}

fn system_font_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();

    #[cfg(target_os = "macos")]
    {
        dirs.push(PathBuf::from("/System/Library/Fonts"));
        dirs.push(PathBuf::from("/Library/Fonts"));
        if let Some(home) = home_dir() {
            dirs.push(home.join("Library/Fonts"));
        }
    }

    #[cfg(target_os = "linux")]
    {
        dirs.push(PathBuf::from("/usr/share/fonts"));
        dirs.push(PathBuf::from("/usr/local/share/fonts"));
        if let Some(home) = home_dir() {
            dirs.push(home.join(".local/share/fonts"));
            dirs.push(home.join(".fonts"));
        }
    }

    #[cfg(target_os = "windows")]
    {
        if let Some(windir) = std::env::var_os("WINDIR") {
            dirs.push(PathBuf::from(windir).join("Fonts"));
        }
        if let Some(local) = std::env::var_os("LOCALAPPDATA") {
            dirs.push(PathBuf::from(local).join("Microsoft\\Fonts"));
        }
    }

    dirs
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}
