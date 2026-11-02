# m8-guide-build: user guide generator

The user guide is Markdown pages in `docs/guide/`. This task writes the program that turns them into static HTML with an offline search box: the new crate `drydock-guide` (library plus the command `drydock-guide SRC_DIR OUT_DIR`). The skeleton is on main (`app/crates/drydock-guide/src/lib.rs`, `main.rs`, with the `pulldown-cmark` dependency and its license row). Real guide text is `m8-guide-text`; here the tests use two sample pages. The pages are static: no file access from the page, nothing fetched from anywhere.

Fill in the skeleton's functions. Tests are in `tests/render.rs` (pure functions) and `tests/build.rs` (whole build and the command). Do not change the tests. Do not add dependencies: only `pulldown-cmark` (std for everything else, JSON and HTML escaping are written by hand). Create `app/crates/drydock-guide/assets/style.css` and `assets/search.js` and embed them with `include_str!`.

## Contents file `SUMMARY.md`

`parse_summary` reads it line by line (line numbers in errors start at 1).
- Blank lines and lines starting with `#` are ignored.
- Every other line is `<indent>- [Title](path.md)`. Indent is spaces, a multiple of 2; `level` = indent / 2. A tab, an odd indent, or a level more than one deeper than the previous entry (or the first entry not at level 0) is an error.
- `path` ends in `.md`, is relative, has no `..` segment, no scheme and no leading `/`. The same path twice is an error (reported on the second line).
- Anything else (prose, `*` bullets, a link without a path) is `GuideError::Summary { line, message }`.
- Entries come back in file order; that order is the sidebar order and the page order.

## `render_markdown`

Use `pulldown_cmark` with tables, strikethrough and footnotes-free defaults (tables and strikethrough on).
- **Raw HTML**: block and inline HTML events are emitted as escaped text (`&lt;script&gt;`), so a page cannot add script or markup.
- **Headings**: each gets `id` = `heading_id(plain text)`; a repeat on the same page gets `-1`, `-2` ... appended to the base (`getting-started`, `getting-started-1`). Output `<h2 id="x">inline html</h2>`. `Heading.text` is the plain text (code and emphasis marks dropped).
- **title**: text of the first level-1 heading, `""` if none.
- **links / images**: every link and image target, as written, in document order (`links` and `images` separately).
- **body_html**: standard pulldown HTML except as above. A link whose target is relative (no `scheme:`, not starting with `#` or `/`) and whose path part ends in `.md` has that `.md` changed to `.html`, fragment kept. Other targets are untouched.
- **sections**: one per heading of level 1 to 3, in order. Text of deeper headings (4 to 6), paragraphs, list items, code blocks, code spans, table cells and link text belongs to the open section, including the deeper headings' own text. Image alt text is not included. Text before the first heading is dropped. `Section.text` is the pieces joined with single spaces, whitespace collapsed, trimmed. `Section.id` is the heading's anchor.

## `heading_id`

Lower-case; keep Unicode alphanumerics; every other run of characters becomes one `-`; no leading or trailing `-`; `section` if empty.

## `sidebar_html(entries, current)`

`<nav class="toc">` containing nested `<ul><li>` lists following `level` (a deeper entry opens a `<ul>` inside the previous `<li>`; lists and items are all closed; with no entries an empty nav, no `<ul>` needed). Each link is `<a href="HREF">Title</a>`, and `<a class="current" href="HREF">Title</a>` for the entry whose path equals `current`. HREF is the entry's path with `.md` changed to `.html`, made relative to the folder of `current` (`drawing/lines.md` from `welcome.md` is `drawing/lines.html`; `welcome.md` from `drawing/lines.md` is `../welcome.html`). Titles are HTML-escaped.

## Search records

`index_entries(page_url, page)`: one `IndexEntry` per section: `title` = page title, `heading` = section heading, `url` = `page_url` for the level-1 section (the page's first section when it is the level-1 heading) and `page_url#id` for the rest, `text` = section text. Simplest rule: the section whose id equals the level-1 heading's id gets the bare url.

`index_json`: a JSON array with no whitespace, objects with keys in the order `title, heading, url, text`; strings escape `"` and `\` with a backslash, `\n`, `\t`, `\r`, other control characters below 0x20 as `\u00xx` (4 hex digits, lower-case); everything else, including non-ASCII, is written as is. Empty list: `[]`.

## `build(src, out)`

1. Read `src/SUMMARY.md` (missing file: `GuideError::Summary { line: 0, .. }`).
2. Every entry's file must exist, else `MissingPage { path }` (the contents path).
3. Render every listed page. Collect, from every other file under `src` (recursively, sorted), the non-`.md` files except `SUMMARY.md` as assets, and `.md` files not listed (other than `SUMMARY.md`) as orphans (sorted, relative `/` paths). Orphans are not rendered or checked.
4. Check every page before writing anything:
   - Image target with a scheme (`http:`, `https:`, `data:`...) or starting with `/`: `ExternalImage`. Otherwise it is resolved against the page's folder (`..` allowed, normalised); outside `src` or no such file: `MissingImage`.
   - Link target: scheme links (`https:`, `mailto:`...) are not checked. `#x` must be an anchor of the same page (`BrokenAnchor`). Otherwise split at `#`; resolve the path against the page's folder; one that leaves `src`, starts with `/`, or does not exist is `BrokenLink`. A `.md` target must also be listed in the contents (else `BrokenLink`), and its fragment, if any, must be an anchor of that page (`BrokenAnchor`). Other existing files (images, samples) are fine.
   - `page` in errors is the page's contents path; `target` is the link as written. First error wins, pages checked in contents order, links before images within a page.
   - `Display` for `GuideError` gives one line naming the page and target (or the line number for `Summary`, the path for `MissingPage`).
5. Write into `out` (created, with parent folders; existing files are overwritten, nothing is deleted). Files are written only after all checks passed.
   - Each page at its path with `.md` changed to `.html`.
   - `index.html`: a page with `<meta http-equiv="refresh" content="0; url=FIRST.html">` and a link `<a href="FIRST.html">` to the first page; no script. (If the contents are empty, `index.html` says the guide is empty.)
   - `style.css`, `search.js` (from `assets/`), `search-index.json` (`index_json` of all pages' records in contents order) and `search-index.js` = `window.DRYDOCK_SEARCH = ` + the same JSON + `;\n`. The script file exists because browsers refuse to `fetch` a JSON file from a `file://` page.
   - Assets copied byte for byte to the same relative path.
   - Deterministic: same input gives the same bytes.
6. `BuildReport` as documented.

### Page frame

```
<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>TITLE - Drydock User Guide</title>
<link rel="stylesheet" href="ROOTstyle.css">
<script src="ROOTsearch-index.js"></script>
<script src="ROOTsearch.js"></script>
</head>
<body data-root="ROOT">
<header><input type="search" id="search" placeholder="Search the guide" aria-label="Search the guide"><ul id="search-results"></ul></header>
SIDEBAR
<main>
BODY
</main>
</body>
</html>
```
`ROOT` is `""` for a page in the root and `"../"` per folder level otherwise. TITLE is the page title, or the contents title if the page has no level-1 heading (HTML-escaped). One element per line is not required except that the strings the tests look for appear exactly.

### `search.js`

Plain JavaScript, no libraries, no `fetch`, `XMLHttpRequest`, `import(` or `WebSocket`, no `http` URLs. It reads `window.DRYDOCK_SEARCH`, listens to `input` on `#search`, and fills `#search-results` with `<li><a>` items. Matching: lower-case the query, split on whitespace; a record matches when every word occurs in its heading or text (case-insensitive); heading matches rank above text-only matches; at most 20 results. Each link goes to `document.body.dataset.root + record.url`, shows heading and page title (use `textContent`, never `innerHTML`); an empty query clears the list. `style.css`: plain layout (sidebar left, content right, narrow-screen friendly, `.current` highlighted), no `@import`, no `url(http...)`. Neither file is tested beyond the strings above; keep them short and readable.

### Command

`drydock-guide SRC_DIR OUT_DIR`. Success: exit 0, print `drydock-guide: built N pages` (and a line `drydock-guide: not in the contents: PATH` per orphan) to stdout. Wrong argument count: print `usage: drydock-guide SRC_DIR OUT_DIR` to stderr, exit 2. Build error: print `drydock-guide: ` + the error to stderr, exit 1.

## Where to look

- `show GuideError`, `show SummaryEntry`, `show Rendered`, `show IndexEntry`, `show BuildReport` in `drydock-guide/src/lib.rs` (the types are fixed; the skeleton's function signatures too).
- `pulldown_cmark::{Parser, Options, Event, Tag, TagEnd, CowStr, html::push_html}` for rendering; map events to your own output, rewrite link/heading events before `push_html`. `pulldown_cmark_escape::escape_html` is available through `pulldown_cmark::escape` (check the 0.13 docs in the registry source under `~/.cargo/registry`).
- Tests' helpers: `show Guide` (a temp folder with `build`, `out_text`, `write`), `toc_hrefs`, `tree` in `tests/common/mod.rs`; the sample pages `WELCOME`, `LINES`, `SUMMARY` there.
