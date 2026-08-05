# Recovery archive: map UI layout and search lenses

This archive preserves the exact local Git history that GitHub's HTTPS OAuth token could not reference directly because an ancestor contains a workflow-file change.

- Source branch: `agent/martbot-mbp-m4-max-128gb/codex-root/map-ui-layout-search-lenses-20260804T120910Z-0302`
- Source head: `9cf4360c34e512bcc86b87c86e9d1734a6affba4`
- Common GitHub base: `39cc1e8b2aa7141436f4049e170a2af0c0f4d89f`
- Bundle SHA-256: `5d7f19ff1b77a1e4df4c88cc4d9b7465ed001cb6972b08c534d8f31ad23ebdb3`

Restore the bundle after downloading this file:

```sh
gzip -dc map-ui-layout-search-lenses.bundle.gz > map-ui-layout-search-lenses.bundle
git bundle verify map-ui-layout-search-lenses.bundle
git fetch map-ui-layout-search-lenses.bundle \
  refs/heads/agent/martbot-mbp-m4-max-128gb/codex-root/map-ui-layout-search-lenses-20260804T120910Z-0302
```
