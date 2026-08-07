# 최종 보고서 — task_m100_4158

- **Issue**: [#4158](https://github.com/edwardkim/rhwp/issues/4158)
- **브랜치**: `task_m100_4158_char_overlap_boxed_pua`
- **기준**: `upstream/devel` `5119ea498`
- **계획서**: [`mydocs/plans/task_m100_4158.md`](../plans/task_m100_4158.md)
- **단계 기록**: [`mydocs/working/task_m100_4158_stage1.md`](../working/task_m100_4158_stage1.md)
- **작성 시각**: 2026-08-07 KST

## 1. 결과

실제 `CharOverlap`의 `U+F02B1` 사각 숫자가 브라우저 글꼴의 PUA glyph에 의존해 tofu로
출력되던 결함을 고쳤다. IR 원문은 보존하면서 Canvas2D·SVG·Native Skia가 한 공통 규칙으로
사각형과 숫자를 합성한다.

실제 HWP 물리 10쪽의 `공정거래위원회` 앞 표식은 수정 후 정답지와 같은 사각형 안 숫자 1로
출력된다. 문서는 17쪽을 유지하고, #4139가 고정한 물리 2쪽 일반 `TextRun` 경로도 통과했다.

## 2. 수정 계약

```
single CharOverlap U+F02B1..U+F02C4 → number 1..20
raw border 0                            → effective square border 3
explicit border 1..4                    → preserve
IR text / CharOverlapInfo               → preserve
```

다중 문자 PUA 숫자, 표준 Unicode 원문자와 범위 밖 PUA 동작은 바꾸지 않았다.

## 3. 검증 요약

- focused Rust 3건 PASS
- Native Skia feature release-test focused 2건 PASS
- `clippy --lib`, fmt, diff PASS
- release WASM build의 compile·wasm-bindgen·wasm-opt·packaging PASS
- 신규 물리 10쪽 Canvas2D E2E 7개 계약 PASS
- 기존 물리 2쪽 #536 E2E 6개 계약 PASS
- 실제 SVG에서 `<rect>`+숫자 1과 raw PUA 부재 확인
- 17쪽 한컴 PDF 물리 10쪽과 시각 대조 완료

증적은 `output/4158/`에 있다. 전체 PR-CI형 로컬 게이트는 작업지시자 별도 승인 후 실행한다.

## 4. 원격 상태

로컬 단계 커밋까지만 수행한다. GitHub push, PR 생성, #4158 comment·close는 수행하지 않았다.
