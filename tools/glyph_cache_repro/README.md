# glyph_cache_repro

Committed reproduction of the glyph-outline disk-cache spike for
`hwpx-template-engine`'s `docs/RHWP_GLYPH_OUTLINE_CACHE_PLAN.md`. See
`src/main.rs` module docs for full rationale.

The original numbers cited in that plan doc (55.4x, 9.084ms→0.164ms,
180.7KB, hit-rate table) came from a throwaway scratchpad project that was
never committed anywhere — this crate replaces that with a real, runnable
one, pinned to the exact dependency versions `usvg` 0.45.1 uses.

## Run

```bash
HWPX_FONTS_DIR=/path/to/hwpx-template-engine/src/main/resources/fonts \
  cargo run --release -- build
HWPX_FONTS_DIR=/path/to/hwpx-template-engine/src/main/resources/fonts \
  cargo run --release -- load
```

## Measured results (2026-08-02, macOS aarch64, real project fonts + templates)

Working set: all 12 project fonts (`hcr`/`kopub`) × all 153 unique visible
characters found in `scslic.hwpx` + `foobar.hwpx`'s static section-XML text
→ **1530 (font, glyph) pairs** that resolve to a real glyph (a superset of
what any single rendered document touches, not an exact per-template set —
see module docs).

| Metric | Value |
|---|---|
| Cold compute (today's behavior, every occurrence) | 94.23ms total, 0.0616ms/glyph |
| Cache file size | 1,410,366 bytes (1377.3 KB) for 1530 glyphs |
| Cache load (deserialize) | 1.107ms |
| Cached lookup (1530 glyphs) | 2.421ms total, 0.0016ms/glyph |
| Total (load + lookup) vs cold compute | 3.528ms vs 94.23ms → **26.7x speedup** |
| Per-glyph cached-lookup-only vs cold | 0.0016ms vs 0.0616ms → **38.5x speedup** |
| Correctness | 1530/1530 exact `Path::eq` match against fresh computation |
| Overlay effectiveness | uncovered glyph: 1st occurrence = 1 miss, 2nd occurrence (same process) = 1 overlay hit, not recomputed |

These are real, reproducible numbers from real project assets — different
from the old unverified spike's numbers (different working set size and
composition), but confirm the same order-of-magnitude finding: caching
glyph outlines is a large (order-of-magnitude), correctness-preserving win.
Production numbers will differ again once measured against actual filled,
rendered documents per template (`GlyphCacheBuilder`, plan Phase 5) rather
than this crate's static-text-only working set — treat this as confirming
the mechanism and the ballpark, not as the final number to cite in the
design doc going forward (Phase 7 of the implementation plan is what
produces that).

## Design notes this spike validates against real crate versions

- `post_script_name: String` (not raw `fontdb::ID`) is confirmed present on
  `fontdb::FaceInfo` in fontdb 0.23.0 — safe, stable cache key.
- `tiny_skia_path::Path` (0.11.4) has **no serde support at all** — no
  `serde` feature, doesn't derive `Serialize`/`Deserialize`. The design
  doc's "trivially serializable... use serde + bincode/postcard"
  recommendation needs a manual DTO (`SerializablePath` in this crate) that
  round-trips through `Path::segments()`/`PathBuilder`'s public API, not a
  derive on `Path` itself. This is a correction to the design doc, not
  something it got wrong in spirit — the *data* is plain and owned, just
  not derive-serializable out of the box.
- `DatabaseExt::outline()` in `crates/usvg/src/text/flatten.rs` (confirmed
  against real `linebender/resvg` @ `v0.45.1` source) is called as
  `fontdb.outline(glyph.font, glyph.id)` from `flatten()`
  (`crates/usvg/src/text/flatten.rs:131-133`), which is called from
  `text::convert()` (`crates/usvg/src/text/mod.rs:214`), which is called
  from `crates/usvg/src/parser/text.rs:143` — where `state.opt` (a
  `&usvg::Options`) **is already in scope**. This confirms the real
  threading path for a new `Options` field: add a cache field to
  `Options`, pass `state.opt.glyph_cache.as_ref()` into `text::convert`,
  thread it through to `flatten()`, and have `flatten()` call a new
  cache-aware outline function instead of `fontdb.outline()` directly at
  line 131-133.
