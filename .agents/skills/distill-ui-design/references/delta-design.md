# Delta design reference

This is Distill's chosen UI design reference, extracted from
[Delta's getting-started page](https://delta.dev/docs/getting-started) on 2026-09-18.
Measurements came from rendered desktop styles and
[the page stylesheet](https://delta.dev/docs/_astro/docs-layout.DF149mf_.css).
They describe that page, not a published Delta brand standard or Distill's current implementation.

## Visual direction

Combine serif headings with typewriter-like body text. Use warm paper surfaces, dark ink,
rust accents, and thin rectangular frames. Establish hierarchy through font family, size,
and whitespace; keep most text at regular weight.

## Typography

| Role | Family | Size / line height | Weight |
|---|---|---|---|
| Page title | IBM Plex Serif | 36px / 45px | 400 |
| Section heading | IBM Plex Serif | 28px / 32.2px | 400 |
| Featured card title | IBM Plex Serif | 24px / 30px | 500 |
| Small card title | IBM Plex Serif | 18px / 22.5px | 500 |
| Article body | iA Writer Quattro S | 16px / 28px | 400 |
| Card description | iA Writer Quattro S | 14px / 20–24px | 400 |
| Sidebar link | iA Writer Quattro S | 13px / 16px | 400 |
| Navigation group label | iA Writer Quattro S | 11px / 16px | 500 |
| Inline code | Lilex | 14px / 24.5px | 400 |

Headings use −0.025em tracking. Navigation group labels are uppercase with wide tracking.
The page title is 30px below the 640px breakpoint. Quattro supplies the writing-tool character;
it is distinct from the true monospace used for code. Reserve serif type for headings rather
than applying it to every control. For Distill, treat these sizes as starting points and keep compact
controls usable; the 11px label is not a body-text standard.

## Palette

Hex values below are rounded sRGB equivalents. The
[CSS asset](../assets/delta-design-tokens.css) owns the reusable OKLCH formulas and theme mappings.

| Role | Approximate hex | Observed use |
|---|---|---|
| Paper surface | `#FAF8F5` | Main panel and sidebars |
| Warm canvas | `#F1EAE0` | Navigation cards |
| Hover surface | `#F8F4F0` | Hovered cards |
| Primary ink | `#2F3339` | Headings and prominent labels |
| Secondary ink | `#545A64` | Body text and descriptions |
| Border | `#AFB3B9` | Frames and separators |
| Brand rust | `#D5442C` | Icons and accents |
| Selected background | `#BE3C26` | Current navigation item |
| Link rust | `#9D301E` | Text links and card actions |
| Blue | `#346DAE` | Supporting stylesheet token |
| Error | `#B63132` | Semantic stylesheet token |
| Success | `#227849` | Semantic stylesheet token |
| Warning | `#AB6F00` | Semantic stylesheet token |

Use the darker link shade for text rather than treating every rust value as interchangeable.
Keep saturated color focused on actions, selection, and meaningful status. The dark theme uses
near-black surfaces, light neutral text, and the same rust family; it is not a simple inversion
of the light palette. Its mappings were extracted from CSS, not visually audited here.

## Layout and rhythm

| Measurement | Observed value |
|---|---|
| Desktop sidebars | 284px each |
| Outer inset and panel gaps | 16px |
| Article maximum width | 768px |
| Article horizontal inset | 16px small; 40px at 1024px; 48px at 1280px |
| Prose block gap | 24px |
| Section heading top margin | 48px |
| Card padding | 20–24px |
| Card gap | 12px |

The documentation uses three framed columns on desktop. Its CSS switches to desktop sidebars
at 1024px and uses mobile navigation plus an inline table of contents below that width.
At 1536px, the center grid track is capped at 832px. These responsive rules were read from CSS;
mobile behavior still needs browser validation when adapted.

For Distill, retain the spacing rhythm while fitting the task: a topic list, a note page, and
the stats dashboard need different column widths. The note page is closest to Delta's article
layout. Do not force a table of contents or paired
sidebars onto a screen that has no use for them.

## Components and interaction

| Element | Observed treatment |
|---|---|
| Panels | 1px border, 2px radius, no decorative shadow |
| Selected navigation | Rust fill fitted to the label, light text, 1px radius |
| Cards | Warm fill, thin border, serif title, outlined icon, text action with arrow |
| Featured card | Full row above two smaller cards |
| Icon containers | 36px small or 44px featured, faint accent fill and thin outline |
| Inline code | Thin border, square corners, 2px vertical and 6px horizontal padding |
| Section headings | Horizontal separator below the text |
| Card hover | Background and border-color transition over 150ms |
| Keyboard focus | Visible accent ring |

Global radius tokens are 0px; the documentation adds 2px on its panels and cards. Preserve that
nearly square geometry when applying this style. Prefer flat fills and precise lines over
large rounding, soft floating shadows, or decorative glass effects.

Use compact label selection where it fits navigation. Give buttons and touch targets enough
interactive area even when the painted selection is small. Keep selected, hover, and focus
states distinguishable. Respect reduced motion when adding transitions.

The faint outer texture and segmented top stripe are optional reference details. Keep such
decoration outside the reading area. They are not required on every Distill screen, and this skill
does not supply Delta logos, artwork, or font binaries.
