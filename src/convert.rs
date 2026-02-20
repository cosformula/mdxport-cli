use std::collections::{HashMap, HashSet};
use std::fmt::{Display, Formatter};

use comrak::{
    Arena, ComrakOptions,
    nodes::{AstNode, ListType, NodeCodeBlock, NodeList, NodeMath, NodeValue, TableAlignment},
    parse_document,
};

use crate::frontmatter::FrontMatter;
use crate::math::latex_to_typst;

const TOC_TOKEN: &str = "MDXPORTTOCPLACEHOLDER7f3a";

#[derive(Debug)]
pub struct ConvertError {
    message: String,
}

impl Display for ConvertError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for ConvertError {}

#[derive(Debug, Clone, Default)]
pub struct ConvertOptions {
    pub title_override: Option<String>,
    pub author_override: Option<String>,
    pub lang_override: Option<String>,
    pub force_toc: Option<bool>,
}

#[derive(Debug, Clone)]
pub struct ConvertedDocument {
    pub title: Option<String>,
    pub authors: Vec<String>,
    pub lang: String,
    pub body: String,
    pub toc: bool,
}

pub fn convert_markdown_to_typst(
    markdown: &str,
    frontmatter: &FrontMatter,
    options: &ConvertOptions,
) -> Result<ConvertedDocument, ConvertError> {
    let (normalized, has_inline_toc) = normalize_toc_tokens(markdown);

    let mut comrak_options = ComrakOptions::default();
    comrak_options.extension.table = true;
    comrak_options.extension.strikethrough = true;
    comrak_options.extension.tasklist = true;
    comrak_options.extension.footnotes = true;
    comrak_options.extension.superscript = true;
    comrak_options.extension.autolink = true;
    comrak_options.extension.math_dollars = true;
    comrak_options.extension.math_code = true;
    comrak_options.extension.subscript = true;
    comrak_options.extension.underline = true;

    let arena = Arena::new();
    let root = parse_document(&arena, &normalized, &comrak_options);

    let toc_enabled = options
        .force_toc
        .unwrap_or_else(|| frontmatter.toc.unwrap_or(has_inline_toc));

    let mut renderer = TypstRenderer::new(toc_enabled);
    renderer.collect_footnotes(root);

    let body = renderer.render_blocks(root, 0).trim().to_string();
    let body = if body.is_empty() {
        String::new()
    } else {
        format!("{body}\n")
    };

    let lang = options
        .lang_override
        .as_deref()
        .and_then(non_empty_str)
        .map(ToOwned::to_owned)
        .or_else(|| {
            frontmatter
                .lang
                .as_deref()
                .and_then(non_empty_str)
                .map(ToOwned::to_owned)
        })
        .unwrap_or_else(|| detect_lang(markdown));

    Ok(ConvertedDocument {
        title: options
            .title_override
            .as_deref()
            .and_then(non_empty_str)
            .map(ToOwned::to_owned)
            .or_else(|| {
                frontmatter
                    .title
                    .as_deref()
                    .and_then(non_empty_str)
                    .map(ToOwned::to_owned)
            }),
        authors: resolve_authors(frontmatter, options),
        lang,
        body,
        toc: toc_enabled && !has_inline_toc,
    })
}

fn resolve_authors(frontmatter: &FrontMatter, options: &ConvertOptions) -> Vec<String> {
    if let Some(author) = options
        .author_override
        .as_deref()
        .and_then(non_empty_str)
        .map(ToOwned::to_owned)
    {
        return vec![author];
    }

    let mut authors = Vec::new();
    let mut seen = HashSet::new();

    for author in &frontmatter.authors {
        if let Some(author) = non_empty_str(author) {
            let owned = author.to_string();
            if seen.insert(owned.clone()) {
                authors.push(owned);
            }
        }
    }

    if authors.is_empty()
        && let Some(author) = frontmatter
            .author
            .as_deref()
            .and_then(non_empty_str)
            .map(ToOwned::to_owned)
    {
        authors.push(author);
    }

    authors
}

fn non_empty_str(value: &str) -> Option<&str> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

fn normalize_toc_tokens(markdown: &str) -> (String, bool) {
    let mut normalized = String::with_capacity(markdown.len() + 16);
    let mut has_inline_toc = false;

    for line in markdown.lines() {
        if line.trim() == "[toc]" {
            has_inline_toc = true;
            normalized.push_str(TOC_TOKEN);
        } else {
            normalized.push_str(line);
        }
        normalized.push('\n');
    }

    (normalized, has_inline_toc)
}

struct TypstRenderer {
    toc_enabled: bool,
    footnotes: HashMap<String, String>,
}

impl TypstRenderer {
    fn new(toc_enabled: bool) -> Self {
        Self {
            toc_enabled,
            footnotes: HashMap::new(),
        }
    }

    fn collect_footnotes<'a>(&mut self, root: &'a AstNode<'a>) {
        for node in root.children() {
            let value = node.data.borrow().value.clone();
            if let NodeValue::FootnoteDefinition(def) = value {
                let content = self.render_footnote_body(node);
                if !content.trim().is_empty() {
                    self.footnotes.insert(def.name, content);
                }
            }
        }
    }

    fn render_footnote_body<'a>(&self, footnote: &'a AstNode<'a>) -> String {
        let mut out = String::new();
        for child in footnote.children() {
            out.push_str(&self.render_block(child, 0));
        }
        out.trim().to_string()
    }

    fn render_blocks<'a>(&self, parent: &'a AstNode<'a>, indent: usize) -> String {
        let mut out = String::new();
        for node in parent.children() {
            out.push_str(&self.render_block(node, indent));
        }
        out
    }

    fn render_block<'a>(&self, node: &'a AstNode<'a>, indent: usize) -> String {
        let value = node.data.borrow().value.clone();
        match value {
            NodeValue::Document => self.render_blocks(node, indent),
            NodeValue::FrontMatter(_) => String::new(),
            NodeValue::Paragraph => self.render_paragraph(node),
            NodeValue::Heading(heading) => {
                let level = usize::from(heading.level.max(1));
                let title = self.render_inlines(node).trim().to_string();
                if title.is_empty() {
                    String::new()
                } else {
                    format!("{} {}\n\n", "=".repeat(level), title)
                }
            }
            NodeValue::BlockQuote | NodeValue::MultilineBlockQuote(_) => {
                let inner = self.render_blocks(node, indent + 1).trim().to_string();
                if inner.is_empty() {
                    String::new()
                } else {
                    format!("#quote[\n{inner}\n]\n\n")
                }
            }
            NodeValue::List(list) => self.render_list(node, &list, indent),
            NodeValue::Item(_) | NodeValue::TaskItem(_) => self.render_blocks(node, indent),
            NodeValue::CodeBlock(code) => render_code_block(&code),
            NodeValue::ThematicBreak => "#line(length: 100%, stroke: 0.5pt)\n\n".to_string(),
            NodeValue::Table(table) => self.render_table(node, &table),
            NodeValue::TableRow(_) | NodeValue::TableCell => self.render_blocks(node, indent),
            NodeValue::FootnoteDefinition(_) => String::new(),
            NodeValue::DescriptionList => self.render_description_list(node, indent),
            NodeValue::DescriptionItem(_) => self.render_description_item(node, indent),
            NodeValue::DescriptionTerm | NodeValue::DescriptionDetails => {
                self.render_blocks(node, indent)
            }
            NodeValue::Alert(alert) => {
                let title = alert
                    .title
                    .unwrap_or_else(|| default_alert_title(&alert.alert_type).to_string());
                let inner = self.render_blocks(node, indent + 1).trim().to_string();
                if inner.is_empty() {
                    String::new()
                } else {
                    format!("#quote[\n*{}*\n\n{}\n]\n\n", escape_text(&title), inner)
                }
            }
            NodeValue::HtmlBlock(_) => String::new(),
            other if other.block() => self.render_blocks(node, indent),
            _ => String::new(),
        }
    }

    fn render_paragraph<'a>(&self, node: &'a AstNode<'a>) -> String {
        if let Some(block_math) = self.extract_single_display_math(node) {
            return format!("{block_math}\n\n");
        }

        let text = self.render_inlines(node).trim().to_string();
        if text.is_empty() {
            String::new()
        } else {
            format!("{text}\n\n")
        }
    }

    fn extract_single_display_math<'a>(&self, node: &'a AstNode<'a>) -> Option<String> {
        let mut children = node.children();
        let first = children.next()?;
        if children.next().is_some() {
            return None;
        }

        let value = first.data.borrow().value.clone();
        let NodeValue::Math(math) = value else {
            return None;
        };

        if math.display_math {
            Some(render_math(&math))
        } else {
            None
        }
    }

    fn render_description_list<'a>(&self, node: &'a AstNode<'a>, indent: usize) -> String {
        let mut out = String::new();
        for item in node.children() {
            out.push_str(&self.render_description_item(item, indent));
        }
        out.push('\n');
        out
    }

    fn render_description_item<'a>(&self, node: &'a AstNode<'a>, indent: usize) -> String {
        let mut term = String::new();
        let mut details = String::new();

        for child in node.children() {
            let value = child.data.borrow().value.clone();
            match value {
                NodeValue::DescriptionTerm => {
                    let rendered = self.render_blocks(child, indent).trim().to_string();
                    if !rendered.is_empty() {
                        term = rendered;
                    }
                }
                NodeValue::DescriptionDetails => {
                    let rendered = self.render_blocks(child, indent + 1).trim().to_string();
                    if !rendered.is_empty() {
                        details = rendered;
                    }
                }
                _ => {}
            }
        }

        if term.is_empty() && details.is_empty() {
            return String::new();
        }

        let mut out = String::new();
        out.push_str(&"  ".repeat(indent));
        out.push_str("- ");
        if term.is_empty() {
            out.push_str(&details);
            out.push('\n');
            return out;
        }

        out.push('*');
        out.push_str(&term);
        out.push('*');
        if !details.is_empty() {
            out.push(':');
            out.push(' ');
            out.push_str(&details.replace('\n', " "));
        }
        out.push('\n');
        out
    }

    fn render_list<'a>(
        &self,
        list_node: &'a AstNode<'a>,
        list: &NodeList,
        indent: usize,
    ) -> String {
        let mut out = String::new();
        let ordered = list.list_type == ListType::Ordered;
        let mut index = list.start.max(1);

        for item in list_node.children() {
            let value = item.data.borrow().value.clone();
            if !matches!(value, NodeValue::Item(_) | NodeValue::TaskItem(_)) {
                continue;
            }

            out.push_str(&self.render_list_item(item, ordered, index, indent));
            if ordered {
                index += 1;
            }
        }

        out.push('\n');
        out
    }

    fn render_list_item<'a>(
        &self,
        item_node: &'a AstNode<'a>,
        ordered: bool,
        index: usize,
        indent: usize,
    ) -> String {
        let marker = if ordered {
            format!("{index}. ")
        } else {
            "- ".to_string()
        };

        let task_prefix = match item_node.data.borrow().value.clone() {
            NodeValue::TaskItem(symbol) => {
                if symbol.is_some() {
                    "[x] "
                } else {
                    "[ ] "
                }
            }
            _ => "",
        };

        let mut head = String::new();
        let mut tail = String::new();

        for child in item_node.children() {
            let value = child.data.borrow().value.clone();
            match value {
                NodeValue::Paragraph if head.is_empty() => {
                    head = self.render_inlines(child).trim().to_string();
                }
                _ => {
                    tail.push_str(&self.render_block(child, indent + 1));
                }
            }
        }

        let mut out = String::new();
        out.push_str(&"  ".repeat(indent));
        out.push_str(&marker);
        out.push_str(task_prefix);
        out.push_str(head.trim());
        out.push('\n');

        let tail = tail.trim_end();
        if !tail.is_empty() {
            out.push_str(&indent_block(tail, indent + 1));
            out.push('\n');
        }

        out
    }

    fn render_table<'a>(
        &self,
        table_node: &'a AstNode<'a>,
        table: &comrak::nodes::NodeTable,
    ) -> String {
        let mut rows: Vec<Vec<String>> = Vec::new();

        for row_node in table_node.children() {
            let value = row_node.data.borrow().value.clone();
            if !matches!(value, NodeValue::TableRow(_)) {
                continue;
            }

            let mut row = Vec::new();
            for cell_node in row_node.children() {
                let cell_value = cell_node.data.borrow().value.clone();
                if matches!(cell_value, NodeValue::TableCell) {
                    row.push(self.render_table_cell(cell_node));
                }
            }
            rows.push(row);
        }

        let max_cols = table
            .num_columns
            .max(rows.iter().map(std::vec::Vec::len).max().unwrap_or(0));

        if max_cols == 0 || rows.is_empty() {
            return String::new();
        }

        let mut out = String::new();
        out.push_str("#table(\n");
        out.push_str(&format!("  columns: {max_cols},\n"));

        let alignments = table
            .alignments
            .iter()
            .take(max_cols)
            .map(table_alignment)
            .collect::<Vec<_>>();
        if !alignments.is_empty() {
            out.push_str("  align: (");
            out.push_str(&alignments.join(", "));
            out.push_str("),\n");
        }

        for row in rows {
            for col in 0..max_cols {
                let cell = row.get(col).map_or("", String::as_str);
                out.push_str("  [");
                out.push_str(cell);
                out.push_str("],\n");
            }
        }

        out.push_str(")\n\n");
        out
    }

    fn render_table_cell<'a>(&self, cell_node: &'a AstNode<'a>) -> String {
        let mut parts = Vec::new();
        let mut inline_part = String::new();

        for child in cell_node.children() {
            let value = child.data.borrow().value.clone();
            match value {
                NodeValue::Paragraph => {
                    let inline = inline_part.trim();
                    if !inline.is_empty() {
                        parts.push(inline.to_string());
                    }
                    inline_part.clear();

                    let text = self.render_inlines(child).trim().to_string();
                    if !text.is_empty() {
                        parts.push(text);
                    }
                }
                other if !other.block() => {
                    inline_part.push_str(&self.render_inline(child));
                }
                _ => {
                    let inline = inline_part.trim();
                    if !inline.is_empty() {
                        parts.push(inline.to_string());
                    }
                    inline_part.clear();

                    let text = self.render_block(child, 0).trim().to_string();
                    if !text.is_empty() {
                        parts.push(text.replace('\n', " "));
                    }
                }
            }
        }

        let inline = inline_part.trim();
        if !inline.is_empty() {
            parts.push(inline.to_string());
        }

        parts.join(" ")
    }

    fn render_inlines<'a>(&self, parent: &'a AstNode<'a>) -> String {
        let mut out = String::new();
        for node in parent.children() {
            out.push_str(&self.render_inline(node));
        }
        out
    }

    fn render_inline<'a>(&self, node: &'a AstNode<'a>) -> String {
        let value = node.data.borrow().value.clone();
        match value {
            NodeValue::Text(text) => self.render_text(&text),
            NodeValue::Code(code) => render_inline_code(&code.literal),
            NodeValue::SoftBreak => " ".to_string(),
            NodeValue::LineBreak => "\\\n".to_string(),
            NodeValue::Emph => wrap_markup("_", &self.render_inlines(node)),
            NodeValue::Strong => wrap_markup("*", &self.render_inlines(node)),
            NodeValue::Strikethrough => wrap_function("strike", &self.render_inlines(node)),
            NodeValue::Superscript => wrap_function("super", &self.render_inlines(node)),
            NodeValue::Subscript => wrap_function("sub", &self.render_inlines(node)),
            NodeValue::Underline => wrap_function("underline", &self.render_inlines(node)),
            NodeValue::SpoileredText => wrap_function("hide", &self.render_inlines(node)),
            NodeValue::Link(link) => {
                let label = self.render_inlines(node).trim().to_string();
                let label = if label.is_empty() {
                    escape_text(&link.url)
                } else {
                    label
                };
                format!("#link(\"{}\")[{}]", escape_string(&link.url), label)
            }
            NodeValue::Image(link) => {
                let alt = self.render_inlines(node).trim().to_string();
                let label = if alt.is_empty() {
                    "image".to_string()
                } else {
                    alt
                };
                format!("#link(\"{}\")[{}]", escape_string(&link.url), label)
            }
            NodeValue::WikiLink(link) => {
                let label = if link.url.trim().is_empty() {
                    "wiki".to_string()
                } else {
                    escape_text(&link.url)
                };
                format!("#link(\"{}\")[{}]", escape_string(&link.url), label)
            }
            NodeValue::FootnoteReference(reference) => {
                if let Some(content) = self.footnotes.get(&reference.name) {
                    format!("#footnote[{}]", content)
                } else {
                    format!("#footnote[{}]", escape_text(&reference.name))
                }
            }
            NodeValue::Math(math) => render_math(&math),
            NodeValue::Raw(raw) => raw,
            NodeValue::EscapedTag(tag) => escape_text(&tag),
            NodeValue::Escaped => "\\".to_string(),
            NodeValue::HtmlInline(_) => String::new(),
            other if !other.block() => self.render_inlines(node),
            _ => String::new(),
        }
    }

    fn render_text(&self, text: &str) -> String {
        if !text.contains(TOC_TOKEN) {
            return escape_text(text);
        }

        let mut out = String::new();
        let mut pieces = text.split(TOC_TOKEN).peekable();
        while let Some(piece) = pieces.next() {
            out.push_str(&escape_text(piece));
            if pieces.peek().is_some() && self.toc_enabled {
                out.push_str("\n#outline()\n");
            }
        }
        out
    }
}

fn table_alignment(alignment: &TableAlignment) -> &'static str {
    match alignment {
        TableAlignment::None => "left",
        TableAlignment::Left => "left",
        TableAlignment::Center => "center",
        TableAlignment::Right => "right",
    }
}

fn default_alert_title(alert_type: &comrak::nodes::AlertType) -> &'static str {
    match alert_type {
        comrak::nodes::AlertType::Note => "Note",
        comrak::nodes::AlertType::Tip => "Tip",
        comrak::nodes::AlertType::Important => "Important",
        comrak::nodes::AlertType::Warning => "Warning",
        comrak::nodes::AlertType::Caution => "Caution",
    }
}

fn indent_block(block: &str, indent: usize) -> String {
    let prefix = "  ".repeat(indent);
    let mut out = String::new();

    for line in block.lines() {
        if line.is_empty() {
            out.push('\n');
        } else {
            out.push_str(&prefix);
            out.push_str(line);
            out.push('\n');
        }
    }

    out.trim_end().to_string()
}

fn wrap_function(name: &str, body: &str) -> String {
    let body = body.trim();
    if body.is_empty() {
        String::new()
    } else {
        format!("#{name}[{body}]")
    }
}

fn wrap_markup(marker: &str, body: &str) -> String {
    let body = body.trim();
    if body.is_empty() {
        String::new()
    } else {
        format!("{marker}{body}{marker}")
    }
}

fn render_code_block(code: &NodeCodeBlock) -> String {
    let language = code.info.split_whitespace().next().unwrap_or("");
    let fence = backtick_fence(&code.literal, 3);

    let mut out = String::new();
    out.push_str(&fence);
    if !language.is_empty() {
        out.push_str(language);
    }
    out.push('\n');
    out.push_str(code.literal.trim_end_matches('\n'));
    out.push('\n');
    out.push_str(&fence);
    out.push_str("\n\n");
    out
}

fn render_inline_code(code: &str) -> String {
    let fence = backtick_fence(code, 1);
    format!("{fence}{}{fence}", escape_inline_code(code))
}

fn backtick_fence(text: &str, min: usize) -> String {
    let mut longest = 0usize;
    let mut current = 0usize;

    for ch in text.chars() {
        if ch == '`' {
            current += 1;
            longest = longest.max(current);
        } else {
            current = 0;
        }
    }

    "`".repeat((longest + 1).max(min))
}

fn render_math(math: &NodeMath) -> String {
    let body = latex_to_typst(math.literal.trim());
    if math.display_math {
        format!("$\n{body}\n$")
    } else {
        format!("${body}$")
    }
}

fn escape_text(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '#' => out.push_str("\\#"),
            '[' => out.push_str("\\["),
            ']' => out.push_str("\\]"),
            '{' => out.push_str("\\{"),
            '}' => out.push_str("\\}"),
            '*' => out.push_str("\\*"),
            '_' => out.push_str("\\_"),
            '$' => out.push_str("\\$"),
            '`' => out.push_str("\\`"),
            _ => out.push(ch),
        }
    }
    out
}

fn escape_inline_code(input: &str) -> String {
    input.replace('\\', "\\\\").replace('`', "\\`")
}

fn escape_string(input: &str) -> String {
    input.replace('\\', "\\\\").replace('"', "\\\"")
}

fn detect_lang(input: &str) -> String {
    if input
        .chars()
        .any(|c| ('\u{4e00}'..='\u{9fff}').contains(&c))
    {
        "zh".to_string()
    } else {
        "en".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts() -> ConvertOptions {
        ConvertOptions::default()
    }

    fn convert(md: &str) -> ConvertedDocument {
        convert_markdown_to_typst(md, &FrontMatter::default(), &opts())
            .expect("conversion should succeed")
    }

    #[test]
    fn heading_levels() {
        let doc = convert("# H1\n## H2\n### H3\n#### H4");
        assert!(doc.body.contains("= H1"));
        assert!(doc.body.contains("== H2"));
        assert!(doc.body.contains("=== H3"));
        assert!(doc.body.contains("==== H4"));
    }

    #[test]
    fn bold_and_italic() {
        let doc = convert("**bold** and *italic* and ***both***");
        // Typst uses *bold* and _italic_ markup
        assert!(doc.body.contains("*bold*"));
        assert!(doc.body.contains("_italic_"));
    }

    #[test]
    fn inline_code() {
        let doc = convert("Use `println!()` here");
        assert!(doc.body.contains("`println!()`"));
    }

    #[test]
    fn fenced_code_block() {
        let doc = convert("```python\nprint('hi')\n```");
        assert!(doc.body.contains("```python"));
        assert!(doc.body.contains("print('hi')"));
    }

    #[test]
    fn unordered_list() {
        let doc = convert("- item a\n- item b\n- item c");
        assert!(doc.body.contains("- item a"));
        assert!(doc.body.contains("- item b"));
    }

    #[test]
    fn ordered_list() {
        let doc = convert("1. first\n2. second");
        // Typst uses `+` or numbered syntax
        assert!(doc.body.contains("first"));
        assert!(doc.body.contains("second"));
    }

    #[test]
    fn link() {
        let doc = convert("[click](https://example.com)");
        assert!(doc.body.contains("#link(\"https://example.com\")[click]"));
    }

    #[test]
    fn image() {
        // Currently images render as links (Typst can't load arbitrary paths)
        let doc = convert("![alt text](image.png)");
        assert!(doc.body.contains("image.png"));
    }

    #[test]
    fn debug_table_typst_output() {
        let doc = convert("| A | B |\n|---|---|\n| one | two |");
        println!("--- debug_table_typst_output ---\n{}", doc.body);
        assert!(doc.body.contains("#table("));
    }

    #[test]
    fn table() {
        let doc = convert("| A | B |\n|---|---|\n| one | two |\n| three | four |");
        assert!(doc.body.contains("#table("));
        assert!(doc.body.contains("columns:"));
        assert!(doc.body.contains("[one]"));
        assert!(doc.body.contains("[two]"));
        assert!(doc.body.contains("[three]"));
        assert!(doc.body.contains("[four]"));
    }

    #[test]
    fn table_with_inline_formatting() {
        let doc = convert("| Left | Right |\n|---|---|\n| **bold** | `code` |");
        assert!(doc.body.contains("[*bold*]"));
        assert!(doc.body.contains("[`code`]"));
    }

    #[test]
    fn table_with_math() {
        let doc = convert("| Expr | Value |\n|---|---|\n| $x^2$ | $\\alpha + \\beta$ |");
        assert!(doc.body.contains("[$x^2$]"));
        assert!(doc.body.contains("[$alpha + beta$]"));
    }

    #[test]
    fn table_edge_cases() {
        let doc = convert("| A | B | C |\n|---|---|---|\n| left |  | tail |\n| only one |||");
        assert!(doc.body.contains("[left]"));
        assert!(doc.body.contains("[tail]"));
        assert!(doc.body.contains("[only one]"));
    }

    #[test]
    fn task_list() {
        let doc = convert("- [x] done\n- [ ] todo");
        assert!(doc.body.contains("[x]") || doc.body.contains("☑"));
        assert!(doc.body.contains("[ ]") || doc.body.contains("☐"));
    }

    #[test]
    fn inline_math() {
        let doc = convert("Equation $E = mc^2$ here");
        assert!(doc.body.contains("$"));
    }

    #[test]
    fn display_math() {
        let doc = convert("$$\n\\frac{a}{b}\n$$");
        assert!(doc.body.contains("$"));
    }

    #[test]
    fn horizontal_rule() {
        let doc = convert("above\n\n---\n\nbelow");
        assert!(doc.body.contains("#line("));
    }

    #[test]
    fn blockquote() {
        let doc = convert("> quoted text");
        assert!(doc.body.contains("#quote[") || doc.body.contains("#blockquote"));
    }

    #[test]
    fn strikethrough() {
        let doc = convert("~~deleted~~");
        assert!(doc.body.contains("#strike[deleted]"));
    }

    #[test]
    fn title_override() {
        let doc = convert_markdown_to_typst(
            "# Original",
            &FrontMatter::default(),
            &ConvertOptions {
                title_override: Some("Override".into()),
                ..ConvertOptions::default()
            },
        )
        .expect("conversion should succeed");
        assert_eq!(doc.title.as_deref(), Some("Override"));
    }

    #[test]
    fn lang_from_frontmatter() {
        let fm = FrontMatter {
            lang: Some("zh".into()),
            ..FrontMatter::default()
        };
        let doc = convert_markdown_to_typst("# Hi", &fm, &opts()).unwrap();
        assert_eq!(doc.lang, "zh");
    }

    #[test]
    fn inline_toc_disables_template_toc() {
        let doc = convert_markdown_to_typst(
            "[toc]\n\n# Title",
            &FrontMatter::default(),
            &ConvertOptions {
                title_override: None,
                author_override: None,
                lang_override: None,
                force_toc: None,
            },
        )
        .expect("conversion should succeed");

        assert!(!doc.toc);
        assert!(doc.body.contains("#outline()"));
    }

    #[test]
    fn force_toc_without_token_enables_template_toc() {
        let doc = convert_markdown_to_typst(
            "# Title",
            &FrontMatter::default(),
            &ConvertOptions {
                title_override: None,
                author_override: None,
                lang_override: None,
                force_toc: Some(true),
            },
        )
        .expect("conversion should succeed");

        assert!(doc.toc);
    }

    #[test]
    fn footnote_definition_turns_into_typst_footnote() {
        let doc = convert_markdown_to_typst(
            "Footnote ref[^a].\n\n[^a]: Content.",
            &FrontMatter::default(),
            &ConvertOptions {
                title_override: None,
                author_override: None,
                lang_override: None,
                force_toc: None,
            },
        )
        .expect("conversion should succeed");

        assert!(doc.body.contains("#footnote[Content."));
    }
}
