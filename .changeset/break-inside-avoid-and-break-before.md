---
"vellora": minor
"@vellora/native": minor
---

The native paginator honours `break-inside: avoid` and `break-before: page` (and the legacy `page-break-inside: avoid` / `page-break-before: always`). Stylo drops these Gecko-only properties, so vellora reads them from top-level `<style>` rules and inline `style`, resolves the selectors with Stylo's selector matching, and keeps an items-table `<tbody>` group, an items table, or a wrapper section on one page, or starts a block, `<tbody>` or `<tr>` on a new page. A kept group taller than an empty page falls back to its rows, so no content is lost.
