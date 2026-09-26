# Changelog

## [Unreleased]

### Changed
- The sidebar holds two tabs, Distill and Review, with every tag listed below them. Distill
  is a timeline of notes (question plus the first two lines of the conclusion, expanding in
  place to the whole note); Review is a dashboard of activity metrics. The separate home,
  notes, topics, tags and stats pages are gone; their content moved into these two.

### Added
- Web UI served by `distill ui`: home, notes search, note page, topics with timelines, tags,
  stats and a problems page. See `README.md`.
- Annotations can be added, edited and deleted on the note page; topics can be renamed and
  merged; sync conflicts can be settled by keeping one copy.
- ⌘K palette over notes, topics and tags; light and dark themes following the system setting.
- Duplicate topics: a "可能重复" view on the Topics page with merge and dismiss, a home-page
  banner, and "相似的 topic" on topic pages. Needs `distill model pull`.
- Pages refetch when the server reports a vault change, so notes saved from an agent appear
  without a reload.
