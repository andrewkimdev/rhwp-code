//! Dioxus subsecond 핫패치용 개발 전용 바이너리.
//!
//! [#3890] `rhwp::subsecond_dev` 는 루트 크레이트에서 `subsecond-dev` feature 뒤에
//! 있다(#3255 의 "프로덕션 격리" 설계 — default 미포함이 의도다). 그런데 이 바이너리가
//! feature 와 무관하게 그 모듈을 불러 **기본 feature 로는 컴파일 자체가 불가능했고**,
//! 그 결과 `cargo build/clippy --workspace` 가 상시 실패했다.
//!
//! feature 로 분기해 격리는 유지하면서 워크스페이스 빌드가 통과하게 한다.

#[cfg(feature = "subsecond-dev")]
fn main() {
    rhwp::subsecond_dev::link_wasm_exports();
}

#[cfg(not(feature = "subsecond-dev"))]
fn main() {
    eprintln!(
        "rhwp-subsecond 는 개발용 핫패치 도구입니다.\n\
         `cargo run -p rhwp-subsecond --features subsecond-dev` 로 실행하세요."
    );
    // 사용법 오류 — `cli_commands.md` 의 종료 코드 계약(2 = 사용법 오류)을 따른다.
    std::process::exit(2);
}
