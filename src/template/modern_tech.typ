#let title-fonts = ("IBM Plex Sans", "Inter", "PingFang SC", "Hiragino Sans GB", "Noto Sans CJK SC", "Noto Sans SC", "Libertinus Serif")
#let body-fonts = ("IBM Plex Sans", "Inter", "PingFang SC", "Hiragino Sans GB", "Noto Sans CJK SC", "Noto Sans SC", "Libertinus Serif")
#let code-fonts = ("JetBrains Mono", "DejaVu Sans Mono", "SFMono-Regular", "Consolas", "Menlo")

#let article(
  title: none,
  authors: (),
  lang: "en",
  toc: false,
  body,
) = {
  set text(font: body-fonts, lang: lang)
  set page(
    paper: "us-letter",
    margin: (x: 16mm, y: 18mm),
    numbering: "1",
  )

  set heading(
    numbering: "1.1",
    supplement: none,
  )
  show heading: set text(font: title-fonts, weight: "semibold")
  set par(justify: true)

  show raw.where(block: true): set text(font: code-fonts, size: 9.5pt)
  show raw.where(block: false): set text(font: code-fonts, size: 9.5pt)
  show link: set text(fill: rgb("#1E88E5"))
  show quote: block.with(
    fill: luma(245),
    stroke: 0.8pt + rgb("#9E9E9E"),
    inset: 1em,
  )
  show table: set table(
    stroke: 0.5pt,
    inset: 6pt,
    fill: (x, y) => if calc.odd(y) { luma(251) } else { white },
  )

  if title != none and title != "" {
    align(center, text(weight: "bold", size: 30pt, lang: lang)[#title])
    v(1.2em)
  }

  if authors != () {
    align(center, {
      for i in range(authors.len()) {
        text(size: 10pt, lang: lang)[#authors.at(i)]
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
