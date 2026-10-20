---
"@vellora/native": patch
---

Resolve per-platform prebuild packages with string-literal `require`s instead of a computed specifier, so bundlers (webpack) and file tracers (`@vercel/nft`) can find the `.node` addon statically. Reported and diagnosed by @AnOrdinaryPeople in #33.
