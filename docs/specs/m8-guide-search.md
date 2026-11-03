# m8-guide-search: the guide's search box finds pages

Typing in the guide's search box does nothing. The page template puts the two search scripts in
`<head>`; `search.js` runs at once, `getElementById("search")` is still null, and it returns
before attaching its listener.

## Change

1. In the page template (`drydock-guide/src/lib.rs`, the `format!` that builds each page), make
   both tags deferred, keeping the index first:
   `<script defer src="{root}search-index.js"></script>` then
   `<script defer src="{root}search.js"></script>`.
2. `assets/search.js`: put everything that touches the page (the lookups of `#search` and
   `#search-results`, the early return, the `input` listener) inside a function named `setup`.
   At the end: if `document.readyState === "loading"`, call
   `document.addEventListener("DOMContentLoaded", setup)`, else call `setup()`. `records` may be
   read inside `setup` too. Keep the matching, ranking and result links as they are; links stay
   `document.body.dataset.root + record.url`, which resolves from pages in subfolders (the index
   urls are relative to the guide root, the root prefix is `../` on `drawing/lines.html`).
3. Update nothing else. `tests/build.rs` already expects the `defer` tags (changed in the spec
   commit).

## Tests (already in the spec commit)

- `tests/search_script.rs`: on both sample pages the index tag comes before `search.js`, both
  are deferred (or follow the `#search` input), `search.js` contains the `readyState` /
  `DOMContentLoaded` setup with the lookup inside `function setup`, and result links use
  `dataset.root` with the deep page's `data-root="../"`.
- `tests/build.rs::page_frame_links_assets_relative_to_the_page`: exact `defer` tags.

Run `harness/bin/run-tests -p drydock-guide`.

## Where to look

- `show build_pages` / the `format!` around line 725 of `crates/drydock-guide/src/lib.rs` (page template; `grep -n "search.js"`).
- `crates/drydock-guide/assets/search.js` (whole file, 60 lines).
- Test helpers: `tests/common/mod.rs` (`Guide::sample`, `build`, `out_text`).
