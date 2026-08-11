# PR #4052 검토

## 결론

**메인터너 안전 보정 후 수용 후보.** 원 변경은 placeholder였던 batch-convert를 실제 `rhwp export-*`
호출로 연결했지만, 같은 상대 경로의 `.hwp`와 `.hwpx`가 같은 산출물을 덮어쓸 수 있었고, 활성 포맷 일부의
실패를 성공으로 집계했으며, 기능 부재(exit 2)까지 재시도했다.

메인터너 보정 commit `f36434eb3`은 출력 충돌을 변환 시작 전에 거부하고, 활성 포맷 중 하나라도 실패하면
해당 문서를 Failed와 exit 1로 집계한다. 재시도는 rhwp 런타임 실패(exit 1)로 한정한다. 최종 병합 조건은
이 code head의 full CI 통과, 최신 mergeable 상태, 작업지시자 승인이다.

## 접수 및 기준

| 항목 | 내용 |
| --- | --- |
| PR | [#4052](https://github.com/edwardkim/rhwp/pull/4052) `fix(tools): batch-convert의 placeholder 변환 로직을 실제 rhwp export 호출로 교체` |
| 관련 이슈 | [#4051](https://github.com/edwardkim/rhwp/issues/4051) |
| 작성자 | `kevin9327` |
| 대상 | `devel` |
| contributor source | `kevin9327/rhwp:pr/feature-batch-convert-tool` |
| 보정 시작 source head | `30ff9736d73f342d36dfc3aabf63b60f5080b990` |
| 메인터너 보정 | `f36434eb3b6b91f38a1d145c7700de44770ac4cb` |
| code head 검증 | [CI 31023915543](https://github.com/edwardkim/rhwp/actions/runs/31023915543), [CodeQL 31023922453](https://github.com/edwardkim/rhwp/actions/runs/31023922453), [Render Diff 31023915292](https://github.com/edwardkim/rhwp/actions/runs/31023915292) 성공 |
| 작성 시점 remote 상태 | 보정 commit은 원격 PR source branch에 반영됨; 이 review·오늘할일 commit은 아직 push하지 않음 |
| 작성 시점 권한 | `maintainerCanModify=true` |

```text
base route: collaborator_external_pr.md
modifiers: intake_and_review.md, local_validation.md, rework_and_exceptions.md
loaded documents: pr_review_workflow.md, pr_review/README.md,
                  collaborator_external_pr.md, intake_and_review.md,
                  local_validation.md, rework_and_exceptions.md
```

## 변경 내용

### 기여자 원 변경

- 배치 설정의 PDF, PNG, SVG, text 활성 포맷을 실제 `rhwp export-*` 호출로 연결했다.
- 병렬 실행, 재시도, 실패 원본 수집, 기존 산출물 처리와 구성 파일 계약을 제공했다.

### 메인터너 보정

- 같은 상대 경로·stem의 HWP/HWPX가 PDF 파일과 페이지별 출력 폴더를 공유하는 경우 사전 거부한다.
  기존 비충돌 산출물 이름은 바꾸지 않는다.
- 활성 포맷은 모두 성공해야 파일을 Successful로 집계한다. 일부 성공 산출물은 보존하지만 배치 종료는
  exit 1이다.
- exit 1만 재시도하고 exit 2 및 rhwp spawn 오류는 즉시 실패로 반환한다.
- mock rhwp에 하위 명령별 실패와 종료 코드 주입을 추가해 세 계약을 회귀 테스트로 고정했다.

## 로컬 검증

| 검증 | 결과 |
| --- | --- |
| `CARGO_TARGET_DIR=target/review-kevin9327-4052-20260806 CARGO_INCREMENTAL=0 cargo test -p batch-convert` | 18개 통합 테스트 통과 |
| 실제 `field-01.hwp` + all-formats + native-skia 없는 rhwp | PNG exit 2를 재시도 없이 Failed 1, exit 1로 집계; PDF·SVG·text 산출물은 보존 |
| `same.hwp` + `same.hwpx` + mock rhwp | 충돌 오류와 exit 1; rhwp 호출 0회, output root 미생성 |
| `cargo fmt --check` | 통과 |
| `cargo clippy -p batch-convert --all-targets -- -D warnings` | 통과 |
| `git diff --check` | 통과 |

renderer, document parser, HWP/HWPX 저장 형식, fixture는 변경하지 않았다. 따라서 Canvas/PDF fidelity
시각 검증은 이 CLI 안전 보정의 판정 대상이 아니다.

## 원격 반영 주의

가시성 branch는 실제 contributor source head `30ff9736d`를 직접 부모로 하며, contributor 원 commit을
rewrite하지 않았다. push 직전에 PR head와 contributor remote ref가 이 SHA와 같은지 다시 확인하고,
변경 파일의 LFS 속성 판독 및 적절한 dry-run을 수행한다. code/test 보정이 포함되어 review-only fast-pass는
사용하지 않으며, push된 최신 head의 full CI를 기다린다.

## 최종 권고

메인터너 보정은 원격 반영과 full CI까지 통과했다. 이 review·오늘할일 trailing commit을 동일 source
branch에 push하고 공용 fast-pass aggregate를 확인한 뒤 병합한다.
