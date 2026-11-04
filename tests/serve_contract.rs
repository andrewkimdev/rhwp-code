//! `rhwp serve` 프로토콜 v1 계약.
//!
//! 허용 명령은 1회 실행 CLI 와 같은 출력 파일·종료 코드·stdout 을 내야 하고(한 워커에서
//! 폰트 옵션을 바꿔 가며 연속 실행해도), 잘못된 요청·금지 인자·긴 줄·큰 출력·연속 200건·
//! EOF·panic 에서 정해진 대로 답해야 한다.
#![cfg(not(target_arch = "wasm32"))]

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use serde_json::{json, Value};

const SAMPLES: &[&str] = &[
    "samples/누름틀-2024.hwpx",
    "samples/issue1893_clickhere_field_roundtrip.hwpx",
    "samples/test-image.hwpx",
    "samples/표-텍스트.hwpx",
    "samples/hwp3-sample19-hwpx.hwpx",
];

fn rhwp_bin() -> String {
    std::env::var("CARGO_BIN_EXE_rhwp").unwrap_or_else(|_| env!("CARGO_BIN_EXE_rhwp").to_string())
}

fn repo_path(rel: &str) -> String {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(rel)
        .to_string_lossy()
        .into_owned()
}

fn temp_dir(label: &str) -> PathBuf {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock")
        .as_nanos();
    let dir =
        std::env::temp_dir().join(format!("rhwp-serve-{label}-{}-{nonce}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    dir
}

struct Worker {
    child: Child,
    stdin: Option<ChildStdin>,
    stdout: BufReader<ChildStdout>,
}

impl Worker {
    fn start() -> Self {
        let mut child = Command::new(rhwp_bin())
            .arg("serve")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("rhwp serve 실행");
        let stdin = child.stdin.take();
        let stdout = BufReader::new(child.stdout.take().unwrap());
        let mut worker = Self {
            child,
            stdin,
            stdout,
        };
        let handshake = worker.read_reply();
        assert_eq!(
            handshake,
            json!({"rhwpServe": 1, "version": env!("CARGO_PKG_VERSION")})
        );
        worker
    }

    fn send_line(&mut self, line: &[u8]) -> Value {
        let stdin = self.stdin.as_mut().unwrap();
        stdin.write_all(line).unwrap();
        stdin.write_all(b"\n").unwrap();
        stdin.flush().unwrap();
        self.read_reply()
    }

    fn request(&mut self, id: i64, argv: &[String]) -> Value {
        let reply = self.send_line(json!({"id": id, "argv": argv}).to_string().as_bytes());
        assert_eq!(reply["id"], json!(id), "응답 id 불일치: {reply}");
        reply
    }

    fn read_reply(&mut self) -> Value {
        let mut line = String::new();
        self.stdout.read_line(&mut line).expect("응답 읽기");
        assert!(
            line.ends_with('\n'),
            "응답 줄이 끊겼다 (워커 종료?): {line:?}"
        );
        serde_json::from_str(&line).expect("응답 JSON")
    }

    /// stdin 을 닫고 종료 코드와 워커 stderr 를 돌려준다.
    fn finish(mut self) -> (Option<i32>, String) {
        drop(self.stdin.take());
        let output = self.child.wait_with_output().expect("워커 종료 대기");
        (
            output.status.code(),
            String::from_utf8_lossy(&output.stderr).into_owned(),
        )
    }
}

fn args(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| s.to_string()).collect()
}

/// 출력 경로(파일 또는 폴더)의 내용 스냅샷. 비교 뒤 지운다.
fn take_outputs(path: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut files = BTreeMap::new();
    if path.is_dir() {
        for entry in std::fs::read_dir(path).unwrap() {
            let entry = entry.unwrap();
            files.insert(
                entry.file_name().to_string_lossy().into_owned(),
                std::fs::read(entry.path()).unwrap(),
            );
        }
        std::fs::remove_dir_all(path).unwrap();
    } else if path.exists() {
        files.insert(String::new(), std::fs::read(path).unwrap());
        std::fs::remove_file(path).unwrap();
    }
    files
}

#[test]
fn allowlisted_commands_match_one_shot_runs() {
    let dir = temp_dir("parity");
    let fonts = repo_path("ttfs/opensource");
    let packaged = [
        "--font-path",
        fonts.as_str(),
        "--packaged-fonts-only",
        "--fallback-serif",
        "Noto Sans KR",
        "--fallback-sans",
        "Noto Sans KR",
        "--fallback-mono",
        "Noto Sans KR",
    ];
    let mut worker = Worker::start();
    let mut id = 0;
    let mut compared = 0;

    for sample in SAMPLES {
        let input = repo_path(sample);
        let fields = Command::new(rhwp_bin())
            .args(["fields", &input, "--json"])
            .output()
            .unwrap();
        let envelope: Value = serde_json::from_slice(&fields.stdout).expect("fields --json");
        let mut data = serde_json::Map::new();
        for field in envelope["fields"].as_array().into_iter().flatten() {
            if let Some(name) = field["name"].as_str() {
                data.insert(name.to_string(), json!(format!("값-{name}")));
            }
        }
        let data_file = dir.join("data.json");
        std::fs::write(&data_file, Value::Object(data).to_string()).unwrap();
        let data_arg = format!("@{}", data_file.display());

        let filled = dir.join("filled.hwpx");
        let pdf = dir.join("out.pdf");
        let svg_dir = dir.join("svg");
        let tree_dir = dir.join("tree");
        let slash = |p: &Path| format!("{}/", p.display());
        let mut cases: Vec<(Vec<String>, PathBuf)> = vec![
            (args(&["fields", &input, "--json"]), dir.join("none")),
            (args(&["fields", &input]), dir.join("none")),
            (
                args(&[
                    "edit",
                    "fill-fields",
                    &input,
                    "--data",
                    &data_arg,
                    "-o",
                    &filled.to_string_lossy(),
                    "--json",
                ]),
                filled.clone(),
            ),
            (
                args(&["export-pdf", &input, "-o", &pdf.to_string_lossy()]),
                pdf.clone(),
            ),
            (
                args(&["export-render-tree", &input, "-o", &slash(&tree_dir)]),
                tree_dir.clone(),
            ),
            (
                args(&["export-svg", &input, "-o", &slash(&svg_dir)]),
                svg_dir.clone(),
            ),
        ];
        // 같은 워커에서 패키지 폰트 모드를 켰다 끄며 섞는다 — 요청 간 상태가 새면 드러난다.
        let mut with_fonts = args(&["export-pdf", &input, "-o", &pdf.to_string_lossy()]);
        with_fonts.extend(args(&packaged));
        cases.insert(3, (with_fonts, pdf.clone()));
        let mut svg_fonts = args(&["export-svg", &input, "-o", &slash(&svg_dir)]);
        svg_fonts.extend(args(&packaged));
        cases.insert(5, (svg_fonts, svg_dir.clone()));

        for (argv, output) in cases {
            let one_shot = Command::new(rhwp_bin()).args(&argv).output().unwrap();
            let expected_files = take_outputs(&output);
            id += 1;
            let reply = worker.request(id, &argv);
            let actual_files = take_outputs(&output);
            let label = format!("{sample} {}", argv[..2].join(" "));
            assert_eq!(
                reply["exit"],
                json!(one_shot.status.code().unwrap()),
                "{label}: exit\n{reply}"
            );
            assert_eq!(
                reply["stdout"].as_str().unwrap(),
                String::from_utf8_lossy(&one_shot.stdout),
                "{label}: stdout"
            );
            assert_eq!(
                expected_files.keys().collect::<Vec<_>>(),
                actual_files.keys().collect::<Vec<_>>(),
                "{label}: 출력 파일 목록"
            );
            for (name, bytes) in &expected_files {
                assert!(
                    bytes == &actual_files[name],
                    "{label}: 출력 파일 {name} 바이트 불일치"
                );
            }
            compared += 1;
        }
    }
    let (code, stderr) = worker.finish();
    assert_eq!(code, Some(0), "EOF 는 정상 종료여야 한다");
    assert!(
        stderr.is_empty(),
        "명령 출력이 워커 stderr 로 새면 안 된다: {stderr}"
    );
    assert_eq!(compared, SAMPLES.len() * 8);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn denied_argv_is_not_run() {
    let dir = temp_dir("deny");
    let input = repo_path(SAMPLES[0]);
    let out = dir.join("x.out").to_string_lossy().into_owned();
    let mut worker = Worker::start();
    let denied: &[&[&str]] = &[
        &[],
        &["export-hwpx", &input, "-o", &out],
        &["edit", "replace-text", &input, "-o", &out],
        &["edit"],
        &["mcp-serve"],
        &["serve"],
        &["export-pdf", &input, "-o", &out, "--password", "x"],
        &["export-pdf", &input, "-o", &out, "--password=x"],
        &["export-pdf", &input, "-o", &out, "--password-stdin"],
        &["export-pdf", &input, "-o", &out, "--output-password", "x"],
        &["export-pdf", &input, "-o", &out, "--output-password-stdin"],
        &[
            "edit",
            "fill-fields",
            &input,
            "--data",
            "{}",
            "-o",
            &out,
            "--verify",
        ],
        &["export-pdf", "-", "-o", &out],
        #[cfg(not(debug_assertions))]
        &["__panic"],
    ];
    for (i, argv) in denied.iter().enumerate() {
        let reply = worker.request(i as i64, &args(argv));
        assert_eq!(reply["exit"], json!(2), "{argv:?}: {reply}");
        assert_eq!(reply["stdout"], json!(""), "{argv:?}");
        assert!(
            !reply["stderr"].as_str().unwrap().is_empty(),
            "{argv:?}: 사유가 없다"
        );
        assert!(
            !Path::new(&out).exists(),
            "{argv:?}: 거부된 명령이 실행됐다"
        );
    }
    let reply = worker.request(99, &args(&["fields", &input, "--json"]));
    assert_eq!(reply["exit"], json!(0));
    assert_eq!(worker.finish().0, Some(0));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn malformed_and_over_long_lines_get_null_id_and_keep_serving() {
    let input = repo_path(SAMPLES[0]);
    let mut worker = Worker::start();
    let mut long = vec![b'a'; (1 << 20) + 10];
    long[0] = b'{';
    let bad: Vec<Vec<u8>> = vec![
        b"not json".to_vec(),
        b"".to_vec(),
        b"[1,2]".to_vec(),
        br#"{"argv":["fields"]}"#.to_vec(),
        br#"{"id":"1","argv":["fields"]}"#.to_vec(),
        br#"{"id":1.5,"argv":["fields"]}"#.to_vec(),
        br#"{"id":1}"#.to_vec(),
        br#"{"id":1,"argv":"fields"}"#.to_vec(),
        br#"{"id":1,"argv":["fields",1]}"#.to_vec(),
        vec![0xff, 0xfe],
        long,
    ];
    for line in &bad {
        let reply = worker.send_line(line);
        assert_eq!(reply["id"], Value::Null, "{reply}");
        assert_eq!(reply["exit"], json!(2), "{reply}");
        assert!(!reply["stderr"].as_str().unwrap().is_empty());
        let ok = worker.request(7, &args(&["fields", &input, "--json"]));
        assert_eq!(ok["exit"], json!(0), "잘못된 줄 뒤에도 계속 처리해야 한다");
    }
    assert_eq!(worker.finish().0, Some(0));
}

#[test]
fn large_output_does_not_deadlock() {
    let dir = temp_dir("large");
    let input = repo_path("samples/누름틀-2024.hwpx");
    let fields = Command::new(rhwp_bin())
        .args(["fields", &input, "--json"])
        .output()
        .unwrap();
    let envelope: Value = serde_json::from_slice(&fields.stdout).unwrap();
    let name = envelope["fields"][0]["name"]
        .as_str()
        .expect("필드 하나 이상")
        .to_string();
    let big = "가".repeat(100_000); // 300 KB
    let data_file = dir.join("data.json");
    let mut data = serde_json::Map::new();
    data.insert(name, json!(big));
    std::fs::write(&data_file, Value::Object(data).to_string()).unwrap();
    let out = dir.join("filled.hwpx");
    let mut worker = Worker::start();
    let reply = worker.request(
        1,
        &args(&[
            "edit",
            "fill-fields",
            &input,
            "--data",
            &format!("@{}", data_file.display()),
            "-o",
            &out.to_string_lossy(),
            "--json",
        ]),
    );
    assert_eq!(reply["exit"], json!(0), "{}", reply["stderr"]);
    let stdout = reply["stdout"].as_str().unwrap();
    assert!(stdout.len() > 64 * 1024, "stdout {} bytes", stdout.len());
    assert!(stdout.contains(&big));
    assert_eq!(worker.finish().0, Some(0));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn two_hundred_requests_in_a_row() {
    let input = repo_path(SAMPLES[0]);
    let argv = args(&["fields", &input, "--json"]);
    let expected = Command::new(rhwp_bin()).args(&argv).output().unwrap();
    let mut worker = Worker::start();
    #[cfg(target_os = "linux")]
    let fd_count = |pid: u32| {
        std::fs::read_dir(format!("/proc/{pid}/fd"))
            .unwrap()
            .count()
    };
    #[cfg(target_os = "linux")]
    let mut fds_after_warmup = 0;
    for id in 0..200 {
        let reply = worker.request(id, &argv);
        assert_eq!(reply["exit"], json!(0));
        assert_eq!(
            reply["stdout"].as_str().unwrap(),
            String::from_utf8_lossy(&expected.stdout)
        );
        #[cfg(target_os = "linux")]
        if id == 9 {
            fds_after_warmup = fd_count(worker.child.id());
        }
    }
    #[cfg(target_os = "linux")]
    assert_eq!(fd_count(worker.child.id()), fds_after_warmup, "fd 누수");
    assert_eq!(worker.finish().0, Some(0));
}

#[test]
fn extra_serve_arguments_are_a_usage_error() {
    let output = Command::new(rhwp_bin())
        .args(["serve", "--password-stdin"])
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
}

/// panic 시 101 응답 후 워커가 101 로 끝난다. 시험용 `__panic` 은 debug 빌드에만 있다.
#[test]
fn panic_replies_101_and_exits_101() {
    if !cfg!(debug_assertions) {
        return;
    }
    let mut worker = Worker::start();
    let reply = worker.request(5, &args(&["__panic"]));
    assert_eq!(reply["exit"], json!(101), "{reply}");
    assert!(
        reply["stderr"]
            .as_str()
            .unwrap()
            .contains("rhwp serve 시험용 panic"),
        "{reply}"
    );
    assert_eq!(worker.finish().0, Some(101));
}

/// panic 격리는 unwind 에 기댄다 — release 프로필을 abort 로 바꾸면 101 응답이 사라진다.
#[test]
fn release_profile_keeps_unwinding_panics() {
    let manifest = std::fs::read_to_string(repo_path("Cargo.toml")).unwrap();
    assert!(
        !manifest
            .lines()
            .any(|l| l.split('#').next().unwrap().replace(' ', "") == "panic=\"abort\""),
        "rhwp serve 는 panic = \"unwind\" 가 필요하다"
    );
}
