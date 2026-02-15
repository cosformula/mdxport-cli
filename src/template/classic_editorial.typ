#let title-fonts = ("Noto Serif CJK SC", "Noto Serif SC", "Times New Roman", "Georgia", "Libertinus Serif")
#let body-fonts = ("Noto Serif CJK SC", "Noto Serif SC", "Times New Roman", "Georgia", "Libertinus Serif")
#let code-fonts = ("IBM Plex Mono", "JetBrains Mono", "DejaVu Sans Mono", "Consolas")

#let article(
  title: none,
  authors: (),
  lang: "en",
  toc: false,
  body,
) = {
  set text(font: body-fonts, size: 11pt, lang: lang)
  set page(
    paper: "us-letter",
    margin: (x: 22mm, y: 22mm),
    numbering: "1",
  )

  set heading(
    numbering: "1.",
    supplement: none,
  )
  show heading: set text(font: title-fonts, weight: "regular")
  set par(justify: true)

  show raw.where(block: true): set text(font: code-fonts, size: 10pt, fill: luma(40%))
  show raw.where(block: false): set text(font: code-fonts, size: 10pt, fill: luma(40%))
  show quote: block.with(
    fill: luma(248),
    stroke: (left: 2pt + black),
    inset: 1.2em,
  )
  show table: set table(
    stroke: 0.6pt,
    fill: (x, y) => if calc.odd(y) { luma(250) } else { white },
  )
  show link: underline

  if title != none and title != "" {
    align(center, text(weight: "bold", size: 32pt, lang: lang)[#title])
    v(1.1em)
  }

  if authors != () {
    align(center, {
      for i in range(authors.len()) {
        text(size: 11pt, lang: lang)[#authors.at(i)]
        if i + 1 < authors.len() {
          ", "
        }
      }
    })
    v(1.2em)
  }

  if toc {
    if lang == "zh" {
      heading(level: 1, outlined: false)[目录]
    } else {
      heading(level: 1, outlined: false)[Table of Contents]
    }
    outline()
    v(1.6em)
  }

  body
}
