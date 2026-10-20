---
"vellora": patch
"@vellora/native": patch
"@vellora/lint": patch
---

The strict gate now rejects the `@page` margin boxes the native renderer does not draw (`@bottom-right`, `@top-left-corner`, `@left-middle` and the other eleven) as `css:page-margin-box`, instead of rendering the PDF without them. Only `@top-center` and `@bottom-center` are drawn. `@vellora/lint` reports the same construct as the `page-margin-box` rule.
