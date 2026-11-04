//! `rhwp serve` — 프로세스 하나로 CLI 명령을 반복 실행하는 워커 (프로토콜 v1).
//!
//! 프로세스 기동 비용(특히 Windows)을 없애려고, 호출자가 1회 실행 CLI 에 넘기던 argv 를
//! 그대로 받아 **같은 명령 함수**를 실행한다. 렌더 상태는 요청 사이에 남기지 않는다.
//!
//! - stdin: 요청 한 줄에 하나 `{"id":<정수>,"argv":[<문자열>...]}` (argv 에 바이너리 경로 제외)
//! - stdout: 첫 줄 `{"rhwpServe":1,"version":"<버전>"}`, 이후 요청마다 정확히 한 줄
//!   `{"id":<같은 값>,"exit":<코드>,"stdout":"...","stderr":"..."}`
//! - 잘못된 요청(JSON 오류·필드 누락·1 MiB 초과 줄)은 `"id":null`, `"exit":2` 로 답하고 계속한다.
//! - 허용 목록 밖 명령·금지 인자는 실행하지 않고 `"exit":2`.
//! - 명령 안 panic 은 `"exit":101` 로 답한 뒤 워커가 101 로 종료한다.
//! - stdin EOF 는 정상 종료(0).
//!
//! 명령 출력(`println!`/`eprintln!`)은 요청마다 OS 수준에서 fd 1/2(Unix) 또는
//! 표준 핸들(Windows, std 가 쓸 때마다 `GetStdHandle` 로 조회)을 파이프로 돌려 받아낸다.
//! 워커 자신의 실제 stdout 에는 프로토콜 줄만, 실제 stderr 에는 워커 진단만 나간다.

use super::*;
use std::io::{self, BufRead, Read, Write};

/// 요청 한 줄의 최대 바이트 수(줄바꿈 제외). 넘으면 파싱하지 않고 거부한다.
const MAX_REQUEST_LINE: usize = 1 << 20;

/// 허용 명령에도 금지하는 인자 — stdin 을 읽거나(`-`, `*-stdin`), `--verify` 처럼 별도 종료
/// 경로를 타거나, 비밀번호 pre-scan(serve 에는 없음)이 필요한 것들. `=값` 형태도 막는다.
const DENIED_ARGS: &[&str] = &[
    "--password",
    "--password-stdin",
    "--output-password",
    "--output-password-stdin",
    "--verify",
];

pub(crate) fn run(args: &[String]) -> i32 {
    if !args.is_empty() {
        eprintln!("사용법: rhwp serve (인자 없음)");
        return EXIT_USAGE;
    }
    let reply = |value: serde_json::Value| -> io::Result<()> {
        let mut out = io::stdout().lock();
        writeln!(out, "{value}")?;
        out.flush()
    };
    let handshake = serde_json::json!({"rhwpServe": 1, "version": env!("CARGO_PKG_VERSION")});
    if let Err(e) = reply(handshake) {
        eprintln!("rhwp serve: stdout 쓰기 실패 - {e}");
        return EXIT_RUNTIME;
    }

    // 가로채기 전의 stdout/stderr — 요청마다 여기로 되돌린다.
    let original = match os::Original::current() {
        Ok(original) => original,
        Err(e) => {
            eprintln!("rhwp serve: 표준 출력 핸들 복제 실패 - {e}");
            return EXIT_RUNTIME;
        }
    };
    let mut input = io::stdin().lock();
    loop {
        let line = match read_request_line(&mut input) {
            Ok(Some(line)) => line,
            Ok(None) => return EXIT_OK,
            Err(e) => {
                eprintln!("rhwp serve: stdin 읽기 실패 - {e}");
                return EXIT_RUNTIME;
            }
        };
        let (response, panicked) = match line.map_err(String::from).and_then(|l| parse_request(&l))
        {
            Err(reason) => (
                response(serde_json::Value::Null, EXIT_USAGE, "", &reason),
                false,
            ),
            Ok((id, argv)) => match denial(&argv) {
                Some(reason) => (response(id, EXIT_USAGE, "", &reason), false),
                None => serve_one(&original, id, &argv),
            },
        };
        if let Err(e) = reply(response) {
            eprintln!("rhwp serve: stdout 쓰기 실패 - {e}");
            return EXIT_RUNTIME;
        }
        if panicked {
            return 101;
        }
    }
}

fn response(id: serde_json::Value, exit: i32, stdout: &str, stderr: &str) -> serde_json::Value {
    serde_json::json!({"id": id, "exit": exit, "stdout": stdout, "stderr": stderr})
}

/// 다음 요청 줄. EOF 면 `None`, 1 MiB 를 넘는 줄은 줄 끝까지 버리고 `Some(Err(..))`.
fn read_request_line(
    input: &mut impl BufRead,
) -> io::Result<Option<Result<Vec<u8>, &'static str>>> {
    let mut line = Vec::new();
    // 줄바꿈 1바이트까지 포함해 상한+1 바이트만 읽는다.
    if input
        .by_ref()
        .take(MAX_REQUEST_LINE as u64 + 1)
        .read_until(b'\n', &mut line)?
        == 0
    {
        return Ok(None);
    }
    if line.last() == Some(&b'\n') {
        line.pop();
        if line.last() == Some(&b'\r') {
            line.pop();
        }
    } else if line.len() > MAX_REQUEST_LINE {
        loop {
            let chunk = input.fill_buf()?;
            if chunk.is_empty() {
                break;
            }
            match chunk.iter().position(|&b| b == b'\n') {
                Some(pos) => {
                    input.consume(pos + 1);
                    break;
                }
                None => {
                    let len = chunk.len();
                    input.consume(len);
                }
            }
        }
        return Ok(Some(Err("요청 줄이 1 MiB 를 넘습니다")));
    }
    Ok(Some(Ok(line)))
}

fn parse_request(line: &[u8]) -> Result<(serde_json::Value, Vec<String>), String> {
    let value: serde_json::Value =
        serde_json::from_slice(line).map_err(|e| format!("요청 JSON 파싱 실패: {e}"))?;
    let id = match value.get("id") {
        Some(id @ serde_json::Value::Number(n)) if n.is_i64() || n.is_u64() => id.clone(),
        _ => return Err("요청에 정수 id 가 없습니다".to_string()),
    };
    let argv = value
        .get("argv")
        .and_then(serde_json::Value::as_array)
        .and_then(|items| {
            items
                .iter()
                .map(|v| v.as_str().map(str::to_string))
                .collect::<Option<Vec<_>>>()
        })
        .ok_or_else(|| "요청의 argv 는 문자열 배열이어야 합니다".to_string())?;
    Ok((id, argv))
}

/// 실행하지 않을 argv 면 그 사유.
fn denial(argv: &[String]) -> Option<String> {
    let allowed = match argv.first().map(String::as_str) {
        Some("fields" | "export-pdf" | "export-svg" | "export-render-tree") => true,
        Some("edit") => argv.get(1).map(String::as_str) == Some("fill-fields"),
        #[cfg(debug_assertions)]
        Some("__panic") => true,
        _ => false,
    };
    if !allowed {
        return Some(
            "serve 허용 명령이 아닙니다 (fields, edit fill-fields, export-pdf, export-svg, export-render-tree)"
                .to_string(),
        );
    }
    argv.iter().skip(1).find_map(|arg| {
        let name = arg.split('=').next().unwrap_or(arg);
        (arg == "-" || DENIED_ARGS.contains(&name))
            .then(|| format!("serve 에서 쓸 수 없는 인자입니다: {name}"))
    })
}

fn dispatch(argv: &[String]) -> i32 {
    let rest = &argv[1..];
    match argv[0].as_str() {
        "fields" => show_fields(rest),
        "edit" => run_edit(rest),
        "export-pdf" => export_pdf(rest),
        "export-svg" => export_svg(rest),
        "export-render-tree" => export_render_tree(rest),
        #[cfg(debug_assertions)]
        "__panic" => panic!("rhwp serve 시험용 panic"),
        _ => EXIT_USAGE,
    }
}

/// 요청 하나를 실행해 응답을 만든다. 두 번째 값은 panic 여부(워커 종료 신호).
fn serve_one(
    original: &os::Original,
    id: serde_json::Value,
    argv: &[String],
) -> (serde_json::Value, bool) {
    // 1회 실행 CLI 와 같은 시작 상태로 되돌린다.
    set_cli_password(None);
    set_cli_output_password(None);
    rhwp::renderer::reset_process_state();

    let captured = capture_output(original, || {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| dispatch(argv)))
    });
    let (result, stdout, stderr) = match captured {
        Ok(captured) => captured,
        Err(e) => {
            eprintln!("rhwp serve: 출력 가로채기 실패 - {e}");
            let reason = format!("rhwp serve: 출력 가로채기 실패 - {e}");
            return (response(id, EXIT_RUNTIME, "", &reason), false);
        }
    };
    let stdout = String::from_utf8_lossy(&stdout);
    let mut stderr = String::from_utf8_lossy(&stderr).into_owned();
    match result {
        Ok(exit) => (response(id, exit, &stdout, &stderr), false),
        Err(payload) => {
            let message = payload
                .downcast_ref::<&str>()
                .map(|s| s.to_string())
                .or_else(|| payload.downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "알 수 없는 panic".to_string());
            stderr.push_str(&format!(
                "rhwp serve: 명령이 panic 으로 끝났습니다 - {message}\n"
            ));
            (response(id, 101, &stdout, &stderr), true)
        }
    }
}

/// `f` 가 실행되는 동안 프로세스 stdout/stderr 를 파이프로 돌려 받아낸다.
/// 파이프마다 읽기 스레드를 둬 출력이 커도 막히지 않는다.
fn capture_output<R>(
    original: &os::Original,
    f: impl FnOnce() -> R,
) -> io::Result<(R, Vec<u8>, Vec<u8>)> {
    let (out_read, out_write) = io::pipe()?;
    let (err_read, err_write) = io::pipe()?;
    let drain = |mut reader: io::PipeReader| {
        std::thread::spawn(move || {
            let mut bytes = Vec::new();
            let _ = reader.read_to_end(&mut bytes);
            bytes
        })
    };
    let (out_thread, err_thread) = (drain(out_read), drain(err_read));

    let redirected = os::point_stdio(&out_write, &err_write);
    let result = redirected.as_ref().ok().map(|_| f());
    let _ = io::stdout().flush();
    let _ = io::stderr().flush();
    let restored = os::point_stdio(&original.stdout, &original.stderr);
    // 쓰기 끝을 닫아야 읽기 스레드가 EOF 를 본다.
    drop((out_write, err_write));
    let stdout = out_thread.join().unwrap_or_default();
    let stderr = err_thread.join().unwrap_or_default();
    redirected?;
    restored?;
    Ok((result.expect("redirected"), stdout, stderr))
}

#[cfg(unix)]
mod os {
    use std::io;
    use std::os::fd::{AsFd, AsRawFd, OwnedFd};

    /// 가로채기 전의 fd 1/2 복제본.
    pub(super) struct Original {
        pub(super) stdout: OwnedFd,
        pub(super) stderr: OwnedFd,
    }

    impl Original {
        pub(super) fn current() -> io::Result<Self> {
            Ok(Self {
                stdout: io::stdout().as_fd().try_clone_to_owned()?,
                stderr: io::stderr().as_fd().try_clone_to_owned()?,
            })
        }
    }

    /// fd 1/2 가 주어진 파일을 가리키게 한다.
    pub(super) fn point_stdio(stdout: &impl AsRawFd, stderr: &impl AsRawFd) -> io::Result<()> {
        for (source, target) in [(stdout.as_raw_fd(), 1), (stderr.as_raw_fd(), 2)] {
            // SAFETY: 둘 다 열린 fd 이고, dup2 는 target 을 원자적으로 교체한다.
            if unsafe { libc::dup2(source, target) } < 0 {
                return Err(io::Error::last_os_error());
            }
        }
        Ok(())
    }
}

#[cfg(windows)]
mod os {
    use std::io;
    use std::os::windows::io::{AsRawHandle, BorrowedHandle};
    use windows_sys::Win32::System::Console::{SetStdHandle, STD_ERROR_HANDLE, STD_OUTPUT_HANDLE};

    /// 가로채기 전의 표준 핸들. 프로세스 소유라 닫지 않는다.
    pub(super) struct Original {
        pub(super) stdout: BorrowedHandle<'static>,
        pub(super) stderr: BorrowedHandle<'static>,
    }

    impl Original {
        pub(super) fn current() -> io::Result<Self> {
            // SAFETY: 표준 핸들은 프로세스가 끝날 때까지 닫지 않는다.
            unsafe {
                Ok(Self {
                    stdout: BorrowedHandle::borrow_raw(io::stdout().as_raw_handle()),
                    stderr: BorrowedHandle::borrow_raw(io::stderr().as_raw_handle()),
                })
            }
        }
    }

    /// std 의 stdout/stderr 는 쓸 때마다 `GetStdHandle` 로 핸들을 얻으므로
    /// `SetStdHandle` 만으로 Rust 출력 전부가 돌아간다.
    pub(super) fn point_stdio(
        stdout: &impl AsRawHandle,
        stderr: &impl AsRawHandle,
    ) -> io::Result<()> {
        for (handle, id) in [
            (stdout.as_raw_handle(), STD_OUTPUT_HANDLE),
            (stderr.as_raw_handle(), STD_ERROR_HANDLE),
        ] {
            // SAFETY: 열린 핸들을 표준 핸들 자리에 둘 뿐이다.
            if unsafe { SetStdHandle(id, handle) } == 0 {
                return Err(io::Error::last_os_error());
            }
        }
        Ok(())
    }
}
