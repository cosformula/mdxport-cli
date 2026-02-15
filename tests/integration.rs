use std::fs;
use std::path::Path;

use mdxport::compile::compile_typst_to_pdf;
use mdxport::convert::{ConvertOptions, convert_markdown_to_typst};
use mdxport::frontmatter::split_frontmatter;
use mdxport::template::{Style, compose_document, compose_document_with_custom};

/// Helper: full pipeline from markdown string to PDF bytes
fn md_to_pdf(markdown: &str, style: Style) -> Vec<u8> {
    let parsed = split_frontmatter(markdown).expect("frontmatter parse");
    let converted = convert_markdown_to_typst(
        &parsed.body,
        &parsed.frontmatter,
        &ConvertOptions::default(),
    )
    .expect("convert");
    let source = compose_document(
        style,
        converted.title.as_deref(),
        &converted.authors,
        &converted.lang,
        converted.toc,
        &converted.body,
    );
    let tmp = Path::new("/tmp").join(format!("mdxport_test_{}.pdf", std::process::id()));
    let bytes = compile_typst_to_pdf(&source, &tmp).expect("compile");
    let _ = fs::remove_file(&tmp);
    bytes
}

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

// ── E2E tests ──────────────────────────────────────────────

#[test]
fn e2e_full_feature_modern_tech() {
    let md = include_str!("fixtures/e2e_full.md");
    let pdf = md_to_pdf(md, Style::ModernTech);
    assert!(pdf.len() > 1000, "PDF too small: {} bytes", pdf.len());
    assert_eq!(&pdf[..5], b"%PDF-");
}

#[test]
fn e2e_full_feature_classic_editorial() {
    let md = include_str!("fixtures/e2e_full.md");
    let pdf = md_to_pdf(md, Style::ClassicEditorial);
    assert!(pdf.len() > 1000, "PDF too small: {} bytes", pdf.len());
    assert_eq!(&pdf[..5], b"%PDF-");
}

#[test]
fn e2e_cjk_chinese_japanese_korean() {
    let md = include_str!("fixtures/cjk.md");
    let pdf = md_to_pdf(md, Style::ModernTech);
    assert!(pdf.len() > 1000, "CJK PDF too small: {} bytes", pdf.len());
    assert_eq!(&pdf[..5], b"%PDF-");
}

#[test]
fn e2e_cjk_classic_editorial() {
    let md = include_str!("fixtures/cjk.md");
    let pdf = md_to_pdf(md, Style::ClassicEditorial);
    assert!(pdf.len() > 1000, "CJK PDF too small: {} bytes", pdf.len());
    assert_eq!(&pdf[..5], b"%PDF-");
}

#[test]
fn e2e_minimal_no_frontmatter() {
    let pdf = md_to_pdf("# Hello\n\nJust a paragraph.", Style::ModernTech);
    assert!(pdf.len() > 500);
    assert_eq!(&pdf[..5], b"%PDF-");
}

#[test]
fn e2e_math_heavy() {
    let md = r#"---
title: Math Test
---

# Equations

Inline $\alpha + \beta = \gamma$ and display:

$$
\int_0^1 \frac{dx}{\sqrt{1-x^2}} = \frac{\pi}{2}
$$

$$
\mathbb{R}^n \to \mathbb{R}^m
$$

$$
\sum_{k=0}^{n} \binom{n}{k} x^k y^{n-k} = (x+y)^n
$$
"#;
    let pdf = md_to_pdf(md, Style::ModernTech);
    assert!(pdf.len() > 1000);
    assert_eq!(&pdf[..5], b"%PDF-");
}

#[test]
fn e2e_custom_template() {
    let md = "# Custom\n\nHello from custom template.";
    let parsed = split_frontmatter(md).unwrap();
    let converted = convert_markdown_to_typst(
        &parsed.body,
        &parsed.frontmatter,
        &ConvertOptions::default(),
    )
    .unwrap();

    // Minimal valid template
    let tmpl = r#"
#let article(title: none, authors: (), lang: "en", toc: false, body) = {
  set page(paper: "a4")
  set text(size: 12pt)
  if title != none { align(center, text(size: 20pt, weight: "bold", title)) }
  body
}
"#;
    let source = compose_document_with_custom(
        tmpl,
        converted.title.as_deref(),
        &converted.authors,
        &converted.lang,
        converted.toc,
        &converted.body,
    );
    let tmp = Path::new("/tmp").join("mdxport_custom_tmpl.pdf");
    let bytes = compile_typst_to_pdf(&source, &tmp).expect("custom template compile");
    assert!(bytes.len() > 500);
    assert_eq!(&bytes[..5], b"%PDF-");
    let _ = fs::remove_file(&tmp);
}

#[test]
fn e2e_high_level_api() {
    let md = "---\ntitle: API Test\nlang: en\n---\n# Hello\n\nWorld.";
    let pdf = mdxport::markdown_to_pdf(md, &mdxport::Options::default())
        .expect("markdown_to_pdf should succeed");
    assert!(pdf.len() > 500);
    assert_eq!(&pdf[..5], b"%PDF-");
}

#[test]
fn e2e_write_and_verify_pdf_files() {
    // Generate actual PDFs to /tmp for visual inspection
    let out_dir = Path::new("/tmp/mdxport-test-output");
    let _ = fs::create_dir_all(out_dir);

    let fixtures = [
        ("e2e_full_modern.pdf", include_str!("fixtures/e2e_full.md"), Style::ModernTech),
        ("e2e_full_classic.pdf", include_str!("fixtures/e2e_full.md"), Style::ClassicEditorial),
        ("cjk_modern.pdf", include_str!("fixtures/cjk.md"), Style::ModernTech),
        ("cjk_classic.pdf", include_str!("fixtures/cjk.md"), Style::ClassicEditorial),
    ];

    for (name, md, style) in &fixtures {
        let pdf = md_to_pdf(md, *style);
        let path = out_dir.join(name);
        fs::write(&path, &pdf).expect("write PDF");
        let meta = fs::metadata(&path).expect("read metadata");
        assert!(meta.len() > 1000, "{name} too small: {} bytes", meta.len());
        eprintln!("  wrote {} ({} bytes)", path.display(), meta.len());
    }
}
