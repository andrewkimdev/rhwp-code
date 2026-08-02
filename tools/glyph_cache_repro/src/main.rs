//! Committed reproduction of the glyph-outline disk-cache spike described in
//! `docs/RHWP_GLYPH_OUTLINE_CACHE_PLAN.md` (hwpx-template-engine repo).
//!
//! Unlike the throwaway scratchpad spike that produced the numbers currently
//! cited in that doc, this is a real, runnable, checked-in Cargo binary.
//! It exercises the exact same crates and versions `usvg` 0.45.1 depends on
//! (fontdb 0.23.0, ttf-parser 0.25.1, tiny-skia-path 0.11.4 — pinned in
//! Cargo.toml to match `crates/usvg`'s own `Cargo.lock` at the `v0.45.1` tag)
//! against the hwpx-template-engine project's real bundled fonts and real
//! template text, and measures:
//!
//!   1. Cold-compute cost: `ttf_parser::Face::parse` + `outline_glyph`,
//!      redone for every glyph occurrence (today's behavior).
//!   2. Serialize a built cache to disk, then measure load + lookup cost in
//!      a fresh process invocation (`--mode load`), simulating what a
//!      per-template glyph-outline cache would cost `rhwp` per request.
//!   3. Confirms cached and freshly-computed outlines are exactly equal
//!      (`tiny_skia_path::Path`'s own `PartialEq`), not approximately.
//!
//! ## Working-set caveat
//!
//! The 153-character working set (`assets/template_chars.txt`) is every
//! unique visible character in the two registered templates' *static*
//! section-XML text (`<hp:t>` runs in `scslic.hwpx`/`foobar.hwpx`), not
//! dynamic field values a real request would fill in — building a truly
//! representative working set requires rendering real filled documents
//! through `rhwp export-svg`, which is what `GlyphCacheBuilder` (the Java
//! tool, plan Phase 5) does in production. This spike exists to validate
//! the caching *mechanism* and get a real, reproducible order-of-magnitude
//! number — not to be the final production cache.
//!
//! Every (font, glyph) pair actually reachable by any of the project's 12
//! fonts is included (all fonts × all working-set characters that resolve
//! to a nonzero glyph index) rather than trying to guess which font each
//! template binds each run of text to — a superset of what any single
//! rendered document would touch, so the measured numbers are a
//! conservative (not favorably biased) estimate.
//!
//! Run: `cargo run --release -- build` then `cargo run --release -- load`

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Instant;

use dashmap::DashMap;
use fontdb::{Database, ID};
use serde::{Deserialize, Serialize};
use tiny_skia_path::{Path as SkPath, PathBuilder, PathSegment};
use ttf_parser::GlyphId;

const TEMPLATE_CHARS: &str = include_str!("../assets/template_chars.txt");
const CACHE_FILE: &str = "glyph_outline_cache_repro.bin";

/// On-disk cache entry key. Matches `RHWP_GLYPH_OUTLINE_CACHE_PLAN.md`'s
/// design: `fontdb::ID` is a `slotmap` generational key, reassigned by
/// insertion order into a `Database` built fresh every process — not a
/// stable cross-process contract. `post_script_name` is stable.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
struct CacheKey {
    post_script_name: String,
    glyph_id: u16,
}

/// Serializable stand-in for `tiny_skia_path::Path`, which has no `serde`
/// support at all in 0.11.4 (no `serde` feature, `Path` doesn't derive
/// (De)Serialize) — contrary to what a literal reading of the design doc's
/// "trivially serializable" claim might suggest. This DTO round-trips a
/// `Path` via its public `segments()`/`PathBuilder` API instead of reaching
/// into private fields.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct SerializablePath {
    /// 0=MoveTo(1pt) 1=LineTo(1pt) 2=QuadTo(2pt) 3=CubicTo(3pt) 4=Close(0pt)
    verb_tags: Vec<u8>,
    points: Vec<(f32, f32)>,
}

impl SerializablePath {
    fn from_path(path: &SkPath) -> Self {
        let mut verb_tags = Vec::new();
        let mut points = Vec::new();
        for seg in path.segments() {
            match seg {
                PathSegment::MoveTo(p) => {
                    verb_tags.push(0);
                    points.push((p.x, p.y));
                }
                PathSegment::LineTo(p) => {
                    verb_tags.push(1);
                    points.push((p.x, p.y));
                }
                PathSegment::QuadTo(p0, p1) => {
                    verb_tags.push(2);
                    points.push((p0.x, p0.y));
                    points.push((p1.x, p1.y));
                }
                PathSegment::CubicTo(p0, p1, p2) => {
                    verb_tags.push(3);
                    points.push((p0.x, p0.y));
                    points.push((p1.x, p1.y));
                    points.push((p2.x, p2.y));
                }
                PathSegment::Close => {
                    verb_tags.push(4);
                }
            }
        }
        SerializablePath { verb_tags, points }
    }

    fn to_path(&self) -> Option<SkPath> {
        let mut builder = PathBuilder::new();
        let mut pts = self.points.iter();
        for &tag in &self.verb_tags {
            match tag {
                0 => {
                    let (x, y) = *pts.next()?;
                    builder.move_to(x, y);
                }
                1 => {
                    let (x, y) = *pts.next()?;
                    builder.line_to(x, y);
                }
                2 => {
                    let (x1, y1) = *pts.next()?;
                    let (x, y) = *pts.next()?;
                    builder.quad_to(x1, y1, x, y);
                }
                3 => {
                    let (x1, y1) = *pts.next()?;
                    let (x2, y2) = *pts.next()?;
                    let (x, y) = *pts.next()?;
                    builder.cubic_to(x1, y1, x2, y2, x, y);
                }
                4 => builder.close(),
                _ => return None,
            }
        }
        builder.finish()
    }
}

/// Mirrors `crate::text::flatten::DatabaseExt::outline()` in the real
/// `usvg` fork target (`crates/usvg/src/text/flatten.rs:204-217`,
/// `linebender/resvg` @ `v0.45.1`) exactly, so the "cold compute" cost
/// measured here is the real per-occurrence cost the patch removes.
fn compute_outline(db: &Database, id: ID, glyph_id: GlyphId) -> Option<SkPath> {
    db.with_face_data(id, |data, face_index| -> Option<SkPath> {
        let font = ttf_parser::Face::parse(data, face_index).ok()?;
        let mut builder = OutlineCollector(PathBuilder::new());
        font.outline_glyph(glyph_id, &mut builder)?;
        builder.0.finish()
    })?
}

struct OutlineCollector(PathBuilder);

impl ttf_parser::OutlineBuilder for OutlineCollector {
    fn move_to(&mut self, x: f32, y: f32) {
        self.0.move_to(x, y);
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.0.line_to(x, y);
    }
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        self.0.quad_to(x1, y1, x, y);
    }
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        self.0.cubic_to(x1, y1, x2, y2, x, y);
    }
    fn close(&mut self) {
        self.0.close();
    }
}

/// Two-layer runtime structure from the design doc: a frozen, disk-loaded
/// base (no lock) plus a per-process `DashMap` overlay for anything the
/// base missed but repeats within one run. `Arc`-free here since the spike
/// is single-threaded; the real patch wraps the base in `Arc` for
/// cross-thread sharing under the page-parallelization patch.
struct TwoLayerCache {
    base: HashMap<CacheKey, Option<SerializablePath>>,
    overlay: DashMap<CacheKey, Option<SkPath>>,
    hits_base: std::sync::atomic::AtomicU64,
    hits_overlay: std::sync::atomic::AtomicU64,
    misses: std::sync::atomic::AtomicU64,
}

impl TwoLayerCache {
    fn load(base: HashMap<CacheKey, Option<SerializablePath>>) -> Self {
        TwoLayerCache {
            base,
            overlay: DashMap::new(),
            hits_base: 0.into(),
            hits_overlay: 0.into(),
            misses: 0.into(),
        }
    }

    fn outline(&self, db: &Database, id: ID, glyph_id: GlyphId, post_script_name: &str) -> Option<SkPath> {
        let key = CacheKey {
            post_script_name: post_script_name.to_string(),
            glyph_id: glyph_id.0,
        };
        if let Some(entry) = self.base.get(&key) {
            self.hits_base.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            return entry.as_ref().and_then(|p| p.to_path());
        }
        if let Some(entry) = self.overlay.get(&key) {
            self.hits_overlay.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            return entry.clone();
        }
        self.misses.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let computed = compute_outline(db, id, glyph_id);
        self.overlay.insert(key, computed.clone());
        computed
    }
}

fn project_fonts_dir() -> PathBuf {
    // Real, checked-in project fonts — not a synthetic/system font set, per
    // the design doc's cross-platform-key validation requirement (every
    // `post_script_name` in a shipped cache must trace back to a font under
    // `src/main/resources/fonts`, never `load_system_fonts()`). This crate
    // lives in the `rhwp-code` fork, a separate repository from
    // `hwpx-template-engine`, so there is no relative path that reaches the
    // fonts directory from here — it must always be passed explicitly.
    match std::env::var("HWPX_FONTS_DIR") {
        Ok(dir) => PathBuf::from(dir),
        Err(_) => {
            eprintln!("HWPX_FONTS_DIR env var not set.");
            eprintln!("Point it at hwpx-template-engine's src/main/resources/fonts, e.g.:");
            eprintln!("  HWPX_FONTS_DIR=/path/to/hwpx-template-engine/src/main/resources/fonts cargo run --release -- build");
            std::process::exit(2);
        }
    }
}

fn load_fontdb() -> Database {
    let mut db = Database::new();
    let dir = project_fonts_dir();
    let canon = dir.canonicalize().unwrap_or_else(|e| {
        panic!(
            "project fonts dir not found at {:?} ({e}); set HWPX_FONTS_DIR to override",
            dir
        )
    });
    db.load_fonts_dir(&canon);
    eprintln!("loaded {} faces from {:?}", db.len(), canon);
    db
}

/// Every (font, glyph) pair any of the project's fonts can render for the
/// real template working set. See module docs for why this is a
/// (conservative) superset rather than a per-template-exact set.
fn build_working_set(db: &Database) -> Vec<(ID, u16, String)> {
    let chars: Vec<char> = TEMPLATE_CHARS.chars().filter(|c| !c.is_whitespace()).collect();
    let mut pairs = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for face in db.faces() {
        let id = face.id;
        let post_script_name = face.post_script_name.clone();
        db.with_face_data(id, |data, face_index| {
            let Ok(font) = ttf_parser::Face::parse(data, face_index) else {
                return;
            };
            for &ch in &chars {
                if let Some(gid) = font.glyph_index(ch) {
                    if gid.0 == 0 {
                        continue; // .notdef, font has no glyph for this char
                    }
                    let key = (post_script_name.clone(), gid.0);
                    if seen.insert(key) {
                        pairs.push((id, gid.0, post_script_name.clone()));
                    }
                }
            }
        });
    }
    pairs
}

fn mode_build() {
    let db = load_fontdb();
    let working_set = build_working_set(&db);
    println!("working set: {} (font, glyph) pairs from {} template chars",
        working_set.len(), TEMPLATE_CHARS.chars().filter(|c| !c.is_whitespace()).count());

    // --- Cold compute, timed (today's behavior: recomputed every occurrence) ---
    let start = Instant::now();
    let mut computed: HashMap<CacheKey, Option<SkPath>> = HashMap::new();
    for &(id, gid, ref psn) in &working_set {
        let key = CacheKey { post_script_name: psn.clone(), glyph_id: gid };
        let outline = compute_outline(&db, id, GlyphId(gid));
        computed.insert(key, outline);
    }
    let cold_elapsed = start.elapsed();

    // --- Serialize to disk ---
    let serializable: HashMap<CacheKey, Option<SerializablePath>> = computed
        .iter()
        .map(|(k, v)| (k.clone(), v.as_ref().map(SerializablePath::from_path)))
        .collect();
    let bytes = bincode::serialize(&serializable).expect("serialize cache");
    std::fs::write(CACHE_FILE, &bytes).expect("write cache file");

    println!("cold compute ({} glyphs): {:?} ({:.4}ms/glyph)",
        working_set.len(), cold_elapsed, cold_elapsed.as_secs_f64() * 1000.0 / working_set.len() as f64);
    println!("cache file: {} bytes ({:.1} KB) at {}", bytes.len(), bytes.len() as f64 / 1024.0, CACHE_FILE);
}

fn mode_load() {
    let db = load_fontdb();
    let working_set = build_working_set(&db);

    // --- Load + deserialize from disk, timed ---
    let start = Instant::now();
    let bytes = std::fs::read(CACHE_FILE).unwrap_or_else(|e| {
        panic!("cache file {CACHE_FILE} not found ({e}) - run `cargo run --release -- build` first")
    });
    let base: HashMap<CacheKey, Option<SerializablePath>> =
        bincode::deserialize(&bytes).expect("deserialize cache");
    let cache = TwoLayerCache::load(base);
    let load_elapsed = start.elapsed();

    // --- Lookup every glyph in the working set via the cache, timed ---
    let start = Instant::now();
    let mut results: HashMap<CacheKey, Option<SkPath>> = HashMap::new();
    for &(id, gid, ref psn) in &working_set {
        let outline = cache.outline(&db, id, GlyphId(gid), psn);
        results.insert(CacheKey { post_script_name: psn.clone(), glyph_id: gid }, outline);
    }
    let lookup_elapsed = start.elapsed();

    println!("cache load (deserialize {} bytes): {:?}", bytes.len(), load_elapsed);
    println!("cached lookup ({} glyphs): {:?} ({:.4}ms/glyph)",
        working_set.len(), lookup_elapsed, lookup_elapsed.as_secs_f64() * 1000.0 / working_set.len() as f64);
    println!("total (load+lookup): {:?}", load_elapsed + lookup_elapsed);
    println!("base hits: {}, overlay hits: {}, misses: {}",
        cache.hits_base.load(std::sync::atomic::Ordering::Relaxed),
        cache.hits_overlay.load(std::sync::atomic::Ordering::Relaxed),
        cache.misses.load(std::sync::atomic::Ordering::Relaxed));

    // --- Correctness: cached output must equal freshly computed output exactly ---
    let mut mismatches = 0usize;
    for &(id, gid, ref psn) in &working_set {
        let key = CacheKey { post_script_name: psn.clone(), glyph_id: gid };
        let cached = results.get(&key).unwrap();
        let fresh = compute_outline(&db, id, GlyphId(gid));
        if cached != &fresh {
            mismatches += 1;
            eprintln!("MISMATCH: {:?}", key);
        }
    }
    if mismatches == 0 {
        println!("correctness: {} / {} glyphs exactly equal to freshly-computed output (Path::eq)",
            working_set.len(), working_set.len());
    } else {
        println!("correctness: {mismatches} MISMATCHES out of {} glyphs", working_set.len());
        std::process::exit(1);
    }

    // --- Overlay-effectiveness check: an uncovered glyph, looked up twice ---
    // Simulates a character not in the shipped cache repeating across
    // several repeat-block rows: first occurrence must miss (compute from
    // scratch), the second occurrence within the same process must hit the
    // overlay, not recompute.
    if let Some(&(id, gid, ref psn)) = working_set.first() {
        let fake_psn = format!("{psn}__not-in-base-cache");
        let before_misses = cache.misses.load(std::sync::atomic::Ordering::Relaxed);
        let before_overlay = cache.hits_overlay.load(std::sync::atomic::Ordering::Relaxed);
        let _ = cache.outline(&db, id, GlyphId(gid), &fake_psn);
        let _ = cache.outline(&db, id, GlyphId(gid), &fake_psn);
        let after_misses = cache.misses.load(std::sync::atomic::Ordering::Relaxed);
        let after_overlay = cache.hits_overlay.load(std::sync::atomic::Ordering::Relaxed);
        assert_eq!(after_misses - before_misses, 1, "first occurrence of an uncovered glyph must be exactly one full miss");
        assert_eq!(after_overlay - before_overlay, 1, "second occurrence of the same uncovered glyph must hit the overlay, not recompute");
        println!("overlay effectiveness: uncovered glyph computed once, hit overlay on repeat - OK");
    }
}

fn main() {
    let mode = std::env::args().nth(1).unwrap_or_default();
    match mode.as_str() {
        "build" => mode_build(),
        "load" => mode_load(),
        _ => {
            eprintln!("usage: glyph_cache_repro <build|load>");
            std::process::exit(2);
        }
    }
}
