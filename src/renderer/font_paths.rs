//! [#2864] 폰트 조달 경로를 한 곳에서 결정한다.
//!
//! 종전에는 `renderer/pdf.rs`·`renderer/svg.rs`·`renderer/skia/image_conv.rs`·
//! `renderer/skia/renderer.rs` 네 곳이 각자 탐색 경로를 하드코딩했고, 그중
//! `ttfs/hwp`·`ttfs/windows`(로컬 전용 저작권 폰트, `.gitignore`)와
//! `/mnt/c/Windows/Fonts`(WSL2 개발 PC 전용)는 **특정 환경에만 존재**했다.
//!
//! 서버·컨테이너에서 CLI 로 대량 변환할 때 이 경로들은 아무 값도 주지 못하면서
//! 경로 검사만 발생시키고, WSL2 에서는 `/mnt/c` 가 9p 파일시스템이라 간헐적
//! 극단 지연으로 `export-pdf` 가 통째로 멎었다(#2268, 누적 4회 관측).
//!
//! 폰트는 호출자가 `--font-path`(`options.font_paths`) 또는 `RHWP_FONT_PATH`
//! 로 지정하거나 시스템에 설치해 쓴다. 이 모듈은 그 순서만 정의한다.
//!
//! # 조달 순서
//!
//! 1. **호출자 지정** — `--font-path` / `options.font_paths` (최우선)
//! 2. **`RHWP_FONT_PATH`** — 환경변수. 백엔드 대량 변환에서 호출마다 인자를
//!    붙이는 대신 한 번만 설정한다. 복수 경로는 OS 관례 구분자로 나눈다
//!    (유닉스 `:`, Windows `;`).
//! 3. **시스템 기본** — OS별 표준 폰트 디렉터리
//! 4. **`ttfs/opensource`** — 최후 폴백. `.gitignore` 대상이 아닌 **저장소 자산**
//!    (NotoSansKR 2종 + OFL)이라 모든 체크아웃에서 확보된다. 폰트 미설치 환경
//!    (CI headless·컨테이너)에서 한국어가 드롭되는 것을 막는다(#2293).
//!
//! # 패키지 폰트 전용 모드 (`--packaged-fonts-only`, opt-in)
//!
//! 서버 대량 변환에서 산출물이 호스트에 설치된 폰트에 따라 달라지지 않도록,
//! 호출자가 `--font-path` 로 넘긴 디렉터리**만** 폰트 출처로 쓰는 모드다.
//! 켜면 2~4 단계(환경변수·시스템·번들 `ttfs/opensource`)와
//! `load_system_fonts()`(시스템 폰트 DB)를 모두 건너뛴다. rhwp CLI 는 렌더
//! 1회당 프로세스 1개라 CLI 파싱 시점에 한 번 켜는 프로세스 전역 설정으로 둔다
//! ([`enable_packaged_fonts_only`]). 기본값은 꺼짐이며 WASM 빌드에는 이 모듈이
//! 없다.

use std::path::{Path, PathBuf};

/// 폰트 탐색 경로 환경변수. 복수 경로는 OS 관례 구분자로 나눈다.
pub const FONT_PATH_ENV: &str = "RHWP_FONT_PATH";

/// 저장소 번들 오픈소스 폰트 — 최후 폴백(#2293 한국어 드롭 방지).
pub const BUNDLED_OPENSOURCE_DIR: &str = "ttfs/opensource";

/// 패키지 폰트 전용 모드 설정 — 호출자 지정 폰트 디렉터리와 generic 폴백 family.
///
/// generic 폴백(`serif`/`sans-serif`/`monospace`)은 문서 글꼴이 패키지에 없을 때
/// 떨어질 자리다. 이 모드에서는 시스템·번들 폰트가 없으므로 반드시 패키지 안의
/// family 를 가리켜야 텍스트가 사라지지 않는다(`--fallback-serif/-sans/-mono`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PackagedFontsOnly {
    /// `--font-path` 로 받은 디렉터리(호출 순서 보존).
    pub dirs: Vec<PathBuf>,
    pub fallback_serif: Option<String>,
    pub fallback_sans: Option<String>,
    pub fallback_mono: Option<String>,
}

static PACKAGED_FONTS_ONLY: std::sync::OnceLock<PackagedFontsOnly> = std::sync::OnceLock::new();

/// 패키지 폰트 전용 모드를 켠다. 프로세스당 한 번만 유효하다(CLI 파싱 시점).
/// 이미 켜져 있으면 `false` 를 돌려주고 기존 설정을 유지한다.
pub fn enable_packaged_fonts_only(config: PackagedFontsOnly) -> bool {
    PACKAGED_FONTS_ONLY.set(config).is_ok()
}

/// 패키지 폰트 전용 모드 설정. 꺼져 있으면(기본) `None`.
pub fn packaged_fonts_only() -> Option<&'static PackagedFontsOnly> {
    PACKAGED_FONTS_ONLY.get()
}

/// 패키지 폰트 전용 모드가 켜져 있는가.
pub fn is_packaged_fonts_only() -> bool {
    PACKAGED_FONTS_ONLY.get().is_some()
}

/// 패키지 폰트 전용 모드에서 호출자 디렉터리를 따로 받지 못하는 경로
/// (`skia::image_conv` 의 프로세스 전역 fontdb 등)용 디렉터리. 꺼져 있으면 빈 목록.
pub fn packaged_font_dirs() -> &'static [PathBuf] {
    PACKAGED_FONTS_ONLY
        .get()
        .map(|c| c.dirs.as_slice())
        .unwrap_or(&[])
}

/// `RHWP_FONT_PATH` 를 읽어 경로 목록으로 나눈다.
///
/// 미설정이면 빈 벡터를 돌려준다. 존재하지 않는 경로도 그대로 담아 호출부가
/// 기존 `--font-path` 와 동일하게 경고·건너뛰기를 하도록 맡긴다.
pub fn env_font_paths() -> Vec<PathBuf> {
    let Ok(raw) = std::env::var(FONT_PATH_ENV) else {
        return Vec::new();
    };
    // `split` 은 빈 조각도 내므로(예: 연속 구분자, 양끝 구분자) 걸러낸다.
    raw.split(PATH_SEPARATOR)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .collect()
}

#[cfg(target_os = "windows")]
const PATH_SEPARATOR: char = ';';

#[cfg(not(target_os = "windows"))]
const PATH_SEPARATOR: char = ':';

/// OS별 시스템 폰트 디렉터리.
///
/// `usvg::fontdb::Database::load_system_fonts()` 를 쓰는 경로에서는 필요 없고,
/// 파일을 직접 훑는 경로(`svg.rs::find_font_file`, `skia` typeface 로딩)에서 쓴다.
pub fn system_font_dirs() -> Vec<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        vec![
            PathBuf::from("/Library/Fonts"),
            PathBuf::from("/System/Library/Fonts"),
            PathBuf::from("/System/Library/Fonts/Supplemental"),
        ]
    }
    #[cfg(target_os = "linux")]
    {
        vec![
            PathBuf::from("/usr/share/fonts"),
            PathBuf::from("/usr/local/share/fonts"),
        ]
    }
    #[cfg(target_os = "windows")]
    {
        vec![PathBuf::from("C:\\Windows\\Fonts")]
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        Vec::new()
    }
}

/// 파일을 직접 훑는 경로용 탐색 디렉터리 전체를 조달 순서대로 돌려준다.
///
/// `extra` 는 호출자 지정 경로(`--font-path` 등)이며 최우선이다.
pub fn search_dirs(extra: &[PathBuf]) -> Vec<PathBuf> {
    search_dirs_with_mode(extra, is_packaged_fonts_only())
}

/// [`search_dirs`] 본체. `packaged_only` 이면 호출자 지정 경로만 돌려준다.
fn search_dirs_with_mode(extra: &[PathBuf], packaged_only: bool) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = extra.to_vec();
    if packaged_only {
        return dirs;
    }
    dirs.extend(env_font_paths());
    dirs.extend(system_font_dirs());
    // 최후 폴백 — 저장소 자산이라 항상 존재한다(#2293).
    dirs.push(PathBuf::from(BUNDLED_OPENSOURCE_DIR));
    dirs
}

/// custom typeface 로더(skia `with_font_paths`)용 디렉터리 — **시스템·번들 제외**.
///
/// skia 의 custom 로더는 family 당 typeface 1개만 담아 굵기(스타일) 변형을
/// 표현하지 못하고, 해석 체인에서 시스템 매칭보다 앞선다(#2864 이전 필드
/// 검증 의미론). 그래서 여기에는 사용자가 의도한 폰트(호출자 지정 → 환경
/// 변수)만 담는다:
/// - 시스템 디렉터리를 넣으면 regular 파일이 family 를 선점해 bold run 이
///   전부 regular 로 렌더된다(#3300) — 시스템 폰트는
///   `FontMgr::match_family_style`(정상 스타일 매칭)이 담당한다.
/// - 번들 opensource 를 넣으면 깊은 폴백(Noto Sans KR)이 시스템 1순위를
///   제치고 선점된다(#3300, r23 발산 −6.9pp) — 번들은 별도 최후-폴백 로더
///   (`bundled_font_dirs`)로 체인 말미에만 선다.
pub fn custom_font_dirs(extra: &[PathBuf]) -> Vec<PathBuf> {
    custom_font_dirs_with_mode(extra, is_packaged_fonts_only())
}

fn custom_font_dirs_with_mode(extra: &[PathBuf], packaged_only: bool) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = extra.to_vec();
    if !packaged_only {
        dirs.extend(env_font_paths());
    }
    dirs
}

/// 번들 최후-폴백 로더용 디렉터리 — 폰트 미설치 환경(CI headless·컨테이너)
/// 에서 한국어 드롭을 막는 저장소 자산(#2293). 해석 체인에서는 custom·시스템
/// 뒤(최후)에만 선다(#3300).
///
/// 패키지 폰트 전용 모드에서는 빈 목록 — 번들 Noto Sans KR 을 등록하지 않는다.
pub fn bundled_font_dirs() -> Vec<PathBuf> {
    bundled_font_dirs_with_mode(is_packaged_fonts_only())
}

fn bundled_font_dirs_with_mode(packaged_only: bool) -> Vec<PathBuf> {
    if packaged_only {
        return Vec::new();
    }
    vec![PathBuf::from(BUNDLED_OPENSOURCE_DIR)]
}

/// `fontdb` 에 조달 순서대로 폰트를 적재한다.
///
/// 시스템 폰트는 `load_system_fonts()` 가 담당하므로 여기서는 호출자 지정 →
/// 환경변수 → 번들 순으로 디렉터리만 추가한다. 존재하지 않는 경로는 건너뛰되,
/// `extra`(사용자가 명시한 경로)만 경고한다 — 환경변수·번들은 없을 수 있다.
///
/// 패키지 폰트 전용 모드에서는 `extra` 만 적재한다(환경변수·번들 제외).
/// 시스템 폰트 생략은 호출부(`load_system_fonts_unless_packaged_only`)가 맡는다.
pub fn load_into_fontdb(fontdb: &mut usvg::fontdb::Database, extra: &[PathBuf]) {
    load_into_fontdb_with_mode(fontdb, extra, is_packaged_fonts_only());
}

fn load_into_fontdb_with_mode(
    fontdb: &mut usvg::fontdb::Database,
    extra: &[PathBuf],
    packaged_only: bool,
) {
    for dir in extra {
        if dir.exists() {
            fontdb.load_fonts_dir(dir);
        } else {
            eprintln!(
                "WARN: font path '{}' not found. 해당 경로의 폰트는 로드하지 않습니다.",
                dir.display()
            );
        }
    }
    if packaged_only {
        return;
    }
    for dir in env_font_paths() {
        if dir.exists() {
            fontdb.load_fonts_dir(&dir);
        } else {
            eprintln!(
                "WARN: {FONT_PATH_ENV} entry '{}' not found. 해당 경로의 폰트는 로드하지 않습니다.",
                dir.display()
            );
        }
    }
    let bundled = Path::new(BUNDLED_OPENSOURCE_DIR);
    if bundled.exists() {
        fontdb.load_fonts_dir(bundled);
    }
}

/// `fontdb.load_system_fonts()` — 패키지 폰트 전용 모드에서는 건너뛴다.
pub fn load_system_fonts_unless_packaged_only(fontdb: &mut usvg::fontdb::Database) {
    if !is_packaged_fonts_only() {
        fontdb.load_system_fonts();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// 환경변수는 프로세스 전역이라 테스트가 병렬로 돌면 서로를 덮는다.
    /// `RHWP_FONT_PATH` 를 만지는 테스트는 이 락으로 직렬화한다 — 락 없이 셋이
    /// 병렬로 돌면 set 과 read 사이에 다른 테스트의 remove_var 가 끼어들어
    /// 간헐 실패한다(실측 플레이크).
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn env_font_paths_parses_and_filters() {
        let _g = ENV_LOCK.lock().unwrap();
        // 미설정
        std::env::remove_var(FONT_PATH_ENV);
        assert!(env_font_paths().is_empty(), "미설정이면 빈 목록이어야 한다");

        // 단일 경로
        std::env::set_var(FONT_PATH_ENV, "/tmp/fonts-a");
        assert_eq!(env_font_paths(), vec![PathBuf::from("/tmp/fonts-a")]);

        // 복수 경로 + 빈 조각(연속·양끝 구분자) 제거
        let joined = format!(
            "{sep}/tmp/fonts-a{sep}{sep}/tmp/fonts-b{sep}",
            sep = PATH_SEPARATOR
        );
        std::env::set_var(FONT_PATH_ENV, joined);
        assert_eq!(
            env_font_paths(),
            vec![PathBuf::from("/tmp/fonts-a"), PathBuf::from("/tmp/fonts-b")],
            "빈 조각은 걸러내고 순서는 보존해야 한다"
        );

        std::env::remove_var(FONT_PATH_ENV);
    }

    #[test]
    fn search_dirs_orders_caller_first_and_bundled_last() {
        let _g = ENV_LOCK.lock().unwrap();
        std::env::remove_var(FONT_PATH_ENV);
        let extra = vec![PathBuf::from("/tmp/caller")];
        let dirs = search_dirs(&extra);

        assert_eq!(
            dirs.first(),
            Some(&PathBuf::from("/tmp/caller")),
            "호출자 지정 경로가 최우선이어야 한다"
        );
        assert_eq!(
            dirs.last(),
            Some(&PathBuf::from(BUNDLED_OPENSOURCE_DIR)),
            "번들 오픈소스 폰트가 최후 폴백이어야 한다"
        );
    }

    /// [#2864] 환경 종속 경로가 조달 목록에 다시 들어오지 않도록 고정한다.
    /// 이 가드가 없으면 편의를 위해 재추가되어 #2268 간헐 행이 재발할 수 있다.
    #[test]
    fn search_dirs_excludes_environment_specific_paths() {
        let _g = ENV_LOCK.lock().unwrap();
        std::env::remove_var(FONT_PATH_ENV);
        let dirs = search_dirs(&[]);
        let joined: Vec<String> = dirs.iter().map(|p| p.display().to_string()).collect();

        for banned in [
            "/mnt/c/Windows/Fonts", // WSL2 개발 PC 전용 — #2268 간헐 행 원인
            "ttfs/hwp",             // .gitignore 로컬 전용(저작권 폰트)
            "ttfs/windows",
        ] {
            assert!(
                !joined.iter().any(|d| d == banned),
                "환경 종속 경로가 조달 목록에 있다: {banned}\n\
                 폰트가 필요하면 --font-path 또는 {FONT_PATH_ENV} 로 지정한다."
            );
        }
    }

    /// [#3300] custom 로더 목록은 시스템·번들을 포함하면 안 된다.
    /// - 시스템: family 당 1 typeface 라 굵기 변형 소실(regular 선점)
    /// - 번들: 깊은 폴백(Noto)이 시스템 1순위를 제치고 본문을 렌더(r23 −6.9pp)
    #[test]
    fn custom_font_dirs_excludes_system_and_bundled() {
        let _g = ENV_LOCK.lock().unwrap();
        std::env::set_var(FONT_PATH_ENV, "/tmp/env-fonts");
        let extra = vec![PathBuf::from("/tmp/caller")];
        let dirs = custom_font_dirs(&extra);
        assert_eq!(
            dirs,
            vec![
                PathBuf::from("/tmp/caller"),
                PathBuf::from("/tmp/env-fonts")
            ],
            "custom 로더는 호출자 지정 → 환경변수만 담는다"
        );
        for sys in system_font_dirs() {
            assert!(!dirs.contains(&sys), "시스템 디렉터리 포함 금지: {sys:?}");
        }
        assert!(
            !dirs.contains(&PathBuf::from(BUNDLED_OPENSOURCE_DIR)),
            "번들은 custom 이 아니라 최후-폴백 로더로 간다"
        );
        std::env::remove_var(FONT_PATH_ENV);
    }

    /// [#3300] 번들 최후-폴백 로더는 저장소 자산만 담는다(#2293 한국어 드롭 방지).
    #[test]
    fn bundled_font_dirs_is_repo_asset_only() {
        assert_eq!(
            bundled_font_dirs(),
            vec![PathBuf::from(BUNDLED_OPENSOURCE_DIR)]
        );
    }

    /// `--packaged-fonts-only`: 탐색 경로는 호출자 지정 경로뿐이다 — 환경변수·
    /// 시스템 디렉터리·번들 `ttfs/opensource` 가 모두 빠진다.
    #[test]
    fn search_dirs_packaged_only_returns_caller_dirs_only() {
        let _g = ENV_LOCK.lock().unwrap();
        std::env::set_var(FONT_PATH_ENV, "/tmp/env-fonts");
        let extra = vec![
            PathBuf::from("/tmp/caller-a"),
            PathBuf::from("/tmp/caller-b"),
        ];
        assert_eq!(
            search_dirs_with_mode(&extra, true),
            extra,
            "패키지 전용 모드는 호출자 지정 경로만 순서대로 돌려준다"
        );
        assert!(
            search_dirs_with_mode(&[], true).is_empty(),
            "호출자 경로가 없으면 탐색 경로도 없다"
        );
        // 모드가 꺼져 있으면 종전 순서 그대로(환경변수·시스템·번들 포함).
        let default_dirs = search_dirs_with_mode(&extra, false);
        assert!(default_dirs.contains(&PathBuf::from("/tmp/env-fonts")));
        assert_eq!(
            default_dirs.last(),
            Some(&PathBuf::from(BUNDLED_OPENSOURCE_DIR))
        );
        std::env::remove_var(FONT_PATH_ENV);
    }

    #[test]
    fn custom_and_bundled_dirs_packaged_only_drop_env_and_bundle() {
        let _g = ENV_LOCK.lock().unwrap();
        std::env::set_var(FONT_PATH_ENV, "/tmp/env-fonts");
        let extra = vec![PathBuf::from("/tmp/caller")];
        assert_eq!(custom_font_dirs_with_mode(&extra, true), extra);
        assert!(
            bundled_font_dirs_with_mode(true).is_empty(),
            "패키지 전용 모드는 번들 Noto Sans KR 을 등록하지 않는다"
        );
        assert_eq!(
            bundled_font_dirs_with_mode(false),
            vec![PathBuf::from(BUNDLED_OPENSOURCE_DIR)]
        );
        std::env::remove_var(FONT_PATH_ENV);
    }

    /// fontdb 적재: 패키지 전용 모드에서는 호출자 경로의 폰트만 들어가고,
    /// 환경변수 경로의 폰트는 들어가지 않는다.
    #[test]
    fn load_into_fontdb_packaged_only_loads_caller_fonts_only() {
        let _g = ENV_LOCK.lock().unwrap();
        let fixtures = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/fonts"));
        let bundled = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/ttfs/opensource"));
        // 환경변수로 번들 디렉터리를 가리켜, 모드가 이를 무시하는지 본다.
        std::env::set_var(FONT_PATH_ENV, &bundled);

        let mut packaged = usvg::fontdb::Database::new();
        load_into_fontdb_with_mode(&mut packaged, std::slice::from_ref(&fixtures), true);
        assert!(packaged.len() > 0, "호출자 경로 폰트는 적재돼야 한다");
        let has_noto = |db: &usvg::fontdb::Database| {
            db.faces().any(|f| {
                f.families
                    .iter()
                    .any(|(n, _)| n.starts_with("Noto Sans KR"))
            })
        };
        assert!(
            !has_noto(&packaged),
            "패키지 전용 모드에서는 환경변수·번들 경로의 Noto Sans KR 이 없어야 한다"
        );

        let mut default_db = usvg::fontdb::Database::new();
        load_into_fontdb_with_mode(&mut default_db, std::slice::from_ref(&fixtures), false);
        assert!(
            has_noto(&default_db),
            "기본 모드는 환경변수 경로도 적재한다"
        );
        std::env::remove_var(FONT_PATH_ENV);
    }
}
