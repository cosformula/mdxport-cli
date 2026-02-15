use std::fmt::{Display, Formatter};

#[derive(Debug, Clone, Copy)]
pub enum Style {
    ModernTech,
    ClassicEditorial,
}

#[derive(Debug, Clone)]
pub struct StyleParseError {
    value: String,
}

impl Display for StyleParseError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "unsupported style: {}", self.value)
    }
}

impl std::error::Error for StyleParseError {}

impl TryFrom<&str> for Style {
    type Error = StyleParseError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "modern-tech" => Ok(Self::ModernTech),
            "classic-editorial" => Ok(Self::ClassicEditorial),
            _ => Err(StyleParseError {
                value: value.to_string(),
            }),
        }
    }
}

impl Style {
    fn source(self) -> &'static str {
        match self {
            Self::ModernTech => include_str!("modern_tech.typ"),
            Self::ClassicEditorial => include_str!("classic_editorial.typ"),
        }
    }
}

pub fn compose_document(
    style: Style,
    title: Option<&str>,
    authors: &[String],
    lang: &str,
    toc: bool,
    body: &str,
) -> String {
    let title_value = title.filter(|v| !v.trim().is_empty()).map_or_else(
        || "none".to_string(),
        |v| format!("\"{}\"", escape_string(v)),
    );

    let authors_value = if authors.is_empty() {
        "()".to_string()
    } else {
        let formatted = authors
            .iter()
            .map(|author| format!("\"{}\"", escape_string(author)))
            .collect::<Vec<_>>()
            .join(", ");
        format!("({formatted})")
    };

    let mut source = String::new();
    source.push_str(style.source());
    source.push_str("\n\n");
    source.push_str(&format!(
        "#article(title: {title_value}, authors: {authors_value}, lang: \"{}\", toc: {toc})[",
        escape_string(lang),
    ));
    source.push('\n');
    source.push_str(body);
    source.push('\n');
    source.push_str("]\n");
    source
}

/// Compose a Typst document using a custom template string.
///
/// The template must define `#let article(title: none, authors: (), lang: "en", toc: false, body)`.
pub fn compose_document_with_custom(
    template: &str,
    title: Option<&str>,
    authors: &[String],
    lang: &str,
    toc: bool,
    body: &str,
) -> String {
    let title_value = title.filter(|v| !v.trim().is_empty()).map_or_else(
        || "none".to_string(),
        |v| format!("\"{}\"", escape_string(v)),
    );

    let authors_value = if authors.is_empty() {
        "()".to_string()
    } else {
        let formatted = authors
            .iter()
            .map(|author| format!("\"{}\"", escape_string(author)))
            .collect::<Vec<_>>()
            .join(", ");
        format!("({formatted})")
    };

    let mut source = String::new();
    source.push_str(template);
    source.push_str("\n\n");
    source.push_str(&format!(
        "#article(title: {title_value}, authors: {authors_value}, lang: \"{}\", toc: {toc})[",
        escape_string(lang),
    ));
    source.push('\n');
    source.push_str(body);
    source.push('\n');
    source.push_str("]\n");
    source
}

fn escape_string(input: &str) -> String {
    input
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', " ")
}
