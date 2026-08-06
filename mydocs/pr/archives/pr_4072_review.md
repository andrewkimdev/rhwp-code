# PR #4072 검토

## 결론

**메인터너 안전 보정 후 수용 후보.** 원 변경은 HWP/HWPX 컨테이너를 직접 추정하지 않고
`rhwp info --json`의 `fonts[]` 계약만 사용하도록 글꼴 분석기를 재구성했다. 다만 분석 보고서의
출력 경로로 원본 파일을 지정하면 원본을 텍스트로 덮어쓸 수 있었고, 재귀 분석과 외부 프로세스
호출에는 자원 상한이 없었다.

메인터너 보정 commit `23a7c70ce`은 원본·출력 경로 충돌, 심볼릭 링크, 무단 덮어쓰기, 과도한
재귀 대상 수, 멈춘 `rhwp info` 호출을 차단한다. code head의 CI와 CodeQL은 성공했다. 최종 병합
조건은 이 검토 기록을 포함한 최신 PR head의 required check와 작업지시자 승인이다.

## 접수 및 기준

| 항목 | 내용 |
| --- | --- |
| PR | [#4072](https://github.com/edwardkim/rhwp/pull/4072) `feat(tools): font-analyzer — rhwp info --json 계약 재사용으로 재작성` |
| 작성자 | `kevin9327` |
| 대상 | `devel` |
| 원 PR head | `5467610fda2a356f86e668b6b372fea38dd6902c` |
| 원 PR 기준선 | `efed25b43a972f79e948f8c28abf863eb18aa1d8` |
| 검토 시작 시점 devel | `32afce965f167cb78a101b26cd3dc64ad9fe7dda` |
| 메인터너 보정 | `23a7c70cee51ac63b4b8e4e590b19392a95c7422` |
| 원 PR 변경 규모 | 3 files, +533 / -0 |
| 작성 시점 상태 | `MERGEABLE`, `CLEAN`, `maintainerCanModify=true` |
| code head CI | [CI 31004212850](https://github.com/edwardkim/rhwp/actions/runs/31004212850) 성공 |
| code head CodeQL | [CodeQL 31004212493](https://github.com/edwardkim/rhwp/actions/runs/31004212493) 성공 |

```text
base route: collaborator_external_pr.md
modifiers: intake_and_review.md, local_validation.md, rework_and_exceptions.md
loaded documents: pr_review_workflow.md, pr_review/README.md,
                  collaborator_external_pr.md, intake_and_review.md,
                  local_validation.md, rework_and_exceptions.md
```

## 변경 내용

### 기여자 원 변경

- HWP/HWPX를 직접 파싱하지 않고 `rhwp info --json`의 `fonts[]`를 글꼴 정보의 단일 신뢰 소스로 사용한다.
- 단일 파일 조회와 디렉터리 집계, text/JSON/Markdown 출력을 표준 라이브러리만으로 제공한다.
- `--rhwp-bin`, `RHWP_BIN`, `PATH`, 저장소 `target/` 순서로 실행 파일을 탐색한다.

### 메인터너 안전 보정

- `--output`이 입력 원본과 같거나 하드링크로 같은 inode를 가리키면 거부한다.
- 입력 또는 출력의 심볼릭 링크를 거부하고, 디렉터리 탐색 중 심볼릭 링크 문서는 제외한다.
- 기존 출력 파일은 `--overwrite`가 있을 때만 덮어쓴다.
- `--max-files`의 기본 상한을 10,000개(최대 100,000개)로 두고 초과 시 분석을 시작하지 않는다.
- 파일별 `rhwp info` 호출에 기본 120초, 최대 1,800초의 `--timeout-seconds` 상한을 적용한다.

## 로컬 검증

아래 검증은 메인터너 보정 commit `23a7c70ce`에서 완료했다.

| 검증 | 결과 |
| --- | --- |
| `RHWP_BIN=$PWD/target/release-test/rhwp python3 tools/font-analyzer/tests/test_font_analyzer.py` | 12 passed |
| `python3 -m py_compile tools/font-analyzer/font_analyzer.py` | 통과 |
| 실제 `samples/field-01.hwp` 복사본을 같은 `--output` 경로로 지정 | exit 1, SHA-256 불변, HWP 5.0 형식 유지 |
| `python3 tools/font-analyzer/font_analyzer.py --help` | 안전 옵션 3종 노출 확인 |
| `git diff --check` | 통과 |

renderer, layout, HWP/HWPX 파일 포맷, fixture는 변경하지 않았다. 따라서 PDF/Canvas 시각 검증은
이번 안전 보정의 판정 대상이 아니다.

## 원격 적용 주의

원 PR head는 현재 `devel`의 조상이 아닌 이전 기준선에서 갈라졌다. 사용자가 그래프에서 최신
`devel` 위 변경을 볼 수 있게 만든 로컬 검토 branch는 원 변경을 재현한 branch이므로, 이 branch를
그대로 원격 `pr/tool-font-analyzer`에 push하면 contributor 이력을 rewrite하게 된다.

원격 반영 전에 `5467610f`가 fork head와 같은지 재확인했고, 원 source commit을 수정하지 않은 채
메인터너 보정 commit만 그 head 위에 적용했다. 변경 파일 3개는 모두 LFS 비대상이었으며, LFS를
건너뛴 dry-run과 실제 push 후 PR head가 `23a7c70ce`와 일치함을 확인했다.

## 최종 권고

code head의 full CI·CodeQL은 통과했다. 이 검토 기록과 오늘 기록만 담은 trailing commit을 같은 PR
head에 push해 aggregate를 재확인하고, 작업지시자 승인 뒤 PR #4072를 병합한다.
