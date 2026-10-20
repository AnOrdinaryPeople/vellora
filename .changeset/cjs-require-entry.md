---
"vellora": patch
"@vellora/native": patch
"@vellora/lint": patch
"@vellora/engine-chromium": patch
---

`require("vellora")` from CommonJS projects (e.g. NestJS compiled with `"module": "commonjs"`) no longer fails with `ERR_PACKAGE_PATH_NOT_EXPORTED`. The library packages' `exports` maps now carry a `default` condition that points at the same ESM entry, so Node loads it through `require(esm)` (Node 20.19+ / 22.12+; older minors still need `import()`). There is still a single ESM build, so `setNativeBridge` and other module state are shared between `import` and `require` callers.
