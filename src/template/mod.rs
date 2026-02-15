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
        // Typst requires trailing comma for single-element tuples: ("a",) not ("a")
        if authors.len() == 1 {
            format!("({formatted},)")
        } else {
            format!("({formatted})")
        }
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
        if authors.len() == 1 {
            format!("({formatted},)")
        } else {
            format!("({formatted})")
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compose_modern_tech() {
        let src = compose_document(
            Style::ModernTech,
            Some("Title"),
            &["Author".into()],
            "en",
            false,
            "body content",
        );
        assert!(src.contains("#let article("));
        assert!(src.contains("Title"));
        assert!(src.contains("Author"));
        assert!(src.contains("body content"));
    }

    #[test]
    fn compose_classic_editorial() {
        let src = compose_document(
            Style::ClassicEditorial,
            Some("Title"),
            &[],
            "zh",
            true,
            "body",
        );
        assert!(src.contains("#let article("));
        assert!(src.contains("toc: true"));
    }

    #[test]
    fn compose_no_title() {
        let src = compose_document(Style::ModernTech, None, &[], "en", false, "body");
        assert!(src.contains("title: none"));
    }

    #[test]
    fn compose_multiple_authors() {
        let src = compose_document(
            Style::ModernTech,
            None,
            &["Alice".into(), "Bob".into()],
            "en",
            false,
            "body",
        );
        assert!(src.contains("\"Alice\""));
        assert!(src.contains("\"Bob\""));
    }

    #[test]
    fn compose_custom_template() {
        let tmpl = "#let article(title: none, authors: (), lang: \"en\", toc: false, body) = { body }";
        let src = compose_document_with_custom(tmpl, Some("T"), &[], "en", false, "hello");
        assert!(src.contains(tmpl));
        assert!(src.contains("hello"));
        assert!(src.contains("title: \"T\""));
    }

    #[test]
    fn escape_quotes_in_title() {
        let src = compose_document(
            Style::ModernTech,
            Some("He said \"hi\""),
            &[],
            "en",
            false,
            "body",
        );
        assert!(src.contains("He said \\\"hi\\\""));
    }

    #[test]
    fn style_roundtrip() {
        assert_eq!(Style::try_from("modern-tech").unwrap() as u8, Style::ModernTech as u8);
        assert_eq!(Style::try_from("classic-editorial").unwrap() as u8, Style::ClassicEditorial as u8);
        assert!(Style::try_from("nonexistent").is_err());
    }
}
