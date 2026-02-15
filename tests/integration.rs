use std::fs;
use std::path::Path;

use mdxport::compile::compile_typst_to_pdf;
use mdxport::convert::{ConvertOptions, convert_markdown_to_typst};
use mdxport::frontmatter::split_frontmatter;
use mdxport::template::{Style, compose_document};

#[test]
fn parse_frontmatter_and_convert_core_syntax() {
    let fixture = include_str!("fixtures/basic.md");
    let parsed = split_frontmatter(fixture).expect("frontmatter parse");

    assert_eq!(
        parsed.frontmatter.title.as_deref(),
        Some("Comprehensive Markdown Demo")
    );
    assert_eq!(parsed.frontmatter.authors.len(), 2);

    let converted = convert_markdown_to_typst(
        &parsed.body,
        &parsed.frontmatter,
        &ConvertOptions {
            title_override: None,
            author_override: None,
            lang_override: None,
            force_toc: None,
        },
    )
    .expect("convert");

    assert!(converted.body.contains("= Top Title"));
    assert!(converted.body.contains("#outline()"));
    assert!(converted.body.contains("- [ ]") || converted.body.contains("- [x]"));
    assert!(converted.body.contains("#table("));
    assert!(converted.body.contains("```rust"));
}

#[test]
fn compose_document_includes_template_and_content() {
    let fixture = include_str!("fixtures/nested_tables.md");
    let parsed = split_frontmatter(fixture).expect("frontmatter parse");

    let converted = convert_markdown_to_typst(
        &parsed.body,
        &parsed.frontmatter,
        &ConvertOptions {
            title_override: Some("Fixture Title".into()),
            author_override: None,
            lang_override: Some("en".into()),
            force_toc: Some(false),
        },
    )
    .expect("convert");

    let source = compose_document(
        Style::ModernTech,
        converted.title.as_deref(),
        &converted.authors,
        &converted.lang,
        converted.toc,
        &converted.body,
    );

    assert!(source.contains("#let article("));
    assert!(source.contains("Fixture Title"));
}

#[test]
fn compile_pipeline_smoke_if_possible() {
    let fixture = include_str!("fixtures/simple.md");
    let parsed = split_frontmatter(fixture).expect("frontmatter parse");

    let converted = convert_markdown_to_typst(
        &parsed.body,
        &parsed.frontmatter,
        &ConvertOptions {
            title_override: None,
            author_override: None,
            lang_override: Some("en".into()),
            force_toc: None,
        },
    )
    .expect("convert");

    let source = compose_document(
        Style::ClassicEditorial,
        converted.title.as_deref(),
        &converted.authors,
        &converted.lang,
        converted.toc,
        &converted.body,
    );

    let tmp_path = Path::new("/tmp").join("mdxport_smoke_test.pdf");
    let bytes = compile_typst_to_pdf(&source, &tmp_path).expect("compile should succeed");
    assert!(!bytes.is_empty());
    let _ = fs::remove_file(&tmp_path);
}
