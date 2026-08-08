# PR #4072 메인터너 안전 보정 계획

## 목적

기여자 PR #4072의 글꼴 분석기에서 원본 문서 덮어쓰기와 무제한 자원 소비 가능성을 제거하되,
HWP/HWPX 글꼴 분석 계약과 출력 형식은 유지한다.

## 기준

| 항목 | SHA |
| --- | --- |
| contributor 원 head | `5467610fda2a356f86e668b6b372fea38dd6902c` |
| 원 기준선 | `efed25b43a972f79e948f8c28abf863eb18aa1d8` |
| 검토 시점 devel | `32afce965f167cb78a101b26cd3dc64ad9fe7dda` |
| 메인터너 보정 | `23a7c70cee51ac63b4b8e4e590b19392a95c7422` |

## 단계

1. 완료: 원 PR을 최신 `devel` 위 가시성 검토 branch에 재현하고, 단일 파일·디렉터리 회귀 테스트를 실행했다.
2. 완료: 원본과 같은 출력, 기존 출력, 심볼릭 링크 입력, 재귀 대상 수 초과, `rhwp info` 시간 초과를
   회귀 테스트로 추가했다.
3. 완료: `--overwrite`, `--max-files`, `--timeout-seconds`와 안전 계약을 구현·문서화하고,
   실제 HWP 복사본의 SHA-256 보존을 확인했다.
4. 완료: 원격 PR head가 `5467610f`와 같은지 확인하고, contributor commit을 rewrite하지 않도록
   보정 commit만 그 위에 적용했다.
5. 완료: LFS 판독·dry-run·실제 push 후 `23a7c70ce`의 full CI와 CodeQL 성공을 확인했다.
6. 진행: 검토 기록과 오늘할일을 같은 PR head에 추가하고, 최신 aggregate와 작업지시자 승인을 확인한 뒤
   병합·후속 처리를 한다.

## 롤백

- 원격 push 전: 로컬 `review/kevin9327-4072-20260805`의 보정 commit만 폐기하면 된다.
- 원격 push 후 CI 실패: contributor 원 commit은 변경하지 않고 보정 commit을 후속 commit으로 수정한다.
- 원격 source SHA가 달라진 경우: 기존 보정을 push하지 않고 새 source head에서 검토·회귀 테스트를 다시 판정한다.
