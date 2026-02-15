---
title: "E2E Full Feature Test"
authors:
  - Alice
  - Bob
lang: en
toc: true
---

# Introduction

This document tests the **complete pipeline** from Markdown to PDF.

## Inline Formatting

**Bold**, *italic*, ***bold italic***, ~~strikethrough~~, `inline code`.

A [hyperlink](https://example.com) and an image reference ![logo](logo.png).

## Mathematics

Inline: $a^2 + b^2 = c^2$

Display:

$$
\sum_{n=1}^{\infty} \frac{1}{n^2} = \frac{\pi^2}{6}
$$

## Code

```javascript
function hello() {
  console.log("Hello, world!");
}
```

## Table

| Feature | Status | Notes |
|---------|--------|-------|
| Headings | ✅ | All levels |
| Math | ✅ | LaTeX syntax |
| Tables | ✅ | GFM style |
| Footnotes | ✅ | Reference style |

## Lists

- [x] Task completed
- [ ] Task pending
- Regular item

1. First
2. Second
   1. Nested

## Blockquote

> This is a blockquote.
> It spans multiple lines.

---

## Footnotes

Here is a footnote reference[^note].

[^note]: This is the footnote content.

## Final

End of test document.
