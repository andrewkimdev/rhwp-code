//! On-disk file format for a [`usvg::GlyphOutlineCache`], plus the
//! font-directory staleness check that wraps it.
//!
//! `usvg::GlyphOutlineCache::to_bytes()`/`from_bytes()` handle only the
//! cache entries themselves (see `crates/usvg/src/text/glyph_cache.rs` in
//! the patched `usvg` fork) — deliberately, so that crate's format stays a
//! plain, reusable building block. Staleness detection is this crate's own
//! responsibility, per
//! `docs/RHWP_GLYPH_OUTLINE_CACHE_PLAN.md` (hwpx-template-engine repo):
//!
//! > Hash the font directory's contents ... into the cache file's header at
//! > build time; at load time, recompute the same hash and compare. On
//! > mismatch: log a warning and fall back to computing everything fresh.
//!
//! Only `--font-path` directories are hashed, not system fonts
//! (`fontdb.load_system_fonts()`) or the hardcoded `ttfs/` search paths —
//! a shipped cache is only ever expected to cover a project's own bundled
//! fonts (validated separately: every `post_script_name` in a shipped
//! cache should trace back to a `--font-path` font, never a system one).
//! A change to a font this hash doesn't cover just means those glyphs stay
//! uncovered (falls through to the per-process overlay, still correct,
//! just uncached) -- not a correctness gap, only a coverage one.
//!
//! # Metadata hash, not content hash (v2)
//!
//! v1 hashed every font file's full *contents* on every cache-enabled
//! invocation. Measured against this project's real font set
//! (`src/main/resources/fonts`, ~159MB — a few CJK-coverage TTFs are
//! 20-29MB each), that cost 70-150ms per `rhwp` process, per real
//! end-to-end benchmarking (`bench/measure-glyph-cache-delta.sh`) —
//! enough to make caching a net *loss* for small/medium documents, since
//! this hash is recomputed fresh every process (no daemon, no
//! cross-invocation memory). v2 hashes each file's path + size + mtime
//! instead — near-instant (`stat`, not `read`), at the cost of not
//! detecting a font file rewritten with identical size and mtime (an
//! extremely narrow window: `touch -r` or an unusually fast same-second
//! edit). That residual risk is accepted here as strictly better than the
//! v1 tradeoff: correctness is unaffected either way (a missed staleness
//! detection still only produces a *cache-hit for glyphs that happen to
//! still be correct*, and any genuinely different font content that also
//! happens to share path+size+mtime with the old one is vanishingly
//! unlikely outside deliberately adversarial conditions this CLI tool
//! doesn't need to defend against).

use std::path::Path;

use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};

const MAGIC: &[u8; 4] = b"RHGC"; // RHwp Glyph Cache
const FORMAT_VERSION: u32 = 2;

/// Hashes the path + size + mtime of every file under each of `font_paths`
/// (recursively, sorted by path for determinism), plus the format version,
/// into a single 32-byte digest. Deliberately metadata-only, not file
/// contents — see the "Metadata hash, not content hash (v2)" module docs.
pub fn hash_font_paths(font_paths: &[std::path::PathBuf]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(&FORMAT_VERSION.to_le_bytes());

    let mut files: Vec<std::path::PathBuf> = Vec::new();
    for dir in font_paths {
        collect_files_sorted(dir, &mut files);
    }
    files.sort();

    for file in files {
        let Ok(metadata) = std::fs::metadata(&file) else {
            continue;
        };
        hasher.update(file.to_string_lossy().as_bytes());
        hasher.update(&metadata.len().to_le_bytes());
        if let Ok(modified) = metadata.modified() {
            if let Ok(since_epoch) = modified.duration_since(std::time::UNIX_EPOCH) {
                hasher.update(&since_epoch.as_nanos().to_le_bytes());
            }
        }
    }

    *hasher.finalize().as_bytes()
}

fn collect_files_sorted(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_files_sorted(&path, out);
        } else {
            out.push(path);
        }
    }
}

/// Writes `cache` to `path`, wrapped with a header containing the format
/// version and a hash of `font_paths`' contents.
pub fn write(
    path: &Path,
    cache: &usvg::GlyphOutlineCache,
    font_paths: &[std::path::PathBuf],
) -> Result<(), String> {
    let hash = hash_font_paths(font_paths);
    let payload = cache.to_bytes();

    let mut out = Vec::with_capacity(4 + 4 + 32 + payload.len());
    out.extend_from_slice(MAGIC);
    out.write_u32::<LittleEndian>(FORMAT_VERSION)
        .map_err(|e| e.to_string())?;
    out.extend_from_slice(&hash);
    out.extend_from_slice(&payload);

    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
    }
    std::fs::write(path, &out).map_err(|e| e.to_string())
}

/// Loads a cache from `path`, validating the header against a fresh hash of
/// `font_paths`. Every failure mode (missing file, corrupt header, version
/// mismatch, stale font hash, corrupt payload) returns `None` with a
/// stderr warning -- never an error the caller has to propagate. This is
/// the "warn and fall back to computing everything fresh" behavior the
/// design doc requires: an out-of-date or absent cache is a performance
/// regression to baseline, never a correctness risk or a hard failure.
pub fn load(path: &Path, font_paths: &[std::path::PathBuf]) -> Option<usvg::GlyphOutlineCache> {
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(_) => return None, // no cache file: silently skip, exactly today's behavior
    };

    if bytes.len() < 4 + 4 + 32 {
        eprintln!(
            "WARN: glyph cache '{}' is truncated -- ignoring, computing glyphs fresh.",
            path.display()
        );
        return None;
    }
    if &bytes[0..4] != MAGIC {
        eprintln!(
            "WARN: glyph cache '{}' has an unrecognized header -- ignoring, computing glyphs fresh.",
            path.display()
        );
        return None;
    }

    let mut version_bytes = &bytes[4..8];
    let version = match version_bytes.read_u32::<LittleEndian>() {
        Ok(v) => v,
        Err(_) => {
            eprintln!(
                "WARN: glyph cache '{}' header is corrupt -- ignoring, computing glyphs fresh.",
                path.display()
            );
            return None;
        }
    };
    if version != FORMAT_VERSION {
        eprintln!(
            "WARN: glyph cache '{}' was built with format version {} (expected {}) -- ignoring, computing glyphs fresh.",
            path.display(),
            version,
            FORMAT_VERSION
        );
        return None;
    }

    let stored_hash = &bytes[8..40];
    let current_hash = hash_font_paths(font_paths);
    if stored_hash != current_hash {
        eprintln!(
            "WARN: glyph cache '{}' was built against a different font set (font files under --font-path changed since it was built) -- ignoring, computing glyphs fresh.",
            path.display()
        );
        return None;
    }

    let payload = &bytes[40..];
    match usvg::GlyphOutlineCache::from_bytes(payload) {
        Ok(cache) => Some(cache),
        Err(e) => {
            eprintln!(
                "WARN: glyph cache '{}' payload is corrupt ({e}) -- ignoring, computing glyphs fresh.",
                path.display()
            );
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_font_dir() -> tempfile_dir::TempDir {
        tempfile_dir::TempDir::new()
    }

    // Minimal throwaway temp-dir helper -- avoids adding a `tempfile` dev-dependency
    // for a handful of tests.
    mod tempfile_dir {
        use std::path::PathBuf;

        pub struct TempDir(pub PathBuf);
        impl TempDir {
            pub fn new() -> Self {
                let dir = std::env::temp_dir().join(format!(
                    "rhwp-glyph-cache-file-test-{}-{}",
                    std::process::id(),
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_nanos()
                ));
                std::fs::create_dir_all(&dir).unwrap();
                TempDir(dir)
            }
            pub fn path(&self) -> &std::path::Path {
                &self.0
            }
        }
        impl Drop for TempDir {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
    }

    /// `GlyphOutlineCache::lookup_or_compute` is `pub(crate)` inside `usvg`
    /// -- rhwp can only populate a cache the same way a real render does:
    /// through `usvg::Tree::from_str` with the cache attached via
    /// `Options::glyph_outline_cache`. Uses the real bundled test font
    /// (`ttfs/opensource/NotoSansKR-Regular.ttf`), same as
    /// `create_fontdb`'s own hardcoded `ttfs` search path.
    #[test]
    fn round_trip_preserves_entries() {
        let font_dir =
            std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/ttfs/opensource"));
        let font_paths = vec![font_dir.clone()];

        let mut fontdb = usvg::fontdb::Database::new();
        fontdb.load_fonts_dir(&font_dir);

        let build_cache = std::sync::Arc::new(usvg::GlyphOutlineCache::empty());
        let build_opt = usvg::Options {
            fontdb: std::sync::Arc::new(fontdb),
            glyph_outline_cache: Some(build_cache.clone()),
            ..Default::default()
        };
        let svg = r#"<svg viewBox="0 0 200 200" xmlns="http://www.w3.org/2000/svg">
            <text x="10" y="50" font-family="Noto Sans KR" font-size="32">Hello</text>
        </svg>"#;
        usvg::Tree::from_str(svg, &build_opt).expect("render must succeed");
        assert!(
            build_cache.len() > 0,
            "rendering real text must populate the cache"
        );

        let cache_dir = temp_font_dir();
        let cache_path = cache_dir.path().join("cache.bin");
        write(&cache_path, &build_cache, &font_paths).unwrap();

        let loaded = load(&cache_path, &font_paths).expect("should load successfully");
        assert_eq!(loaded.len(), build_cache.len());
    }

    #[test]
    fn missing_file_returns_none_silently() {
        assert!(load(Path::new("/nonexistent/path/cache.bin"), &[]).is_none());
    }

    #[test]
    fn font_change_invalidates_cache() {
        let font_dir = temp_font_dir();
        let font_file = font_dir.path().join("a.ttf");
        std::fs::write(&font_file, b"version 1").unwrap();
        let font_paths = vec![font_dir.path().to_path_buf()];

        let cache = usvg::GlyphOutlineCache::empty();
        let cache_dir = temp_font_dir();
        let cache_path = cache_dir.path().join("cache.bin");
        write(&cache_path, &cache, &font_paths).unwrap();

        assert!(load(&cache_path, &font_paths).is_some());

        std::fs::write(&font_file, b"version 2 - different content").unwrap();
        assert!(
            load(&cache_path, &font_paths).is_none(),
            "a changed font file must invalidate the cache"
        );
    }

    #[test]
    fn corrupt_payload_falls_back_cleanly() {
        let cache_dir = temp_font_dir();
        let cache_path = cache_dir.path().join("cache.bin");
        std::fs::write(&cache_path, b"not a valid cache file at all").unwrap();
        assert!(load(&cache_path, &[]).is_none());
    }
}
