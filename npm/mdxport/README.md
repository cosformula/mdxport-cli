# mdxport

Markdown to PDF via [Typst](https://typst.app) — as a Rust library and CLI.

## Features

- **AST-based conversion** — uses [comrak](https://crates.io/crates/comrak) to parse Markdown into an AST, preserving semantics (math, footnotes, task lists, tables) instead of going through lossy HTML
- **In-process Typst compilation** — embeds the Typst compiler as a library, no external `typst` binary needed
- **LaTeX math support** — automatically converts LaTeX math (`$E = mc^2$`, `\frac{a}{b}`) to Typst math via [tex2typst-rs](https://crates.io/crates/tex2typst-rs)
- **Built-in templates** — `modern-tech` (sans-serif) and `classic-editorial` (serif), or bring your own `.typ` template
- **YAML frontmatter** — title, author(s), language, TOC toggle
- **Watch mode** — recompile on file change
- **Single binary** — no runtime dependencies

## Install

```sh
cargo install mdxport
```

## CLI Usage

```sh
# Basic conversion
mdxport input.md -o output.pdf

# Choose a style
mdxport input.md -s classic-editorial

# Custom template
mdxport input.md --template my_style.typ

# Override metadata
mdxport input.md -t "My Title" -a "Author Name" --lang zh

# Watch mode
mdxport input.md -w

# Multiple files
mdxport chapter1.md chapter2.md -o output_dir/

# From stdin
cat input.md | mdxport -o output.pdf
```

## Library Usage

### One-liner

```rust
use mdxport::{markdown_to_pdf, Options};

let md = "# Hello\n\nWorld with $E = mc^2$.";
let pdf = markdown_to_pdf(md, &Options::default()).unwrap();
std::fs::write("output.pdf", &pdf).unwrap();
```

### With options

```rust
use mdxport::{markdown_to_pdf, Options, Style};

let pdf = markdown_to_pdf("# Hello", &Options {
    style: Style::ClassicEditorial,
    title: Some("My Doc".into()),
    lang: Some("zh".into()),
    toc: Some(true),
    ..Options::default()
}).unwrap();
```

### Custom template

```rust
use mdxport::{markdown_to_pdf, Options};

let template = std::fs::read_to_string("my_template.typ").unwrap();
let pdf = markdown_to_pdf("# Hello", &Options {
    custom_template: Some(template),
    ..Options::default()
}).unwrap();
```

### Lower-level API

```rust
use mdxport::{frontmatter, convert, template, compile};

let input = "---\ntitle: Demo\n---\n# Hello";
let parsed = frontmatter::split_frontmatter(input).unwrap();
let converted = convert::convert_markdown_to_typst(
    &parsed.body,
    &parsed.frontmatter,
    &convert::ConvertOptions::default(),
).unwrap();
let typst_source = template::compose_document(
    template::Style::ModernTech,
    converted.title.as_deref(),
    &converted.authors,
    &converted.lang,
    converted.toc,
    &converted.body,
);
let pdf = compile::compile_typst_to_pdf(
    &typst_source,
    std::path::Path::new("output.pdf"),
).unwrap();
```

## Frontmatter

```yaml
---
title: "Document Title"
author: "Single Author"
# or multiple:
authors:
  - Alice
  - Bob
lang: zh  # auto-detected if omitted
toc: true
---
```

## Custom Templates

Templates are Typst files that define an `#article` function:

```typst
#let article(
  title: none,
  authors: (),
  lang: "en",
  toc: false,
  body,
) = {
  // your styling here
  body
}
```

See `src/template/modern_tech.typ` and `src/template/classic_editorial.typ` for examples.

## Architecture

```
Markdown ──comrak──→ AST ──convert──→ Typst markup
                                         │
                            LaTeX math ──tex2typst-rs──→ Typst math
                                         │
                              template ──compose──→ full .typ source
                                         │
                          typst crate ──compile──→ PDF bytes
```

## License

MIT
