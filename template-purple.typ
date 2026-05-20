// ============================================================
//  template-purple.typ  –  Obsidian-branded document template
//  for use with md2typst.
//
//  Usage:
//    md2typst input.md output.pdf --template template-purple.typ
//
//  Expected frontmatter keys (all optional, fall back to ""):
//    title      Document title
//    subtitle   Sub-title / project description
//    author     Author name(s)
//    version    Version number, e.g. "1.0"
//    status     Document status, e.g. "Concept" / "Definitief"
//    date       Date string, e.g. "2026-05-14"
//    summary    Executive summary (single paragraph)
//    language   Language code, e.g. "nl" (default) or "en"
//
//  Required exports consumed by md2typst:
//    template   Show-rule function  (#show: template.with(fm: fm))
//    callout    Renders Obsidian callout blocks
// ============================================================

// ── Colours ──────────────────────────────────────────────────
#let accent = rgb("#a90061")   // violet — headings, links, table headers
#let bar    = rgb("#42145f")   // purple — left sidebar on non-content pages

// ── Callout colours & function ───────────────────────────────
#let _co-colors = (
  "note":      (fill: rgb("#dbeafe"), accent: rgb("#1d4ed8")),
  "info":      (fill: rgb("#dbeafe"), accent: rgb("#1d4ed8")),
  "tip":       (fill: rgb("#dcfce7"), accent: rgb("#15803d")),
  "hint":      (fill: rgb("#dcfce7"), accent: rgb("#15803d")),
  "important": (fill: rgb("#dcfce7"), accent: rgb("#15803d")),
  "warning":   (fill: rgb("#fef9c3"), accent: rgb("#a16207")),
  "caution":   (fill: rgb("#fef9c3"), accent: rgb("#a16207")),
  "attention": (fill: rgb("#fef9c3"), accent: rgb("#a16207")),
  "danger":    (fill: rgb("#fee2e2"), accent: rgb("#b91c1c")),
  "error":     (fill: rgb("#fee2e2"), accent: rgb("#b91c1c")),
  "bug":       (fill: rgb("#fee2e2"), accent: rgb("#b91c1c")),
  "quote":     (fill: rgb("#f1f5f9"), accent: rgb("#475569")),
  "cite":      (fill: rgb("#f1f5f9"), accent: rgb("#475569")),
)

#let callout(kind, title, body) = {
  let c = _co-colors.at(kind, default: (fill: rgb("#f3f4f6"), accent: rgb("#374151")))
  block(fill: c.fill, inset: (x: 12pt, y: 10pt), radius: 4pt, width: 100%)[
    #text(fill: c.accent, weight: "bold")[#title] \
    #body
  ]
}

// ── Language strings ─────────────────────────────────────────
// Add entries here to support additional languages.
// Set  language: <code>  in the document frontmatter to activate.
#let _lang_strings = (
  "nl": (
    toc:      "Inhoudsopgave",
    figures:  "Lijst van figuren",
    summary:  "Samenvatting",
    version:  "Versie",
    status:   "Status",
    date:     "Datum",
    author:   "Auteur",
    page:     "Pagina",
    of:       "van",
    fig_nr:   "Figuur",
    fig_desc: "Omschrijving",
  ),
  "en": (
    toc:      "Table of Contents",
    figures:  "List of Figures",
    summary:  "Summary",
    version:  "Version",
    status:   "Status",
    date:     "Date",
    author:   "Author",
    page:     "Page",
    of:       "of",
    fig_nr:   "Figure",
    fig_desc: "Description",
  ),
)

// ── Sidebar helper ────────────────────────────────────────────
// Places the purple bar flush left, full page height.
// Call inside any zero-margin page block that should show the bar.
#let _sidebar = place(left + top,
  rect(width: 1.2cm, height: 100%, fill: bar)
)

// ── Template function ─────────────────────────────────────────
// fm   — frontmatter dictionary emitted by md2typst
// doc  — document body (all content blocks)
#let template(fm: (:), doc) = {

  // Extract metadata with safe defaults
  let title    = fm.at("title",    default: "")
  let subtitle = fm.at("subtitle", default: "")
  let author   = fm.at("author",   default: "")
  let version  = fm.at("version",  default: "")
  let status   = fm.at("status",   default: "")
  let date     = fm.at("date",     default: fm.at("published", default: ""))
  let summary  = fm.at("summary",  default: "")
  let _lang    = fm.at("language", default: "nl")
  let _t       = _lang_strings.at(_lang, default: _lang_strings.at("nl"))

  // ── Page layout ─────────────────────────────────────────────
  set page(
    paper: "a4",
    margin: (top: 2.5cm, bottom: 2.5cm, left: 2.8cm, right: 2.5cm),
    header: context {
      // Non-content pages use explicit page() with header: none, so this
      // rule only fires on numbered body pages.
      if counter(page).get().first() > 2 {
        set text(size: 8pt, fill: rgb("#888888"))
        grid(
          columns: (1fr, 1fr),
          align(left)[#title],
          align(right)[#_t.version #version],
        )
        line(length: 100%, stroke: 0.4pt + rgb("#cccccc"))
      }
    },
    footer: context {
      if counter(page).get().first() > 2 {
        set text(size: 8pt, fill: rgb("#888888"))
        line(length: 100%, stroke: 0.4pt + rgb("#cccccc"))
        let current = counter(page).get().first()
        let total   = counter(page).final().first()
        grid(
          columns: (1fr, 1fr),
          align(left)[#status],
          align(right)[#_t.page #current #_t.of #total],
        )
      }
    },
  )

  // ── Typography ───────────────────────────────────────────────
  set text(font: ("Helvetica Neue", "Arial", "Liberation Sans"), size: 10.5pt, lang: _lang)
  set par(justify: true, leading: 0.65em)
  set heading(numbering: "1.1.")

  show heading.where(level: 1): it => {
    v(1.4em)
    text(size: 17pt, weight: "bold", fill: accent, it.body)
    v(0.5em)
  }
  show heading.where(level: 2): it => {
    v(1.0em)
    text(size: 13pt, weight: "bold", fill: accent, it.body)
    v(0.3em)
  }
  show heading.where(level: 3): it => {
    v(0.8em)
    text(size: 11pt, weight: "bold", fill: rgb("#333333"), it.body)
    v(0.2em)
  }

  set list(indent: 1em, marker: [•])
  set enum(indent: 1em)

  // ── Tables ───────────────────────────────────────────────────
  set table(
    stroke: (_, y) => if y == 0 { (bottom: 1.5pt + accent) }
                      else       { (bottom: 0.5pt + rgb("#dddddd")) },
    fill:   (_, y) => if y == 0 { rgb("#f3e6f5") } else { none },
    inset: (x: 8pt, y: 6pt),
  )
  show table.cell.where(y: 0): set text(weight: "bold", fill: accent)
  show link: set text(fill: accent)

  // ── Code blocks ──────────────────────────────────────────────
  show raw.where(block: true): it => block(
    fill: luma(245), inset: (x: 10pt, y: 8pt), radius: 4pt, width: 100%, it,
  )
  show raw.where(block: false): it => box(
    fill: luma(245), inset: (x: 3pt, y: 1pt), radius: 2pt, it,
  )

  // ── Block quotes ─────────────────────────────────────────────
  show quote.where(block: true): it => block(
    stroke: (left: 1pt + rgb("#d4a8d4")),
    inset: (left: 1em, top: 0.4em, bottom: 0.4em),
    width: 100%,
    it.body,
  )

  // ── Cover page ───────────────────────────────────────────────
  page(
    margin: (top: 0pt, bottom: 0pt, left: 0pt, right: 0pt),
    header: none,
    footer: none,
  )[
    // Left purple bar
    #_sidebar

    // Obsidian logo — centred at the top of the page
    #place(top + center, dy: 1.5cm,
      image("obsidian.svg", height: 3cm)
    )

    // Content — pushed down to clear the logo
    #pad(left: 2.5cm, right: 2.5cm, top: 6cm)[
      // Title
      #text(size: 26pt, weight: "bold", fill: accent, title)
      #v(0.4cm)
      #line(length: 80%, stroke: 2pt + accent)
      #v(0.4cm)

      // Subtitle — between the thick line and the metadata grid
      #if subtitle != "" [
        #text(size: 11pt, fill: rgb("#666666"), subtitle)
        #v(0.8cm)
      ] else [
        #v(0.8cm)
      ]

      // Metadata grid
      #grid(
        columns: (4cm, 1fr),
        row-gutter: 0.5em,
        text(weight: "bold", fill: rgb("#555555"), _t.version), version,
        text(weight: "bold", fill: rgb("#555555"), _t.status),  status,
        text(weight: "bold", fill: rgb("#555555"), _t.date),    date,
        text(weight: "bold", fill: rgb("#555555"), _t.author),  author,
      )
      #v(2cm)

      // Summary box (only shown when frontmatter has a summary key)
      #if summary != "" [
        #rect(
          width: 85%,
          inset: 14pt,
          radius: 4pt,
          stroke: 1pt + rgb("#d4a8d4"),
          fill: rgb("#f8f0ff"),
        )[
          #text(weight: "bold", fill: accent)[#_t.summary]
          #v(0.4em)
          #summary
        ]
      ]
    ]
  ]

  // ── Revision / Approval page (optional) ─────────────────────
  // Shown when frontmatter contains revision-table and/or approval-table keys
  // naming an H2 section. The sections are extracted from the document body
  // and rendered here; they are not numbered and do not appear in the ToC.
  let _rev  = fm.at("revision-blocks",  default: none)
  let _appr = fm.at("approval-blocks", default: none)
  if _rev != none or _appr != none {
    page(
      margin: (top: 0pt, bottom: 0pt, left: 0pt, right: 0pt),
      header: none,
      footer: none,
    )[
      #_sidebar
      #pad(left: 2.5cm, right: 2.5cm, top: 2.5cm, bottom: 2.5cm)[
        #if _rev != none [
          #text(size: 13pt, weight: "bold", fill: accent)[#fm.at("revision-table", default: "")]
          #v(0.8em)
          #_rev
          #if _appr != none { v(2em) }
        ]
        #if _appr != none [
          #text(size: 13pt, weight: "bold", fill: accent)[#fm.at("approval-table", default: "")]
          #v(0.8em)
          #_appr
        ]
      ]
    ]
  }

  // ── Table of contents ────────────────────────────────────────
  page(
    margin: (top: 0pt, bottom: 0pt, left: 0pt, right: 0pt),
    header: none,
    footer: none,
  )[
    #_sidebar
    #pad(left: 2.5cm, right: 2.5cm, top: 2.5cm, bottom: 2.5cm)[
      #text(size: 17pt, weight: "bold", fill: accent)[#_t.toc]
      #v(1.2em)
      #outline(title: none, indent: auto, depth: 3)
    ]
  ]

  // ── Document body ────────────────────────────────────────────
  doc
}
