//! # mdxport
//!
//! Markdown to PDF via Typst — with comrak AST parsing, in-process Typst
//! compilation, and automatic LaTeX-to-Typst math conversion.
//!
//! ## Quick start
//!
//! ```rust,no_run
//! use std::path::Path;
//! use mdxport::{markdown_to_pdf, Options};
//!
//! let markdown = "# Hello\n\nWorld with $E = mc^2$.";
//! let pdf_bytes = markdown_to_pdf(markdown, &Options::default())
//!     .expect("conversion failed");
//! std::fs::write("output.pdf", &pdf_bytes).unwrap();
//! ```
//!
//! ## Lower-level API
//!
//! For more control, use the individual modules:
//!
//! ```rust,no_run
//! use mdxport::{frontmatter, convert, template, compile};
//!
//! let input = "---\ntitle: Demo\n---\n# Hello";
//! let parsed = frontmatter::split_frontmatter(input).unwrap();
//! let converted = convert::convert_markdown_to_typst(
//!     &parsed.body,
//!     &parsed.frontmatter,
//!     &convert::ConvertOptions::default(),
//! ).unwrap();
//! let typst_source = template::compose_document(
//!     template::Style::ModernTech,
//!     converted.title.as_deref(),
//!     &converted.authors,
//!     &converted.lang,
//!     converted.toc,
//!     &converted.body,
//! );
//! let pdf = compile::compile_typst_to_pdf(
//!     &typst_source,
//!     std::path::Path::new("output.pdf"),
//! ).unwrap();
//! ```

pub mod compile;
pub mod convert;
pub mod frontmatter;
pub mod math;
pub mod template;

#[cfg(feature = "cli")]
pub mod watch;

pub use compile::{CompileError, compile_typst_to_pdf};
pub use convert::{ConvertError, ConvertOptions, ConvertedDocument, convert_markdown_to_typst};
pub use frontmatter::{FrontMatter, ParsedMarkdown, split_frontmatter};
pub use template::{Style, compose_document};

/// High-level options for the one-shot `markdown_to_pdf` function.
#[derive(Debug, Clone)]
pub struct Options {
    /// Template style. Default: `ModernTech`.
    pub style: Style,
    /// Override document title (takes precedence over frontmatter).
    pub title: Option<String>,
    /// Override document author (takes precedence over frontmatter).
    pub author: Option<String>,
    /// Override document language (takes precedence over frontmatter).
    pub lang: Option<String>,
    /// Force table of contents on/off. `None` = use frontmatter / inline `[toc]`.
    pub toc: Option<bool>,
    /// Custom Typst template source. When set, overrides the built-in style.
    pub custom_template: Option<String>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            style: Style::ModernTech,
            title: None,
            author: None,
            lang: None,
            toc: None,
            custom_template: None,
        }
    }
}

/// Convert a Markdown string to PDF bytes in one call.
///
/// This is the highest-level API. For streaming / watch / incremental use
/// cases, use the lower-level modules directly.
pub fn markdown_to_pdf(markdown: &str, options: &Options) -> Result<Vec<u8>, Error> {
    let parsed = split_frontmatter(markdown).map_err(Error::Frontmatter)?;

    let converted = convert_markdown_to_typst(
        &parsed.body,
        &parsed.frontmatter,
        &ConvertOptions {
            title_override: options.title.clone(),
            author_override: options.author.clone(),
            lang_override: options.lang.clone(),
            force_toc: options.toc,
        },
    )
    .map_err(Error::Convert)?;

    let typst_source = if let Some(ref custom) = options.custom_template {
        template::compose_document_with_custom(
            custom,
            converted.title.as_deref(),
            &converted.authors,
            &converted.lang,
            converted.toc,
            &converted.body,
        )
    } else {
        compose_document(
            options.style,
            converted.title.as_deref(),
            &converted.authors,
            &converted.lang,
            converted.toc,
            &converted.body,
        )
    };

    // Compile to PDF in memory (write to temp, read back)
    let tmp = std::env::temp_dir().join(format!("mdxport_{}.pdf", std::process::id()));
    let pdf_bytes = compile_typst_to_pdf(&typst_source, &tmp).map_err(Error::Compile)?;
    let _ = std::fs::remove_file(&tmp);

    Ok(pdf_bytes)
}

/// Top-level error type combining all pipeline stages.
#[derive(Debug)]
pub enum Error {
    Frontmatter(frontmatter::FrontMatterError),
    Convert(ConvertError),
    Compile(CompileError),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Frontmatter(e) => write!(f, "frontmatter: {e}"),
            Self::Convert(e) => write!(f, "convert: {e}"),
            Self::Compile(e) => write!(f, "compile: {e}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Frontmatter(e) => Some(e),
            Self::Convert(e) => Some(e),
            Self::Compile(e) => Some(e),
        }
    }
}
