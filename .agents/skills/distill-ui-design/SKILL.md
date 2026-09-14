---
name: distill-ui-design
description: Design, implement, or review the Distill web UI using the project's Delta-inspired typography, palette, spacing, and component guidance. Use for visual UI work in this repository.
---

# Distill UI design

Use Delta's editorial typography, warm surfaces, rust accents, and precise borders as the
visual direction for Distill. Adapt the reference to the requested screen and its information
density.

Read [the design reference](references/delta-design.md) for measured typography, color roles,
layout, and interaction rules. It distinguishes observed Delta values from guidance for Distill.

## Apply to this project

The web UI lives in `apps/web` (React + Vite + Tailwind v4, see
[spec D9](../../../spec/2026-09-13-distill-foundation/README.md)). Read the target component and
[the app stylesheet](../../../apps/web/src/styles.css) before editing. The stylesheet owns the live
tokens; the reference describes the design direction. Use
[the web changelog](../../../apps/web/CHANGELOG.md) to check shipped UI behavior.

- If the stylesheet has no tokens yet, define semantic roles first — for example `canvas`,
  `panel`, `ink`, `line`, and `brand` — and map the reference palette into them. Once they exist,
  keep them; don't add a parallel token system.
- Switch themes with a `data-theme` attribute and scoped overrides. Use Tailwind's plain `@theme`;
  `@theme inline` would bake values into utilities and break palette switching.
- Inspect existing font imports before adding families. The reference fonts are a target, not
  evidence that those fonts are installed. Use properly licensed font packages or local assets;
  do not hotlink Delta's CDN. `distill ui` serves the app offline, so fonts must ship with it.
- Apply changes within the requested surface. A component task is not a request to replace the
  app shell, change the default theme, or migrate every screen.
- Reuse the app's components and installed icon and motion libraries. Keep status colors tied to
  their meanings — a note that needs conflict review, a topic asked many times, a note you
  haven't annotated. A brand accent is not a status indicator.

[The extracted CSS](assets/delta-design-tokens.css) is an adaptation asset, not a stylesheet to
import wholesale. It contains original color formulas and proposed aliases for measured page
values. Adapt its `.dark` mappings to the app's theme selector.

## Check the result

For rendered UI changes, inspect the affected screen at desktop and narrow widths in both
supported themes. Check font loading, text wrapping, navigation selection, keyboard focus, and
the relevant hover states. Keep readable contrast if a reference value does not work on the
target background. Dense screens such as topic lists and stats need their own layout; Delta's
documentation columns are not a required app shell.

Follow [the repository rules](../../../AGENTS.md) for validation and change records.
