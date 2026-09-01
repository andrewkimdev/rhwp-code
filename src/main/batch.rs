//! batch 모듈 — src/main.rs 에서 무변동 이동
use super::*;

/// `batch` 는 stdin 전체를 파일 경로 목록으로 소비한다. 전역 인증 옵션 중 stdin
/// 변형은 그 목록과 같은 바이트 스트림을 두 번 읽으려 하고, 리터럴 변형도 worker
/// thread-local 인증 상태로 전달되지 않는다. 따라서 암호화 batch 를 정식으로 설계하기
/// 전에는 네 옵션을 모두 호출 경계에서 거부한다.
///
/// 명령 위치 앞의 전역 인증 옵션만 건너뛰어 `batch` 여부를 판정한다. 단순히 모든 인자에서
/// `batch` 문자열을 찾으면 `search --query batch` 같은 정상 호출을 잘못 막게 된다.
pub(crate) fn is_batch_invocation(args: &[String]) -> bool {
    let mut i = 1; // args[0] 은 프로그램 경로
    while let Some(arg) = args.get(i) {
        match arg.as_str() {
            "--password" | "--output-password" => i += 2,
            "--password-stdin" | "--output-password-stdin" => i += 1,
            _ => return arg == "batch",
        }
    }
    false
}



pub(crate) fn run_batch(args: &[String]) -> i32 {
    use std::io::{BufRead, Write};

    const USAGE: &str = "사용법: <파일 목록> | rhwp batch <export-text|info|export-structure|export-tables|fields|search|extract-data|convert> --json [--mode auto|outline|clause] [--query <검색어>] [--kind date|amount|number|all] [--limit <N>] [--threads <N>] [convert: --out-dir <폴더> [--verify] [--verify-pages]]  (stdin: 한 줄당 파일 경로 하나)\n      rhwp batch fill --form <서식> --data <행.jsonl|행.csv> --out-dir <폴더> --json  (fill 만 stdin 을 읽지 않는다)";

    let subcommand = args.first().map(String::as_str);
    // [#3719 §6-6] fill 축은 **입력 축 자체가 다르다** — stdin 파일 목록이 아니라 서식 1 개와
    // 데이터 파일 1 개를 받고, 산출은 행 수만큼 나온다. 인자 문법이 다른 축과 겹치지 않으므로
    // 파싱부터 갈라 놓는다(경로 목록 읽기를 절대 타지 않게 하는 것이 요점이다).
    if subcommand == Some("fill") {
        return run_batch_fill(&args[1..]);
    }
    let is_structure = subcommand == Some("export-structure");
    // [#3346] --query 는 search 축 전용이다 (--mode 가 export-structure 전용인 것과 같은 규약).
    let is_search = subcommand == Some("search");
    // [#3626] --out-dir·--verify·--verify-pages 는 convert 축 전용이다 (같은 규약).
    let is_convert = subcommand == Some("convert");
    // [#3830] --kind·--limit 는 extract-data 축 전용이다 (같은 규약).
    let is_extract_data = subcommand == Some("extract-data");
    if !matches!(
        subcommand,
        Some("export-text")
            | Some("info")
            | Some("export-structure")
            | Some("export-tables")
            | Some("fields")
            | Some("search")
            | Some("extract-data")
            | Some("convert")
    ) {
        match subcommand {
            Some(unknown) => eprintln!(
                "오류: batch 는 export-text·info·export-structure·export-tables·fields·search·extract-data·convert·fill 만 지원합니다 - {}",
                unknown
            ),
            None => eprintln!("오류: batch 서브커맨드를 지정해주세요."),
        }
        eprintln!("{USAGE}");
        return EXIT_USAGE;
    }

    let mut json_mode = false;
    let mut threads_opt: Option<usize> = None;
    let mut structure_mode = rhwp::document_core::queries::structure::StructureMode::Auto;
    let mut search_query: Option<String> = None;
    // [#3830] extract-data 축 전용 — 종류 필터·문서당 상한.
    let mut extract_kind = "all".to_string();
    let mut extract_limit: Option<usize> = None;
    // [#3626] convert 축 전용 — 목적지와 검증 게이트.
    let mut out_dir: Option<std::path::PathBuf> = None;
    // batch 레코드는 언제나 JSON 이므로 json 은 켠 채로 둔다 — verify/verify_pages 만 옵션.
    let mut verify_options = ConversionVerifyOptions {
        json: true,
        ..ConversionVerifyOptions::default()
    };
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--json" => {
                json_mode = true;
                i += 1;
            }
            "--out-dir" => {
                // [#3626] --out-dir 는 convert 축 전용이다.
                if !is_convert {
                    eprintln!("오류: --out-dir 는 convert 에서만 사용할 수 있습니다.");
                    return EXIT_USAGE;
                }
                let Some(value) = args.get(i + 1) else {
                    eprintln!("오류: --out-dir 뒤에 폴더 경로가 필요합니다.");
                    return EXIT_USAGE;
                };
                if value.is_empty() || value.starts_with('-') {
                    eprintln!(
                        "오류: --out-dir 뒤에 플래그가 아닌 폴더 경로가 필요합니다 (이름이 - 로 시작하면 ./ 를 붙이세요)."
                    );
                    return EXIT_USAGE;
                }
                out_dir = Some(std::path::PathBuf::from(value));
                i += 2;
            }
            "--verify" | "--verify-pages" => {
                // 옵션 이름을 리터럴로 고정한다 — 인자에서 온 문자열을 그대로 찍으면
                // CodeQL cleartext-logging 대상이 된다(extract-pages 와 같은 규약).
                let opt: &'static str = if args[i] == "--verify" {
                    "--verify"
                } else {
                    "--verify-pages"
                };
                // [#3626] 검증 게이트는 파일을 쓰는 convert 축에서만 뜻이 있다.
                if !is_convert {
                    eprintln!("오류: {opt} 는 convert 에서만 사용할 수 있습니다.");
                    return EXIT_USAGE;
                }
                if opt == "--verify" {
                    verify_options.verify = true;
                } else {
                    verify_options.verify_pages = true;
                }
                i += 1;
            }
            "--query" => {
                // [#3346] --query 는 search 축 전용이다.
                if !is_search {
                    eprintln!("오류: --query 는 search 에서만 사용할 수 있습니다.");
                    return EXIT_USAGE;
                }
                let Some(value) = args.get(i + 1) else {
                    eprintln!("오류: --query 뒤에 검색어가 필요합니다.");
                    return EXIT_USAGE;
                };
                if value.is_empty() {
                    eprintln!("오류: --query 검색어가 비어 있습니다.");
                    return EXIT_USAGE;
                }
                search_query = Some(value.clone());
                i += 2;
            }
            "--kind" => {
                // [#3830] --kind 는 extract-data 축 전용이다.
                if !is_extract_data {
                    eprintln!("오류: --kind 는 extract-data 에서만 사용할 수 있습니다.");
                    return EXIT_USAGE;
                }
                let Some(value) = args.get(i + 1) else {
                    eprintln!("오류: --kind 뒤에 date|amount|number|all 이 필요합니다.");
                    return EXIT_USAGE;
                };
                match value.as_str() {
                    "all" => extract_kind = "all".to_string(),
                    v if rhwp::document_core::queries::extract_data::DataKind::parse(v)
                        .is_some() =>
                    {
                        extract_kind = v.to_string();
                    }
                    _ => {
                        eprintln!("오류: --kind 는 date|amount|number|all 중 하나여야 합니다.");
                        return EXIT_USAGE;
                    }
                }
                i += 2;
            }
            "--limit" => {
                // [#3830] --limit 는 extract-data 축 전용 — **문서마다** 적용되는 상한이다.
                if !is_extract_data {
                    eprintln!("오류: --limit 는 extract-data 에서만 사용할 수 있습니다.");
                    return EXIT_USAGE;
                }
                let Some(value) = args.get(i + 1) else {
                    eprintln!("오류: --limit 뒤에 1 이상의 정수가 필요합니다.");
                    return EXIT_USAGE;
                };
                match value.parse::<usize>() {
                    Ok(n) if n >= 1 => extract_limit = Some(n),
                    _ => {
                        eprintln!("오류: --limit 뒤에 1 이상의 정수가 필요합니다.");
                        return EXIT_USAGE;
                    }
                }
                i += 2;
            }
            "--mode" => {
                // [#3261] --mode 는 export-structure 축 전용이다.
                if !is_structure {
                    eprintln!("오류: --mode 는 export-structure 에서만 사용할 수 있습니다.");
                    return EXIT_USAGE;
                }
                let Some(value) = args.get(i + 1) else {
                    eprintln!("오류: --mode 뒤에 auto|outline|clause 가 필요합니다.");
                    return EXIT_USAGE;
                };
                match rhwp::document_core::queries::structure::StructureMode::parse(value) {
                    Some(m) => structure_mode = m,
                    None => {
                        eprintln!("오류: --mode 는 auto|outline|clause - {}", value);
                        return EXIT_USAGE;
                    }
                }
                i += 2;
            }
            "--threads" => {
                let Some(value) = args.get(i + 1) else {
                    eprintln!("오류: --threads 뒤에 스레드 수가 필요합니다.");
                    return EXIT_USAGE;
                };
                match value.parse::<usize>() {
                    Ok(n) if n >= 1 => threads_opt = Some(n),
                    _ => {
                        eprintln!("오류: 스레드 수가 올바르지 않습니다 - {}", value);
                        return EXIT_USAGE;
                    }
                }
                i += 2;
            }
            other => {
                eprintln!("알 수 없는 옵션: {}", other);
                eprintln!("{USAGE}");
                return EXIT_USAGE;
            }
        }
    }
    if !json_mode {
        eprintln!("오류: batch 는 현재 --json 출력만 지원합니다.");
        eprintln!("{USAGE}");
        return EXIT_USAGE;
    }

    let mode = match subcommand {
        Some("export-text") => BatchMode::ExportText,
        Some("info") => BatchMode::Info,
        Some("export-tables") => BatchMode::Tables,
        Some("fields") => BatchMode::Fields,
        Some("search") => {
            let Some(q) = search_query.as_deref() else {
                eprintln!("오류: batch search 는 --query <검색어> 가 필요합니다.");
                eprintln!("{USAGE}");
                return EXIT_USAGE;
            };
            BatchMode::Search { query: q }
        }
        Some("extract-data") => BatchMode::ExtractData {
            kind: extract_kind.as_str(),
            limit: extract_limit,
        },
        Some("convert") => {
            // [#3626] 목적지는 명시적이어야 한다. 읽기 전용 6축과 달리 이 축은 입력마다
            // 파일을 쓰는데, 경로는 stdin 에서 오므로 호출자가 산출물이 어디 생기는지
            // 명령줄만 보고 알 수 없으면 안 된다.
            let Some(dir) = out_dir.as_deref() else {
                eprintln!("오류: batch convert 는 --out-dir <폴더> 가 필요합니다.");
                eprintln!("{USAGE}");
                return EXIT_USAGE;
            };
            BatchMode::Convert {
                out_dir: dir,
                verify: verify_options,
            }
        }
        _ => BatchMode::Structure(structure_mode),
    };

    let stdin = std::io::stdin();
    let mut paths: Vec<String> = Vec::new();
    for line in stdin.lock().lines() {
        match line {
            Ok(l) => {
                let path = l.trim().to_string();
                if !path.is_empty() {
                    paths.push(path);
                }
            }
            Err(e) => {
                eprintln!("오류: stdin 읽기 실패 - {}", e);
                return EXIT_RUNTIME;
            }
        }
    }

    // [#3626] 변환 축은 파일을 쓴다 — 읽기 전용 6축에 없던 사전 점검이 필요하다.
    // 산출 이름은 입력 파일 이름만 따르므로 서로 다른 폴더의 같은 이름이 한 경로로 겹친다.
    // 겹침을 레코드로 보고하며 진행하면 이미 절반이 변환된 산출 폴더가 남는다. 한 바이트도
    // 쓰기 전에 전건을 미리 계산해 잡고, 잡히면 사용법 오류로 끝낸다(부분 산출물 없음).
    if let BatchMode::Convert { out_dir, .. } = mode {
        let mut claimed: std::collections::HashMap<String, &str> =
            std::collections::HashMap::with_capacity(paths.len());
        for path in &paths {
            let candidate = batch_convert_output_path(out_dir, Path::new(path));
            if let Some(first) =
                claimed.insert(batch_convert_collision_key(&candidate), path.as_str())
            {
                eprintln!(
                    "오류: 산출 경로가 겹칩니다 - {} ← {} · {}",
                    candidate.display(),
                    first,
                    path
                );
                eprintln!(
                    "      --out-dir 는 입력 파일 이름만 남기므로 서로 다른 폴더의 같은 이름을 구분할 수 없습니다. 입력을 나눠 실행하세요."
                );
                return EXIT_USAGE;
            }
        }
        if let Err(e) = fs::create_dir_all(out_dir) {
            eprintln!(
                "오류: 출력 폴더를 만들 수 없습니다 - {}: {}",
                out_dir.display(),
                e
            );
            return EXIT_RUNTIME;
        }
    }

    let threads = threads_opt
        .unwrap_or_else(|| {
            std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(1)
        })
        .max(1);

    let started = std::time::Instant::now();
    let stdout = std::io::stdout();
    let mut out = std::io::BufWriter::new(stdout.lock());

    let tally = batch_stream_records(
        paths.len(),
        threads,
        |idx| batch_record(mode, &paths[idx]),
        &mut out,
    );

    if tally.aborted {
        return EXIT_RUNTIME;
    }
    if let Err(e) = out.flush() {
        eprintln!("오류: stdout 쓰기 실패 - {}", e);
        return EXIT_RUNTIME;
    }

    eprintln!(
        "batch: {}건 중 {} 성공, {} 실패 ({}ms, threads={})",
        tally.emitted,
        tally.emitted - tally.failed,
        tally.failed,
        started.elapsed().as_millis(),
        threads
    );
    if tally.verify_diff > 0 || tally.verify_pages_diff > 0 {
        eprintln!(
            "batch: 검증 판정 — verify 차이 {}건, verify-pages 불일치 {}건 (변환·저장 자체는 성공)",
            tally.verify_diff, tally.verify_pages_diff
        );
    }
    tally.exit_code()
}



/// [#3238→#3719] 작업 간 병렬 처리 + 한계 재정렬 버퍼(bounded reorder buffer) 스트리밍.
///
/// 배리어 없이 완전 병렬로 돌리되, 완료 레코드는 **입력 순서대로** 즉시 방출한다.
/// 완료-미방출 레코드가 cap 을 넘으면 워커가 대기(역압)해 메모리를 상한한다.
/// 단, 방출 차례(next_emit) 레코드는 cap 과 무관하게 넣을 수 있어야 교착이 없다 —
/// 느린 작업 하나가 버퍼를 채워도, 그 작업이 곧 방출 차례이므로 항상 전진한다.
///
/// [#3719] `run_batch`(stdin 경로 목록)와 `run_batch_fill`(데이터 행)이 이 하나를 쓴다.
/// 작업 단위가 무엇인지는 `make` 가 정하고, 순서 보존·역압·종료 코드 집계 규약은 공유한다.
pub(crate) fn batch_stream_records<F>(
    n: usize,
    threads: usize,
    make: F,
    out: &mut impl std::io::Write,
) -> BatchStreamTally
where
    F: Fn(usize) -> serde_json::Value + Sync,
{
    let cap = threads.saturating_mul(8).max(1);
    let next_claim = std::sync::atomic::AtomicUsize::new(0);
    let abort = std::sync::atomic::AtomicBool::new(false);
    let buf: std::sync::Mutex<std::collections::HashMap<usize, serde_json::Value>> =
        std::sync::Mutex::new(std::collections::HashMap::new());
    let next_emit = std::sync::atomic::AtomicUsize::new(0);
    let space = std::sync::Condvar::new(); // 버퍼에 자리가 났다
    let ready = std::sync::Condvar::new(); // 방출 차례 레코드가 도착했다

    let (failed, emitted, verify_diff, verify_pages_diff) = std::thread::scope(|scope| {
        for _ in 0..threads.min(n) {
            scope.spawn(|| loop {
                if abort.load(std::sync::atomic::Ordering::Relaxed) {
                    break;
                }
                let idx = next_claim.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                if idx >= n {
                    break;
                }
                let record = make(idx);
                let mut guard = buf.lock().expect("batch buf lock");
                while guard.len() >= cap
                    && idx != next_emit.load(std::sync::atomic::Ordering::Relaxed)
                    && !abort.load(std::sync::atomic::Ordering::Relaxed)
                {
                    guard = space.wait(guard).expect("batch buf lock");
                }
                if abort.load(std::sync::atomic::Ordering::Relaxed) {
                    break;
                }
                guard.insert(idx, record);
                // 방출자는 하나뿐이므로 notify_one 으로 충분하다.
                ready.notify_one();
            });
        }

        // 방출자(현재 스레드): 입력 순서대로 도착 즉시 방출한다. 도착해 있는 연속
        // 레코드는 한 번의 락으로 일괄 드레인하고 notify 도 배치당 1회만 보낸다 —
        // 레코드당 notify_all 은 대기 워커 전원을 헛깨우는 thundering herd 가 된다
        // (271건 실측에서 방출 버스트 구간 수 초 손실).
        let mut failed = 0usize;
        let mut emitted = 0usize;
        // [#3626] 검증 판정은 실패가 아니다 — 변환·저장은 성공했고 산출물도 있다.
        // 실패 계수와 섞으면 소비자가 "읽을 수 없었다"와 "변환은 됐는데 IR 이 다르다"를
        // 종료 코드로 구분할 수 없다.
        let mut verify_diff = 0usize;
        let mut verify_pages_diff = 0usize;
        let mut drained: Vec<serde_json::Value> = Vec::new();
        'emit: while emitted < n {
            drained.clear();
            {
                let mut guard = buf.lock().expect("batch buf lock");
                while guard.get(&emitted).is_none() {
                    guard = ready.wait(guard).expect("batch buf lock");
                }
                while let Some(record) = guard.remove(&emitted) {
                    emitted += 1;
                    drained.push(record);
                }
                next_emit.store(emitted, std::sync::atomic::Ordering::Relaxed);
            }
            space.notify_all();
            for record in &drained {
                if record.get("error").is_some() {
                    failed += 1;
                } else if batch_verdict_differs(record, "verifyPages") {
                    verify_pages_diff += 1;
                } else if batch_verdict_differs(record, "verify") {
                    verify_diff += 1;
                }
                if let Err(e) = writeln!(out, "{record}") {
                    // 파이프 소비자가 끊은 경우(broken pipe 등): 새 작업 수주를 멈추고
                    // 대기 중인 워커를 전부 깨워 정리한다.
                    eprintln!("오류: stdout 쓰기 실패 - {}", e);
                    abort.store(true, std::sync::atomic::Ordering::Relaxed);
                    space.notify_all();
                    break 'emit;
                }
            }
        }
        (failed, emitted, verify_diff, verify_pages_diff)
    });

    BatchStreamTally {
        emitted,
        failed,
        verify_diff,
        verify_pages_diff,
        aborted: abort.load(std::sync::atomic::Ordering::Relaxed),
    }
}

// ─── [#3719 §6-6] batch fill — 서식 1 + 데이터 N행 → 산출 N개 (진짜 메일머지) ───



/// [#3719 §6-6] `batch fill` — 서식 하나에 데이터 N행을 채워 산출 N개를 만든다.
///
/// 다른 batch 축과 **입력 축이 다르다**: stdin 은 읽지 않고, `--data` 파일의 한 행이
/// 산출물 하나가 된다(기존 축의 입력은 '경로'지만 여기서는 '행'이다). 채움 자체는 단건
/// `edit fill-fields` 와 같은 `fill_fields_core` 를 행마다 부를 뿐 — 새 편집 로직은 없다.
pub(crate) fn run_batch_fill(args: &[String]) -> i32 {
    use std::io::Write;

    const USAGE: &str = "사용법: rhwp batch fill --form <서식.hwp|서식.hwpx> --data <행.jsonl|행.csv> --out-dir <폴더> --json [--name-field <필드>] [--verify] [--dry-run] [--threads <N>]\n      데이터는 stdin 이 아니라 --data 파일이다 — 다른 batch 축은 stdin 으로 파일 경로 목록을 받지만 fill 의 입력은 경로가 아니라 '행'이다.";

    let mut form: Option<&str> = None;
    let mut data_path: Option<&str> = None;
    let mut out_dir: Option<std::path::PathBuf> = None;
    let mut name_field: Option<&str> = None;
    let mut json_mode = false;
    let mut verify_mode = false;
    let mut dry_run = false;
    let mut threads_opt: Option<usize> = None;

    let mut i = 0;
    while i < args.len() {
        // 옵션 이름은 리터럴로 고정한다 — 인자에서 온 문자열을 그대로 찍으면 CodeQL
        // cleartext-logging 대상이 된다(batch convert 의 --verify 와 같은 규약).
        let opt: &'static str = match args[i].as_str() {
            "--json" => {
                json_mode = true;
                i += 1;
                continue;
            }
            "--verify" => {
                verify_mode = true;
                i += 1;
                continue;
            }
            "--dry-run" => {
                dry_run = true;
                i += 1;
                continue;
            }
            "--form" => "--form",
            "--data" => "--data",
            "--out-dir" => "--out-dir",
            "--name-field" => "--name-field",
            "--threads" => "--threads",
            other => {
                eprintln!("알 수 없는 옵션: {}", other);
                eprintln!("{USAGE}");
                return EXIT_USAGE;
            }
        };
        let Some(value) = args.get(i + 1) else {
            eprintln!("오류: {opt} 뒤에 값이 필요합니다.");
            eprintln!("{USAGE}");
            return EXIT_USAGE;
        };
        // 값 자리에 플래그가 오면 삼키지 않는다 — 삼키면 "지정했다고 믿는 옵션이 실제로는
        // 없는" 채로 실행돼 산출물이 엉뚱한 곳에 생긴다.
        if value.is_empty() || value.starts_with('-') {
            eprintln!(
                "오류: {opt} 뒤에 플래그가 아닌 값이 필요합니다 (이름이 - 로 시작하면 ./ 를 붙이세요)."
            );
            return EXIT_USAGE;
        }
        match opt {
            "--form" => form = Some(value),
            "--data" => data_path = Some(value),
            "--out-dir" => out_dir = Some(std::path::PathBuf::from(value)),
            "--name-field" => name_field = Some(value),
            _ => match value.parse::<usize>() {
                Ok(n) if n >= 1 => threads_opt = Some(n),
                _ => {
                    eprintln!("오류: 스레드 수가 올바르지 않습니다 - {}", value);
                    return EXIT_USAGE;
                }
            },
        }
        i += 2;
    }

    if !json_mode {
        eprintln!("오류: batch 는 현재 --json 출력만 지원합니다.");
        eprintln!("{USAGE}");
        return EXIT_USAGE;
    }
    // `--dry-run` 에서도 --out-dir 를 요구한다. 선검증은 **실행과 같은 명령줄에서 --dry-run
    // 하나만 빼면 되는 것**이라야 뜻이 있다 — 인자 모양이 다르면 선검증이 통과한 명령과
    // 실제로 실행하는 명령이 서로 다른 명령이 된다.
    let (Some(form), Some(data_path), Some(out_dir)) = (form, data_path, out_dir.as_deref()) else {
        eprintln!(
            "오류: batch fill 은 --form <서식> --data <행 파일> --out-dir <폴더> 가 모두 필요합니다."
        );
        eprintln!("{USAGE}");
        return EXIT_USAGE;
    };

    // 서식은 행마다 다시 열린다. 못 여는 서식이면 N행을 다 돌고 같은 실패를 N번 보고하게
    // 되므로 — 그건 진단이 아니다 — 한 행을 처리하기 전에 여기서 한 번 판정한다.
    let form_bytes = match fs::read(form) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("오류: 서식을 읽을 수 없습니다 - {}: {}", form, e);
            return EXIT_RUNTIME;
        }
    };
    if let Err(e) = rhwp::wasm_api::HwpDocument::from_bytes(&form_bytes) {
        eprintln!("오류: 서식 HWP 파싱 실패 - {}: {}", form, e);
        return EXIT_RUNTIME;
    }
    // [#3383] 산출 형식은 서식 형식을 따른다 — 파일 이름의 확장자도 여기서 정해진다.
    let out_format = edit_output_format(&form_bytes, None);

    let rows = match read_fill_rows(Path::new(data_path)) {
        Ok(r) => r,
        Err((message, code)) => {
            eprintln!("오류: {message}");
            if code == EXIT_USAGE {
                eprintln!("{USAGE}");
            }
            return code;
        }
    };

    // 산출 경로는 **한 행도 쓰기 전에** 전부 정한다 — 병렬 실행에서도 이름이 행 순서만으로
    // 결정되고, 이름 충돌 해소가 실행 순서에 좌우되지 않는다.
    let outputs = batch_fill_output_paths(&rows, out_dir, name_field, out_format.ext());
    if !dry_run {
        if let Err(e) = fs::create_dir_all(out_dir) {
            eprintln!(
                "오류: 출력 폴더를 만들 수 없습니다 - {}: {}",
                out_dir.display(),
                e
            );
            return EXIT_RUNTIME;
        }
    }

    let threads = threads_opt
        .unwrap_or_else(|| {
            std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(1)
        })
        .max(1);

    let started = std::time::Instant::now();
    let stdout = std::io::stdout();
    let mut out = std::io::BufWriter::new(stdout.lock());

    let tally = batch_stream_records(
        rows.len(),
        threads,
        |idx| {
            batch_fill_record(
                form,
                idx,
                &rows[idx],
                outputs[idx].as_deref(),
                dry_run,
                verify_mode,
            )
        },
        &mut out,
    );

    if tally.aborted {
        return EXIT_RUNTIME;
    }
    if let Err(e) = out.flush() {
        eprintln!("오류: stdout 쓰기 실패 - {}", e);
        return EXIT_RUNTIME;
    }

    eprintln!(
        "batch fill: {}행 중 {} 성공, {} 실패 ({}ms, threads={}{})",
        tally.emitted,
        tally.emitted - tally.failed,
        tally.failed,
        started.elapsed().as_millis(),
        threads,
        if dry_run { ", dry-run" } else { "" }
    );
    if tally.verify_diff > 0 {
        eprintln!(
            "batch fill: 검증 판정 — verify 차이 {}건 (채움·저장 자체는 성공)",
            tally.verify_diff
        );
    }
    tally.exit_code()
}



/// [#3719 §6-6] 행 하나 → NDJSON 레코드 하나. 실패도 레코드다(스트림은 계속된다).
///
/// 한 행의 파서 panic 이 메일머지 전체를 죽여서는 안 된다 — 기존 `batch_record` 와 같은
/// 격리 규약이다.
pub(crate) fn batch_fill_record(
    form: &str,
    row_index: usize,
    row: &FillRow,
    output: Option<&Path>,
    dry_run: bool,
    verify_mode: bool,
) -> serde_json::Value {
    let mut record = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        batch_fill_record_inner(form, row, output, dry_run, verify_mode)
    })) {
        Ok(record) => record,
        Err(payload) => {
            let message = payload
                .downcast_ref::<&str>()
                .map(|s| (*s).to_string())
                .or_else(|| payload.downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "원인 불명".to_string());
            batch_fail_record(form, format!("내부 오류(panic): {}", message))
        }
    };
    // 행 번호는 성공·실패 어느 쪽에도 붙는다. 없으면 어느 행이 빠졌는지 셀 수 없어
    // 스트림 전체가 감사 불가가 된다.
    record["row"] = serde_json::json!(row_index);
    record
}


pub(crate) fn batch_fill_record_inner(
    form: &str,
    row: &FillRow,
    output: Option<&Path>,
    dry_run: bool,
    verify_mode: bool,
) -> serde_json::Value {
    let data = match row {
        FillRow::Data(map) => map,
        FillRow::Broken(reason) => return batch_fail_record(form, reason.clone()),
    };
    let Some(output) = output else {
        return batch_fail_record(form, "산출 경로를 정하지 못했습니다".to_string());
    };
    let output_path = output.to_string_lossy().to_string();
    match fill_fields_core(form, data, Some(output_path.clone()), dry_run, verify_mode) {
        Ok(outcome) => {
            let mut record = outcome.envelope;
            if dry_run {
                // 선검증에도 목적지를 밝힌다. 같은 봉투에 `dryRun: true` 가 함께 있으므로
                // "만들 예정" 경로임이 레코드 안에서 구분된다(디스크에 파일은 없다).
                record["output"] = serde_json::Value::String(output_path);
                record["outputFormat"] =
                    serde_json::Value::String(outcome.output_format.label().to_string());
            }
            record
        }
        Err(message) => batch_fail_record(form, message),
    }
}



/// [#3719 §6-6] 행마다 산출 파일 경로를 정한다.
///
/// 이름은 `--name-field` 값, 없으면 1 기준 순번이다. 파일명에 쓸 수 없는 문자는 치환하고,
/// 서로 다른 행이 같은 이름을 내면 뒤에 순번을 붙인다 — 덮어쓰면 앞 행의 산출물이
/// **조용히** 사라져서 성공 레코드 N건과 실제 파일 수가 어긋난다.
pub(crate) fn batch_fill_output_paths(
    rows: &[FillRow],
    out_dir: &Path,
    name_field: Option<&str>,
    ext: &str,
) -> Vec<Option<std::path::PathBuf>> {
    // 대소문자만 다른 이름도 한 파일이 되는 파일시스템(Windows·macOS 기본)이 있다.
    // batch convert 와 같은 보수적 규약으로 소문자 키 하나로 판정해야, OS 를 바꾼
    // 재실행이 달라지지 않는다.
    let mut taken: std::collections::HashSet<String> = std::collections::HashSet::new();
    let width = rows.len().to_string().len().max(4);
    let mut paths = Vec::with_capacity(rows.len());
    for (idx, row) in rows.iter().enumerate() {
        let FillRow::Data(map) = row else {
            // 읽지 못한 행은 산출물이 없다 — 이름도 잡지 않는다.
            paths.push(None);
            continue;
        };
        let seq = format!("{:0width$}", idx + 1, width = width);
        let base = name_field
            .and_then(|f| map.get(f))
            .map(|v| match v {
                serde_json::Value::String(s) => s.clone(),
                other => other.to_string(),
            })
            .map(|s| sanitize_output_stem(&s))
            .filter(|s| !s.is_empty())
            // 이름 필드가 비었거나 치환 후 아무것도 남지 않으면 순번으로 되돌린다.
            .unwrap_or_else(|| seq.clone());

        let mut candidate = base.clone();
        let mut dup = 1usize;
        while !taken.insert(format!("{}.{}", candidate.to_lowercase(), ext)) {
            dup += 1;
            candidate = format!("{base}_{dup}");
        }
        paths.push(Some(out_dir.join(format!("{candidate}.{ext}"))));
    }
    paths
}



/// [#3719 §6-6] 데이터 값에서 파일 이름을 만든다 — 데이터에서 온 문자열이 경로 문법을
/// 타지 못하게 한다.
///
/// 경로 구분자·Windows 금지 문자·제어 문자는 `_` 로 바꾼다. 구분자가 사라지므로
/// `../..` 같은 값도 `--out-dir` 밖으로 나갈 수 없다. Windows 는 이름 끝의 공백·점을
/// 조용히 잘라내므로 미리 없애고, 예약 장치 이름(CON·NUL·COM1…)은 앞에 `_` 를 붙여 피한다.
pub(crate) fn sanitize_output_stem(raw: &str) -> String {
    /// 경로 길이 한도(Windows 260자)에 여유를 두는 이름 길이 상한.
    const MAX_CHARS: usize = 80;
    const RESERVED: [&str; 22] = [
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
        "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    ];

    let mut stem = String::new();
    for ch in raw.chars().take(MAX_CHARS) {
        let forbidden =
            matches!(ch, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') || ch.is_control();
        stem.push(if forbidden { '_' } else { ch });
    }
    let trimmed = stem.trim().trim_end_matches(['.', ' ']).trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let head = trimmed.split('.').next().unwrap_or("").to_ascii_uppercase();
    if RESERVED.contains(&head.as_str()) {
        format!("_{trimmed}")
    } else {
        trimmed.to_string()
    }
}



/// [#3719 §6-6] `--data` 파일 → 행 목록.
///
/// `Err((사유, 종료 코드))` 는 **한 행도 처리하기 전에** 끝낼 입력 오류다(확장자·헤더·
/// 빈 파일). 개별 행의 결함은 여기서 끝내지 않고 `FillRow::Broken` 으로 스트림에 남긴다 —
/// 한 행이 깨졌다고 나머지 N-1행의 산출물을 포기할 이유가 없다.
pub(crate) fn read_fill_rows(path: &Path) -> Result<Vec<FillRow>, (String, i32)> {
    let text = fs::read_to_string(path).map_err(|e| {
        (
            format!("--data 파일을 읽을 수 없습니다 - {}: {}", path.display(), e),
            EXIT_RUNTIME,
        )
    })?;
    // 엑셀이 저장한 CSV 는 UTF-8 BOM 으로 시작한다. 남겨 두면 첫 헤더 이름이 통째로
    // 어긋나(BOM+이름) 문서의 누름틀과 영영 매칭되지 않는다.
    let text: &str = text.strip_prefix('\u{feff}').unwrap_or(&text);

    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    let rows = match ext.as_str() {
        "jsonl" | "ndjson" => parse_jsonl_rows(text),
        "csv" => parse_csv_rows(text)?,
        "" => {
            return Err((
                "--data 파일에 확장자가 없습니다 — .jsonl 또는 .csv 로 지정하세요.".to_string(),
                EXIT_USAGE,
            ));
        }
        other => {
            return Err((
                format!("--data 는 .jsonl 또는 .csv 여야 합니다 - .{other}"),
                EXIT_USAGE,
            ));
        }
    };
    if rows.is_empty() {
        // 0행을 성공(exit 0)으로 끝내면 "전부 처리했다"와 구분되지 않는다.
        return Err((
            format!("--data 에 데이터 행이 없습니다 - {}", path.display()),
            EXIT_USAGE,
        ));
    }
    Ok(rows)
}



/// JSONL: 한 줄 한 객체. 빈 줄은 건너뛴다.
pub(crate) fn parse_jsonl_rows(text: &str) -> Vec<FillRow> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(
            |line| match serde_json::from_str::<serde_json::Value>(line) {
                Ok(serde_json::Value::Object(m)) => FillRow::Data(m),
                Ok(_) => FillRow::Broken(
                    "JSONL 행은 {\"필드이름\":\"값\"} 형식의 JSON 객체여야 합니다".to_string(),
                ),
                Err(e) => FillRow::Broken(format!("JSONL 행 파싱 실패 - {e}")),
            },
        )
        .collect()
}



/// CSV: 첫 줄 헤더가 누름틀 이름이다. 헤더 이름은 **공백까지 그대로** 문서의 이름으로 쓴다.
pub(crate) fn parse_csv_rows(text: &str) -> Result<Vec<FillRow>, (String, i32)> {
    let records = parse_csv_records(text).map_err(|e| (e, EXIT_USAGE))?;
    let mut it = records.into_iter();
    let Some(header) = it.next() else {
        return Err(("--data CSV 에 헤더 줄이 없습니다.".to_string(), EXIT_USAGE));
    };
    for (i, name) in header.iter().enumerate() {
        if name.is_empty() {
            return Err((
                format!("--data CSV 헤더 {}번째 칸의 이름이 비었습니다.", i + 1),
                EXIT_USAGE,
            ));
        }
        // 같은 이름이 두 번이면 뒤 칸이 앞 칸을 덮어 **한 열이 통째로 무시된다**.
        if header[..i].contains(name) {
            return Err((
                format!("--data CSV 헤더에 같은 이름이 두 번 있습니다 - {name}"),
                EXIT_USAGE,
            ));
        }
    }
    Ok(it
        .map(|record| {
            if record.len() != header.len() {
                // 칸 수가 다르면 값이 한 칸씩 밀려 엉뚱한 누름틀로 들어간다. 채우고 나면
                // 아무 오류 없이 잘못된 문서가 나오므로 행 단위로 거부한다.
                return FillRow::Broken(format!(
                    "CSV 칸 수가 헤더와 다릅니다 - 헤더 {}칸, 행 {}칸",
                    header.len(),
                    record.len()
                ));
            }
            FillRow::Data(
                header
                    .iter()
                    .cloned()
                    .zip(record.into_iter().map(serde_json::Value::String))
                    .collect(),
            )
        })
        .collect())
}



/// [#3719 §6-6] RFC 4180 CSV 읽기 — 엑셀 저장본을 그대로 받는다.
///
/// 따옴표 안의 쉼표·줄바꿈·이중 따옴표(`""`)를 보존하고 CRLF/LF 를 모두 줄 끝으로 읽는다.
/// 전용 crate 를 새로 들이지 않는 이유는 여기서 필요한 문법이 RFC 4180 그 자체뿐이라서다.
pub(crate) fn parse_csv_records(text: &str) -> Result<Vec<Vec<String>>, String> {
    let mut records: Vec<Vec<String>> = Vec::new();
    let mut record: Vec<String> = Vec::new();
    let mut field = String::new();
    let mut in_quotes = false;
    let mut chars = text.chars().peekable();

    while let Some(c) = chars.next() {
        if in_quotes {
            if c == '"' {
                if chars.peek() == Some(&'"') {
                    chars.next();
                    field.push('"');
                } else {
                    in_quotes = false;
                }
            } else {
                field.push(c);
            }
            continue;
        }
        match c {
            // 여는 따옴표는 칸 맨 앞에서만 뜻이 있다. 칸 중간의 따옴표는 값의 일부다.
            '"' if field.is_empty() => in_quotes = true,
            ',' => record.push(std::mem::take(&mut field)),
            '\r' | '\n' => {
                if c == '\r' && chars.peek() == Some(&'\n') {
                    chars.next();
                }
                record.push(std::mem::take(&mut field));
                records.push(std::mem::take(&mut record));
            }
            _ => field.push(c),
        }
    }
    if in_quotes {
        // 여기서 멈추지 않으면 "따옴표 하나 빠뜨린 CSV"가 뒤 행 전체를 한 칸으로 삼킨다.
        return Err("--data CSV 의 따옴표가 닫히지 않았습니다.".to_string());
    }
    if !field.is_empty() || !record.is_empty() {
        record.push(field);
        records.push(record);
    }
    // 마지막 개행이 만든 빈 줄은 행이 아니다(엑셀 저장본은 늘 개행으로 끝난다).
    records.retain(|r| !(r.len() == 1 && r[0].trim().is_empty()));
    Ok(records)
}



/// [#3238] 파일 하나를 처리해 NDJSON 레코드 하나를 만든다. 실패는 레코드로 보고하고
/// 스트림은 계속된다 — 프로세스 중단 없이 부분 실패를 종료 코드로 신호하기 위함.
///
/// 배치는 신뢰할 수 없는 대량 코퍼스를 훑는 용도라, 한 건의 파서 panic 이 배치 전체를
/// 죽여서는 안 된다. panic 도 해당 파일의 `error` 레코드로 격리한다.
pub(crate) fn batch_record(mode: BatchMode<'_>, path: &str) -> serde_json::Value {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| match mode {
        BatchMode::ExportText => batch_export_text_record_inner(path),
        BatchMode::Info => batch_info_record_inner(path),
        BatchMode::Structure(structure_mode) => batch_structure_record_inner(path, structure_mode),
        BatchMode::Tables => batch_tables_record_inner(path),
        BatchMode::Fields => batch_fields_record_inner(path),
        BatchMode::Search { query } => batch_search_record_inner(path, query),
        BatchMode::Convert { out_dir, verify } => batch_convert_record_inner(path, out_dir, verify),
        BatchMode::ExtractData { kind, limit } => {
            batch_extract_data_record_inner(path, kind, limit)
        }
    })) {
        Ok(record) => record,
        Err(payload) => {
            let message = payload
                .downcast_ref::<&str>()
                .map(|s| (*s).to_string())
                .or_else(|| payload.downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "원인 불명".to_string());
            batch_fail_record(path, format!("내부 오류(panic): {}", message))
        }
    }
}


pub(crate) fn batch_fail_record(path: &str, message: String) -> serde_json::Value {
    provenance::marked(
        serde_json::json!({
            "schemaVersion": ENVELOPE_SCHEMA_VERSION,
            "source": path,
            "error": message,
            "exitClass": "runtime",
        }),
        "batch",
    )
}


pub(crate) fn batch_export_text_record_inner(path: &str) -> serde_json::Value {
    let data = match fs::read(path) {
        Ok(d) => d,
        Err(e) => return batch_fail_record(path, format!("파일을 읽을 수 없습니다: {}", e)),
    };
    let doc = match rhwp::wasm_api::HwpDocument::from_bytes(&data) {
        Ok(d) => d,
        Err(e) => return batch_fail_record(path, format!("파싱 실패: {}", e)),
    };

    let page_count = doc.page_count();
    let mut text = String::new();
    for page_num in 0..page_count {
        match doc.extract_page_text_native(page_num) {
            Ok(t) => {
                text.push_str(&t);
                if !t.ends_with('\n') {
                    text.push('\n');
                }
            }
            Err(e) => {
                return batch_fail_record(
                    path,
                    format!("페이지 {} 텍스트 추출 실패: {}", page_num, e),
                )
            }
        }
    }

    provenance::marked(
        serde_json::json!({
            "schemaVersion": ENVELOPE_SCHEMA_VERSION,
            "source": path,
            "pageCount": page_count,
            "text": text,
        }),
        "export-text",
    )
}



/// [#3830] `batch extract-data --json` 의 파일당 레코드 — 단건 `extract-data --json`
/// 봉투(`extract_data_json_value` 공유)와 같은 스키마다. 추출 로직은 새로 만들지 않고
/// `DocumentCore::extract_data` 를 그대로 부른다(`extract_data_command` 와 동일한 절차).
///
/// [§6-10] `limit` 은 **이 문서 하나**에 대한 상한이다 — 배치 전체에 걸친 전역 상한이
/// 아니다. 전역 상한으로 읽으면 앞선 문서가 한도를 다 써버려 뒤 문서가 조용히 0건으로
/// 보고되고, 소비자는 "그 문서에 값이 없다"와 "한도를 이미 다 썼다"를 구별할 수 없다.
/// 그래서 문서마다 독립적으로 전수 추출 후 절단한다 — 단건 `extract-data` 와 같은 규약.
pub(crate) fn batch_extract_data_record_inner(
    path: &str,
    kind_arg: &str,
    limit: Option<usize>,
) -> serde_json::Value {
    use rhwp::document_core::queries::extract_data::DataKind;

    let data = match fs::read(path) {
        Ok(d) => d,
        Err(e) => return batch_fail_record(path, format!("파일을 읽을 수 없습니다: {}", e)),
    };
    let doc = match rhwp::wasm_api::HwpDocument::from_bytes(&data) {
        Ok(d) => d,
        Err(e) => return batch_fail_record(path, format!("파싱 실패: {}", e)),
    };

    let selected: Vec<DataKind> = if kind_arg == "all" {
        DataKind::ALL.to_vec()
    } else {
        DataKind::parse(kind_arg).into_iter().collect()
    };

    let all_items = doc.extract_data(&selected);
    let total_item_count = all_items.len();
    let mut counts = serde_json::Map::new();
    for kind in &selected {
        let n = all_items.iter().filter(|it| it.kind == *kind).count();
        counts.insert(kind.as_str().to_string(), serde_json::json!(n));
    }
    let counts = serde_json::Value::Object(counts);

    let items: Vec<_> = match limit {
        Some(n) => all_items.into_iter().take(n).collect(),
        None => all_items,
    };

    extract_data_json_value(path, kind_arg, &items, total_item_count, &counts)
}



/// [#3261] `batch export-structure --json` 의 파일당 레코드 — `export-structure --json`
/// 봉투(`structure_json_value` 공유)와 같은 스키마다.
pub(crate) fn batch_structure_record_inner(
    path: &str,
    mode: rhwp::document_core::queries::structure::StructureMode,
) -> serde_json::Value {
    let data = match fs::read(path) {
        Ok(d) => d,
        Err(e) => return batch_fail_record(path, format!("파일을 읽을 수 없습니다: {}", e)),
    };
    let doc = match rhwp::wasm_api::HwpDocument::from_bytes(&data) {
        Ok(d) => d,
        Err(e) => return batch_fail_record(path, format!("파싱 실패: {}", e)),
    };
    let st = rhwp::document_core::queries::structure::build_structure(doc.document(), mode);
    structure_json_value(path, &st)
}



/// [#3346] `batch export-tables --json` 의 파일당 레코드 — `export-tables --json` 봉투와
/// 같은 스키마(`tables_json_value` 공유)다.
pub(crate) fn batch_tables_record_inner(path: &str) -> serde_json::Value {
    use rhwp::document_core::queries::table_extract::extract_tables;
    let data = match fs::read(path) {
        Ok(d) => d,
        Err(e) => return batch_fail_record(path, format!("파일을 읽을 수 없습니다: {}", e)),
    };
    let doc = match rhwp::wasm_api::HwpDocument::from_bytes(&data) {
        Ok(d) => d,
        Err(e) => return batch_fail_record(path, format!("파싱 실패: {}", e)),
    };
    let tables = extract_tables(doc.document());
    tables_json_value(path, &tables)
}



/// [#3346] `batch fields --json` 의 파일당 레코드 — `fields --json` 봉투와 같은 스키마.
pub(crate) fn batch_fields_record_inner(path: &str) -> serde_json::Value {
    let data = match fs::read(path) {
        Ok(d) => d,
        Err(e) => return batch_fail_record(path, format!("파일을 읽을 수 없습니다: {}", e)),
    };
    let doc = match rhwp::wasm_api::HwpDocument::from_bytes(&data) {
        Ok(d) => d,
        Err(e) => return batch_fail_record(path, format!("파싱 실패: {}", e)),
    };
    let fields = collect_field_records(&doc);
    fields_json_value(path, &fields)
}



/// [#3346] `batch search --json` 의 파일당 레코드 — `search --json` 봉투와 같은 스키마.
///
/// 대량 코퍼스에서 한 문서가 매치를 수만 건 쏟아내면 스트림이 부풀므로, 배치 경로는
/// 파일당 매치 상한을 둔다(단건 `search --limit` 과 같은 취지).
pub(crate) fn batch_search_record_inner(path: &str, query: &str) -> serde_json::Value {
    const BATCH_MATCH_LIMIT: usize = 1000;
    let data = match fs::read(path) {
        Ok(d) => d,
        Err(e) => return batch_fail_record(path, format!("파일을 읽을 수 없습니다: {}", e)),
    };
    let doc = match rhwp::wasm_api::HwpDocument::from_bytes(&data) {
        Ok(d) => d,
        Err(e) => return batch_fail_record(path, format!("파싱 실패: {}", e)),
    };
    // 단건 `search --limit`와 동일하게 전체 매치 수를 먼저 관찰하고, NDJSON 크기만
    // 배치 상한으로 자른다. 그래야 단건·배치가 같은 envelope 계약을 공유한다.
    let all_matches = doc.grep(query, true, None);
    let total_match_count = all_matches.len();
    let matches: Vec<_> = all_matches.into_iter().take(BATCH_MATCH_LIMIT).collect();
    search_json_value(path, query, true, &matches, total_match_count)
}



/// [#3626] `batch convert` 의 산출 경로 — `<out-dir>/<입력 파일이름>.hwp`.
///
/// stdin 은 한 줄에 경로 하나뿐이라 출력 경로를 함께 받을 자리가 없다. 그래서 정책으로
/// 정한다: 목적지는 `--out-dir` 하나, 이름은 입력 파일 이름을 따른다. 입력 폴더 구조를
/// 미러링하지 않는 것은 의도다 — 절대 경로·`..`·드라이브 문자가 섞인 목록에서는 "무엇을
/// 기준으로 한 상대 경로인가"가 정의되지 않는다. 대신 이름 겹침은 `run_batch` 가 **한
/// 바이트도 쓰기 전에** 전건 사전 점검으로 잡는다.
pub(crate) fn batch_convert_output_path(out_dir: &Path, input: &Path) -> std::path::PathBuf {
    let stem = input
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "output".to_string());
    out_dir.join(format!("{stem}.hwp"))
}



/// batch convert 는 macOS/Windows 기본 파일시스템에서도 안전해야 한다. 따라서
/// 대소문자만 다른 두 입력이 같은 산출물을 덮어쓰는 일을 모든 호스트에서 미리
/// 금지한다. Linux 에서도 이 보수적 규약을 공유해야 OS를 바꾼 재실행이 달라지지 않는다.
pub(crate) fn batch_convert_collision_key(output: &Path) -> String {
    output.to_string_lossy().to_lowercase()
}



/// [#3626] 검증 판정 봉투가 "차이 있음"인가. 필드가 없거나 null 이면 판정 자체가 없다.
pub(crate) fn batch_verdict_differs(record: &serde_json::Value, key: &str) -> bool {
    record
        .get(key)
        .and_then(|v| v.get("identical"))
        .and_then(|v| v.as_bool())
        == Some(false)
}



/// [#3626] `batch convert --json` 의 파일당 레코드 — 단건 `convert --json` 봉투와 같은
/// 스키마다. 쪽수 불일치면 IR 비교를 하지 않고 `verify: null` 로 두는 단락(short-circuit)
/// 까지 단건과 같다.
///
/// 다른 것은 끝내는 방식뿐이다. 단건은 검증 차이에서 `process::exit(3|4)` 로 프로세스를
/// 끊지만 배치는 뒤 파일이 남아 있어 끊을 수 없다. 그래서 판정은 레코드에만 담고
/// (`ir-diff --json` 과 같은 "판정은 데이터" 규약) `run_batch` 가 전건을 모아 집계한다.
///
/// 재파싱 실패는 "판정 불가"가 아니라 **열 수 없는 산출물**이므로, 단건이 3/4 로 끝내는
/// 것과 달리 배치가 가진 `error` 레코드 채널로 보고한다(→ 최종 exit 1). 배치에는 단건에
/// 없는 실패 채널이 있고, 이쪽이 소비자에게 더 정확하다.
pub(crate) fn batch_convert_record_inner(
    path: &str,
    out_dir: &Path,
    verify_options: ConversionVerifyOptions,
) -> serde_json::Value {
    let input_path = Path::new(path);
    let output_path = batch_convert_output_path(out_dir, input_path);
    // 사전 점검은 산출물끼리의 겹침만 본다. "산출 경로가 곧 그 입력"(--out-dir 이 입력
    // 폴더이고 입력이 이미 .hwp)은 파일 동일성 판정이 필요하므로 여기서 막는다 —
    // 단건 convert/export-hwpx 의 "원본을 덮어쓰지 않는다" 가드와 같은 규약.
    if paths_refer_to_same_file(input_path, &output_path) {
        return batch_fail_record(
            path,
            "입력과 출력 경로가 같습니다. 원본을 덮어쓰지 않습니다.".to_string(),
        );
    }

    let data = match fs::read(input_path) {
        Ok(d) => d,
        Err(e) => return batch_fail_record(path, format!("파일을 읽을 수 없습니다: {}", e)),
    };
    // [#3505] --verify 비교 강도를 정하려면 원본 포맷을 알아야 한다 (대상은 항상 HWP5).
    let source_format = rhwp::parser::detect_format(&data);
    let mut doc = match rhwp::wasm_api::HwpDocument::from_bytes(&data) {
        Ok(d) => d,
        Err(e) => return batch_fail_record(path, format!("파싱 실패: {:?}", e)),
    };

    let page_count_before = if verify_options.verify_pages {
        Some(doc.page_count())
    } else {
        None
    };
    let was_distribution = doc.document().header.distribution;
    if let Err(e) = doc.convert_to_editable_native() {
        return batch_fail_record(path, format!("변환 실패: {:?}", e));
    }

    let bytes = match doc.export_hwp_with_adapter() {
        Ok(b) => b,
        Err(e) => return batch_fail_record(path, format!("직렬화 실패: {:?}", e)),
    };
    if let Err(e) = fs::write(&output_path, &bytes) {
        // [#2707] 출력 파일이 아예 안 만들어졌는데 성공 레코드를 내던 부류의 경로.
        return batch_fail_record(
            path,
            format!("파일 저장 실패 - {}: {}", output_path.display(), e),
        );
    }

    let bytes_len = bytes.len();
    let envelope = |verify: serde_json::Value, verify_pages: serde_json::Value| {
        provenance::marked(
            serde_json::json!({
                "schemaVersion": ENVELOPE_SCHEMA_VERSION,
                "source": path,
                "output": output_path.display().to_string(),
                "format": "hwp5",
                "bytes": bytes_len,
                "wasDistribution": was_distribution,
                // batch 는 비밀번호 옵션을 받지 않는다(run_batch 가드) — 늘 false 다.
                "passwordProtected": false,
                "verify": verify,
                "verifyPages": verify_pages,
            }),
            "convert",
        )
    };

    if !verify_options.enabled() {
        return envelope(serde_json::Value::Null, serde_json::Value::Null);
    }

    let reloaded = match rhwp::wasm_api::HwpDocument::from_bytes(&bytes) {
        Ok(d) => d,
        Err(e) => {
            return batch_fail_record(path, format!("검증 실패: 저장된 HWP 재파싱 실패 - {:?}", e))
        }
    };

    let mut verify_pages_report = serde_json::Value::Null;
    if let Some(before) = page_count_before {
        let after = reloaded.page_count();
        verify_pages_report = serde_json::json!({
            "before": before, "after": after, "identical": before == after,
        });
        if before != after {
            // 단건 convert 와 같은 단락 — 쪽수가 다르면 IR 비교까지 가지 않는다.
            return envelope(serde_json::Value::Null, verify_pages_report);
        }
    }

    let mut verify_report = serde_json::Value::Null;
    if verify_options.verify {
        let diff =
            rhwp::serializer::hwpx::roundtrip::diff_documents(doc.document(), reloaded.document());
        // [#3505, #3930] 출처별로 대상 포맷에 표현 자리가 없는 항목만 걷어낸다.
        let diff = match source_format {
            rhwp::parser::FileFormat::Hwp => diff,
            rhwp::parser::FileFormat::Hwpx => {
                rhwp::serializer::hwpx::roundtrip::strip_hwpx_to_hwp_noise(diff)
            }
            _ => rhwp::serializer::hwpx::roundtrip::strip_cross_format_noise(diff),
        };
        verify_report = serde_json::json!({
            "identical": diff.is_empty(), "diffCount": diff.differences.len(),
        });
    }

    envelope(verify_report, verify_pages_report)
}



/// [#3238] `batch info --json` 의 파일당 레코드 — `info --json` 과 같은 스키마
/// (`info_json_value` 공유)라 소비자가 단건/배치를 같은 코드로 읽는다.
pub(crate) fn batch_info_record_inner(path: &str) -> serde_json::Value {
    let data = match fs::read(path) {
        Ok(d) => d,
        Err(e) => return batch_fail_record(path, format!("파일을 읽을 수 없습니다: {}", e)),
    };
    let file_size = data.len();
    let detected_format = rhwp::parser::detect_format(&data);
    let doc = match rhwp::wasm_api::HwpDocument::from_bytes(&data) {
        Ok(d) => d,
        Err(e) => return batch_fail_record(path, format!("파싱 실패: {}", e)),
    };
    info_json_value(path, file_size, detected_format, &doc)
}



/// [#3261] `export-structure --json`·`batch export-structure --json` 이 공유하는
/// 구조 봉투 레코드. `mode`/`nodeCount` 를 톱레벨로 올려 스윕 선별(jq select)이 싸다.
pub(crate) fn structure_json_value(
    file_path: &str,
    st: &rhwp::document_core::queries::structure::StructureDoc,
) -> serde_json::Value {
    provenance::marked(
        serde_json::json!({
            "schemaVersion": ENVELOPE_SCHEMA_VERSION,
            "source": file_path,
            "mode": st.mode,
            "nodeCount": st.node_count,
            "structure": st,
        }),
        "export-structure",
    )
}



/// [#3346] `export-tables --json` 과 `batch export-tables` 가 공유하는 봉투.
pub(crate) fn tables_json_value(
    file_path: &str,
    tables: &[rhwp::document_core::queries::table_extract::TableGrid],
) -> serde_json::Value {
    provenance::marked(
        serde_json::json!({
            "schemaVersion": ENVELOPE_SCHEMA_VERSION,
            "source": file_path,
            "tableCount": tables.len(),
            "tables": tables,
        }),
        "export-tables",
    )
}



/// [#3346] `fields --json` 과 `batch fields` 가 공유하는 봉투.
pub(crate) fn fields_json_value(file_path: &str, fields: &[serde_json::Value]) -> serde_json::Value {
    let names: Vec<String> = fields
        .iter()
        .filter_map(|f| f["name"].as_str().map(String::from))
        .collect();
    provenance::marked(
        serde_json::json!({
            "schemaVersion": ENVELOPE_SCHEMA_VERSION,
            "source": file_path,
            "fieldCount": fields.len(),
            "fields": fields,
            "textSecurity": text_security_value(&names),
        }),
        "fields",
    )
}



/// 누름틀 이름 축의 유니코드 기만 판정 봉투.
///
/// 봉투에 담기는 이름은 **공격자가 내용을 정할 수 있는 문서**에서 온다. 에이전트는
/// 그 이름으로 "이 칸을 채워라"를 지목하므로, 화면상 같지만 바이트가 다른 이름 쌍이
/// 있으면 엉뚱한 칸이 채워지고도 `filledCount` 는 성공을 보고한다(#3707).
///
/// 판정만 하고 이름을 고치지 않는다 — 문서 엔진이 사용자 문자열을 조용히 바꾸는 것은
/// 어떤 보안 이득으로도 정당화되지 않는다. `status` 는 `clean`/`warning` 2단이고,
/// 항상 실려 나간다: 필드가 없으면 `clean`, 옛 바이너리면 키 자체가 없다 —
/// 소비자가 "검사했는데 깨끗함"과 "검사하지 않음"을 구별할 수 있어야 한다.
pub(crate) fn text_security_value(names: &[String]) -> serde_json::Value {
    use rhwp::document_core::text_security as ts;

    let mut findings: Vec<serde_json::Value> = Vec::new();

    // ① 화면상 같은 이름 쌍 — 실제 공격 서명이다.
    for (_, group) in ts::confusable_collisions(names) {
        findings.push(serde_json::json!({
            "kind": "confusableFieldName",
            "scope": "fieldName",
            "names": group,
            "note": "이름이 화면상 구별되지 않는 누름틀이 둘 이상입니다 — 이름으로 지목해 채우면 의도와 다른 칸이 채워질 수 있습니다. occurrence 대신 hwp_fields 가 돌려준 바이트를 그대로 쓰거나, 사람 확인을 거치세요.",
        }));
    }

    // ② 이름 하나하나의 혼합 스크립트·보이지 않는 문자.
    for name in names {
        for risk in ts::scan_identifier(name) {
            findings.push(serde_json::json!({
                "kind": risk.kind.label(),
                "scope": "fieldName",
                "names": [name],
                "codepoints": risk.codepoints.iter().map(|c| ts::format_codepoint(*c))
                    .collect::<Vec<_>>(),
                "note": risk.kind.describe(),
            }));
        }
    }

    if findings.is_empty() {
        return serde_json::json!({ "status": "clean" });
    }
    serde_json::json!({
        "status": "warning",
        "findingCount": findings.len(),
        "findings": findings,
    })
}



/// [#3346] `search --json` 과 `batch search` 가 공유하는 봉투.
pub(crate) fn search_json_value(
    file_path: &str,
    query: &str,
    case_sensitive: bool,
    matches: &[rhwp::document_core::queries::grep::GrepMatch],
    total_match_count: usize,
) -> serde_json::Value {
    provenance::marked(
        serde_json::json!({
        "schemaVersion": ENVELOPE_SCHEMA_VERSION,
        "source": file_path,
        "query": query,
        "caseSensitive": case_sensitive,
        "matchCount": matches.len(),
        "totalMatchCount": total_match_count,
        "truncated": matches.len() < total_match_count,
        // [#3787 S7] 절단 축의 어휘를 텍스트 축(`export-text --max-chars`)과 맞춘다.
        // `totalMatchCount - matchCount` 로 유도할 수 있는 값이지만, 유도를 요구하면
        // "전부 봤다"는 오독이 그대로 남는다 — 생략량은 명시가 계약이다.
        "omittedCount": total_match_count.saturating_sub(matches.len()),
        "matches": matches,
        }),
        "search",
    )
}



/// [#3787 S7] 페이지 텍스트 산출의 문자 예산 절단 — CLI `export-text --json` 과
/// MCP `hwp_doc_text` 가 같은 규칙을 공유한다.
///
/// **조용히 자르지 않는다.** 거대 문서가 에이전트 컨텍스트를 밀어내는 것을 막는 게
/// 목적이지만, 잘랐다는 사실을 숨기면 그 절단이 "전부 읽었다"는 거짓말이 된다.
/// 그래서 두 가지를 지킨다.
///
/// 1. **쪽 주소를 보존한다** — 예산이 떨어져도 `pages[]` 에서 항목을 빼지 않는다.
///    빼면 `pageCount` 가 줄어 문서가 실제보다 짧아 보인다.
/// 2. **생략량을 남긴다** — 잘린 페이지마다 `truncated:true`·`omittedCount`(생략된
///    문자 수)를 싣고, 봉투 최상위에 합계를 싣는다. 최상위 `truncated` 는 절단이
///    없어도 항상 나가고(false), 페이지 항목의 두 필드는 잘린 페이지에만 붙는다.
///
/// `max_chars` 가 `None` 이면 무제한이다(기본값 — 종전 동작 무변경).
pub(crate) fn truncate_page_texts(
    pages: &[(u32, String)],
    max_chars: Option<usize>,
) -> (Vec<serde_json::Value>, usize) {
    let mut objs = Vec::with_capacity(pages.len());
    let mut budget = max_chars;
    let mut omitted_total = 0usize;
    for (page, text) in pages {
        let total = text.chars().count();
        let keep = match budget {
            Some(remaining) => remaining.min(total),
            None => total,
        };
        if let Some(remaining) = budget.as_mut() {
            *remaining -= keep;
        }
        let omitted = total - keep;
        omitted_total += omitted;
        let kept: String = if omitted == 0 {
            text.clone()
        } else {
            text.chars().take(keep).collect()
        };
        let mut obj = serde_json::json!({ "page": page, "text": kept });
        if omitted > 0 {
            obj["truncated"] = serde_json::json!(true);
            obj["omittedCount"] = serde_json::json!(omitted);
        }
        objs.push(obj);
    }
    (objs, omitted_total)
}



/// [#3407] 문서 제목 best-effort 추출 — 대량 아카이브 1-pass 대장화용.
///
/// 렌더된 페이지 텍스트(`extract_page_text_native`, `export-text --json` 과 같은
/// 원천)의 첫 의미 줄(trim 후 비어있지 않은 첫 줄)을 돌려준다. 종전 2-pass
/// 대장화(`batch info` + 문서별 `export-text` 첫 줄 파싱)가 소비자 쪽에서 하던
/// 규칙을 엔진이 한 번만 정의한다. 표지가 이미지라 첫 쪽 텍스트가 비면 다음
/// 쪽으로 내려가며(앞 `TITLE_SCAN_PAGES` 쪽까지), 그래도 없으면 `None`(JSON
/// null)이다. 값 자체는 계약이 아닌 best-effort 필드이고, 추출 실패도 문서
/// 메타 조회를 막지 않도록 조용히 다음 쪽으로 넘어간다.
pub(crate) fn document_title(doc: &rhwp::wasm_api::HwpDocument) -> Option<String> {
    for page in 0..doc.page_count().min(TITLE_SCAN_PAGES) {
        let Ok(text) = doc.extract_page_text_native(page) else {
            continue;
        };
        if let Some(line) = text.lines().map(str::trim).find(|l| !l.is_empty()) {
            return Some(line.to_string());
        }
    }
    None
}



/// [#3237] `info --json`·`batch info --json` 이 공유하는 문서 메타 JSON 레코드.
/// `schemaVersion` 이 계약이며 필드 추가는 허용, 변경·삭제는 계약 테스트가 잡는다.
pub(crate) fn info_json_value(
    file_path: &str,
    file_size: usize,
    detected_format: rhwp::parser::FileFormat,
    doc: &rhwp::wasm_api::HwpDocument,
) -> serde_json::Value {
    let document = doc.document();
    let format_str = match detected_format {
        rhwp::parser::FileFormat::Hwp => "hwp5",
        rhwp::parser::FileFormat::Hwpx => "hwpx",
        rhwp::parser::FileFormat::Hwp3 => "hwp3",
        rhwp::parser::FileFormat::Hml => "hml",
        // 파싱이 성공한 뒤에는 도달하지 않지만, 계약상 문자열은 고정해 둔다.
        rhwp::parser::FileFormat::DrmProtected => "drm-protected",
        rhwp::parser::FileFormat::Empty => "empty",
        rhwp::parser::FileFormat::Unknown => "unknown",
    };
    let version = if detected_format == rhwp::parser::FileFormat::Hml {
        serde_json::Value::Null
    } else {
        serde_json::Value::String(format!(
            "{}.{}.{}.{}",
            document.header.version.major,
            document.header.version.minor,
            document.header.version.build,
            document.header.version.revision,
        ))
    };
    let fonts: Vec<String> = document
        .doc_info
        .font_faces
        .first()
        .map(|faces| faces.iter().map(|f| f.name.clone()).collect())
        .unwrap_or_default();
    let para_count: usize = document.sections.iter().map(|s| s.paragraphs.len()).sum();
    // [#5932] HWPX만 채워짐 — HWP5/HWP3/HML은 항상 null(상응 스트림 미조사).
    let (last_saved_application, last_saved_application_version) =
        match hwpx_last_saved_application(document) {
            Some((application, app_version)) => (
                serde_json::Value::String(application),
                serde_json::Value::String(app_version),
            ),
            None => (serde_json::Value::Null, serde_json::Value::Null),
        };
    provenance::marked(
        serde_json::json!({
            "schemaVersion": ENVELOPE_SCHEMA_VERSION,
            "source": file_path,
            "format": format_str,
            "sizeBytes": file_size,
            "version": version,
            "sections": document.sections.len(),
            "pageCount": doc.page_count(),
            "paraCount": para_count,
            "fonts": fonts,
            // [#3407] best-effort 문서 제목 — 없으면 null. batch info 로 자동 전파.
            "title": document_title(doc),
            // [#5932] 마지막 저장 한컴오피스 애플리케이션/버전(HWPX version.xml 보존값).
            "lastSavedApplication": last_saved_application,
            "lastSavedApplicationVersion": last_saved_application_version,
            // [#3880 T1] 파싱 중 건너뛴 것을 봉투가 스스로 밝힌다.
            //
            // 인간 출력은 `warnings: N` 과 상세를 stderr 로 내는데 JSON 분기는 그
            // 앞에서 `return EXIT_OK` 로 끝나 도달하지 못했다. 그래서 리소스가 조용히
            // 잘린 문서가 **exit 0 + 완전해 보이는 봉투**를 냈다 — `fonts` 가 부분
            // 목록인데 봉투는 그렇다고 말하지 않았다(#3719 "부분 목록 금지" 위반).
            //
            // 경고가 없으면 빈 배열이다. 키를 빼면 소비자가 "경고 없음"과 "이 빌드는
            // 경고를 모름"을 구별할 수 없다.
            "warnings": info_warnings_value(doc),
        }),
        "info",
    )
}



/// [#3880 T1] `info --json` 의 `warnings[]` — 파싱이 건너뛴 것의 기계 판정용.
///
/// 현재 원천은 HML 파서의 `hml_metadata().warnings` 하나다. 다른 포맷이 같은 기구를
/// 갖추면 여기에 합류시킨다 — 그때까지 이 배열이 비어 있다고 해서 "문서가 온전하다"는
/// 뜻은 아니며, 그 한계는 `mydocs/manual/cli_commands.md` 에 적는다.
pub(crate) fn info_warnings_value(doc: &rhwp::wasm_api::HwpDocument) -> serde_json::Value {
    let Some(metadata) = doc.hml_metadata() else {
        return serde_json::Value::Array(Vec::new());
    };
    serde_json::Value::Array(
        metadata
            .warnings
            .iter()
            .map(|w| {
                serde_json::json!({
                    "code": format!("{:?}", w.code),
                    "xmlPath": w.xml_path,
                    "message": w.message,
                })
            })
            .collect(),
    )
}



/// [#3633 후속] `--pages a..b` 범위 파서 — 0 기준 양끝 포함, `a<=b` 만 유효.
/// 형식이 어긋나면 None(사용법 오류 처리).
pub(crate) fn parse_digest_pages(s: &str) -> Option<(u32, u32)> {
    let (a, b) = s.split_once("..")?;
    let from = a.parse::<u32>().ok()?;
    let to = b.parse::<u32>().ok()?;
    if from <= to {
        Some((from, to))
    } else {
        None
    }
}



/// [#3633] `digest` — 초소형 모델용 매크로 도구 축 1호.
///
/// 도구 체이닝을 못 하는 모델(4B급)을 위해 "info 로 훑고 → export-structure 로
/// 개요를 얻고 → export-text 로 첫 장을 읽는" 3단 파이프라인을 한 번 호출로
/// 결정론적으로 수행한다. 새 로직 없이 기존 원천만 재사용한다:
/// `load_document` → `info_json_value` 의 필드 + `build_structure` 상위 노드 제목 +
/// `extract_page_text_native` 발췌(`--max-chars` 문자 절단).
///
/// 출력은 항상 봉투 한 줄 JSON 이다(기계 전용 명령 — 표면 규약 통일을 위해
/// `--json` 플래그는 받아만 둔다). 실패 시 stdout 은 0바이트.
pub(crate) fn digest_document(args: &[String]) -> i32 {
    use rhwp::document_core::queries::structure::{build_structure, StructureMode};

    let mut file_path: Option<&str> = None;
    let mut max_chars: Option<usize> = None;
    let mut sections_mode = false;
    let mut pages_range: Option<(u32, u32)> = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--json" => {}
            "--sections" => sections_mode = true,
            "--pages" => {
                i += 1;
                match args.get(i).and_then(|v| parse_digest_pages(v)) {
                    Some(r) => pages_range = Some(r),
                    None => {
                        eprintln!("오류: --pages 뒤에 a..b 형식(0 기준, a<=b)이 필요합니다.");
                        return EXIT_USAGE;
                    }
                }
            }
            "--max-chars" => {
                i += 1;
                match args.get(i).and_then(|v| v.parse::<usize>().ok()) {
                    Some(n) if n > 0 => max_chars = Some(n),
                    _ => {
                        eprintln!("오류: --max-chars 뒤에 1 이상의 숫자가 필요합니다.");
                        return EXIT_USAGE;
                    }
                }
            }
            other if other.starts_with('-') => {
                eprintln!("알 수 없는 옵션: {other}");
                return EXIT_USAGE;
            }
            other => {
                if file_path.replace(other).is_some() {
                    eprintln!("오류: 입력 파일은 하나만 지정할 수 있습니다: {other}");
                    return EXIT_USAGE;
                }
            }
        }
        i += 1;
    }
    if sections_mode && pages_range.is_some() {
        eprintln!("오류: --sections 와 --pages 는 동시에 쓸 수 없습니다.");
        return EXIT_USAGE;
    }
    let Some(file_path) = file_path else {
        eprintln!(
            "사용법: rhwp digest <파일> [--sections | --pages a..b] [--max-chars N] [--json]"
        );
        return EXIT_USAGE;
    };

    let data = match fs::read(file_path) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("오류: 파일을 읽을 수 없습니다 - {}: {}", file_path, e);
            return EXIT_RUNTIME;
        }
    };
    let file_size = data.len();
    let detected_format = rhwp::parser::detect_format(&data);
    let doc = match load_document(&data) {
        Ok(d) => d,
        Err(e) => return e.report(),
    };

    // 메타는 info --json 과 같은 원천(info_json_value)에서 뽑는다 — 어휘 동형 보장.
    let info = info_json_value(file_path, file_size, detected_format, &doc);
    let page_count = doc.page_count();

    // 문자 수 기준 절단 (char 경계 안전). 발췌보다 짧으면 truncated 로 판정을 남긴다.
    let cut = |src: String, cap: usize| -> (String, bool) {
        if src.chars().count() > cap {
            (src.chars().take(cap).collect(), true)
        } else {
            (src, false)
        }
    };

    // ── [#3633 후속] sections 모드: 주소 보존 절 단위 청킹 ──────────────────
    // 페이지 발췌 대신 build_structure 의 최상위 노드를 청크로 낸다. 각 청크는
    // {title,page,charCount,excerpt} — page 는 제목 문단의 글로벌 쪽 번호(기존
    // get_page_of_position_native 재사용)라 요약 결과가 원문 쪽으로 되짚어진다.
    // charCount(절 전체) vs excerpt 길이로 소비자가 잔여량을 판정한다.
    if sections_mode {
        use rhwp::document_core::queries::structure::StructureNode;

        let cap = max_chars.unwrap_or(DIGEST_SECTION_EXCERPT_CHARS);
        let st = build_structure(doc.document(), StructureMode::Auto);

        // 절 본문 수집: 자기 body + 자식 제목·본문 전부 (하위 트리가 절의 내용이다).
        fn collect_section_text(node: &StructureNode, out: &mut String) {
            for line in &node.body {
                if !out.is_empty() {
                    out.push('\n');
                }
                out.push_str(line);
            }
            for child in &node.children {
                if !out.is_empty() {
                    out.push('\n');
                }
                out.push_str(&child.heading);
                collect_section_text(child, out);
            }
        }

        let mut sections = Vec::new();
        let mut any_truncated = false;
        let (sections_mode_label, section_count): (&str, usize) = if st.roots.is_empty() {
            // 구조가 없는 문서: 쪽 단위 폴백으로 강등하되 sectionsMode 로 강등 사실을
            // 명시한다 — 쪽 번호는 그 자체로 주소라 인용 계약은 유지된다.
            for p in 0..page_count.min(DIGEST_SECTIONS_LIMIT as u32) {
                let text = match doc.extract_page_text_native(p) {
                    Ok(t) => t,
                    Err(e) => {
                        eprintln!("오류: 페이지 {} 텍스트 추출 실패 - {:?}", p, e);
                        return EXIT_RUNTIME;
                    }
                };
                let char_count = text.chars().count();
                let (excerpt, truncated) = cut(text, cap);
                any_truncated |= truncated;
                sections.push(serde_json::json!({
                    "title": "",
                    "page": p,
                    "charCount": char_count,
                    "excerpt": excerpt,
                }));
            }
            ("page", page_count as usize)
        } else {
            for node in st.roots.iter().take(DIGEST_SECTIONS_LIMIT) {
                // 제목 문단의 글로벌 쪽 번호 — 기존 위치→쪽 질의를 그대로 재사용한다.
                let page = match doc.get_page_of_position_native(node.section, node.paragraph) {
                    Ok(raw) => serde_json::from_str::<serde_json::Value>(&raw)
                        .ok()
                        .and_then(|v| v["page"].as_u64())
                        .unwrap_or(0),
                    Err(e) => {
                        eprintln!(
                            "오류: 절 '{}' 쪽 번호 조회 실패 - {:?}",
                            node.heading.trim(),
                            e
                        );
                        return EXIT_RUNTIME;
                    }
                };
                let mut text = String::new();
                collect_section_text(node, &mut text);
                let char_count = text.chars().count();
                let (excerpt, truncated) = cut(text, cap);
                any_truncated |= truncated;
                sections.push(serde_json::json!({
                    "title": node.heading.trim(),
                    "page": page,
                    "charCount": char_count,
                    "excerpt": excerpt,
                }));
            }
            (st.mode, st.roots.len())
        };

        let truncated = any_truncated || section_count > sections.len();
        let envelope = serde_json::json!({
            "schemaVersion": ENVELOPE_SCHEMA_VERSION,
            "source": file_path,
            "format": info["format"],
            "pageCount": info["pageCount"],
            "paraCount": info["paraCount"],
            "sectionsMode": sections_mode_label,
            "sectionCount": section_count,
            "sections": sections,
            "truncated": truncated,
            "nextStep": DIGEST_SECTIONS_NEXT_STEP,
        });
        println!("{}", provenance::marked(envelope, "digest"));
        return EXIT_OK;
    }

    // ── [#3633 후속] pages 모드: 범위 지정 발췌 (대형 문서 분할 요약용) ─────
    // nextStep 이 같은 폭의 다음 창을 그대로 받아 적게 안내한다 — 체이닝을 못 하는
    // 모델도 "이어 읽기"를 계획 없이 수행할 수 있다.
    if let Some((from, to)) = pages_range {
        if from >= page_count {
            eprintln!(
                "오류: 시작 쪽 {} 이 문서 범위(0..{}) 밖입니다.",
                from,
                page_count.saturating_sub(1)
            );
            return EXIT_RUNTIME;
        }
        let to = to.min(page_count - 1);
        let mut excerpt_src = String::new();
        for p in from..=to {
            match doc.extract_page_text_native(p) {
                Ok(text) => {
                    if !excerpt_src.is_empty() {
                        excerpt_src.push('\n');
                    }
                    excerpt_src.push_str(&text);
                }
                Err(e) => {
                    eprintln!("오류: 페이지 {} 텍스트 추출 실패 - {:?}", p, e);
                    return EXIT_RUNTIME;
                }
            }
        }
        let (excerpt, truncated) = cut(excerpt_src, max_chars.unwrap_or(DIGEST_DEFAULT_MAX_CHARS));
        let next_step = if to + 1 < page_count {
            let next_from = to + 1;
            let next_to = (next_from + (to - from)).min(page_count - 1);
            format!("이어서 digest --json --pages {next_from}..{next_to}")
        } else {
            DIGEST_PAGES_DONE_NEXT_STEP.to_string()
        };
        let envelope = serde_json::json!({
            "schemaVersion": ENVELOPE_SCHEMA_VERSION,
            "source": file_path,
            "format": info["format"],
            "pageCount": info["pageCount"],
            "paraCount": info["paraCount"],
            "pages": { "from": from, "to": to },
            "excerpt": excerpt,
            "truncated": truncated,
            "nextStep": next_step,
        });
        println!("{}", provenance::marked(envelope, "digest"));
        return EXIT_OK;
    }

    // ── 기본(v1) 모드 — #3633 봉투 무회귀 ───────────────────────────────────
    // 구조 최상위 노드 제목만 싣는다 — 트리 전체는 export-structure 의 몫이다.
    let st = build_structure(doc.document(), StructureMode::Auto);
    let outline: Vec<&str> = st
        .roots
        .iter()
        .take(DIGEST_OUTLINE_LIMIT)
        .map(|n| n.heading.as_str())
        .collect();

    // 앞쪽 페이지 텍스트 발췌 → max_chars 문자에서 절단 (char 경계 안전).
    let mut excerpt_src = String::new();
    for p in 0..page_count.min(DIGEST_EXCERPT_PAGES) {
        match doc.extract_page_text_native(p) {
            Ok(text) => {
                if !excerpt_src.is_empty() {
                    excerpt_src.push('\n');
                }
                excerpt_src.push_str(&text);
            }
            Err(e) => {
                eprintln!("오류: 페이지 {} 텍스트 추출 실패 - {:?}", p, e);
                return EXIT_RUNTIME;
            }
        }
    }
    let (excerpt, truncated) = cut(excerpt_src, max_chars.unwrap_or(DIGEST_DEFAULT_MAX_CHARS));

    let envelope = serde_json::json!({
        "schemaVersion": ENVELOPE_SCHEMA_VERSION,
        "source": file_path,
        "format": info["format"],
        "pageCount": info["pageCount"],
        "paraCount": info["paraCount"],
        "outline": outline,
        "excerpt": excerpt,
        "truncated": truncated,
        "nextStep": DIGEST_NEXT_STEP,
    });
    println!("{}", provenance::marked(envelope, "digest"));
    EXIT_OK
}
