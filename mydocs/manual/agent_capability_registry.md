---
kind: canonical
status: active
canonical: mydocs/manual/agent_capability_registry.md
last_verified: 2026-07-26
---

# 에이전트 capability 카탈로그

이 문서는 Claude와 Codex에서 재사용하는 **프로젝트 capability의 유일한 등록부**다.
파일 하나가 아니라 사용자에게 제공하는 책임 단위로 등록한다. 따라서 같은 capability의 Claude
에이전트와 Codex Skill은 별도 항목이 아니라 하나의 항목에 연결한다.

## 등록부

| ID | 책임과 비범위 | 권위 문서 | Claude 진입점 | Codex 진입점 | 상태·소유 |
| --- | --- | --- | --- | --- | --- |
| `rhwp-cli` | HWP/HWPX 분석·내보내기·진단. 구현 변경·한컴 최종 판정은 책임 밖 | [CLI 명령어 매뉴얼](cli_commands.md) | [Skill](../../.claude/skills/rhwp-cli/SKILL.md) | — | active · rhwp maintainers |
| `rhwp-exam-ingest` | 시험지 자료를 HWPX로 변환. 일반 문서 양식 생성은 책임 밖 | [ingest 명령](cli_commands.md#build-from-ingest) | [Skill](../../.claude/skills/rhwp-exam-ingest/SKILL.md) | — | active · rhwp maintainers |
| `bug-hunter` | 실사례 여정과 정답지 대조로 재현 가능한 결함을 발굴. 수정 구현은 요청 시 별도 작업 | [버그 헌팅 playbook](bug_hunting_playbook.md) | [에이전트](../../.claude/agents/bug-hunter.md) | [Skill](../../.agents/skills/bug-hunter/SKILL.md) | active · rhwp maintainers |

`—`는 해당 런타임용 어댑터가 아직 없다는 뜻이며, capability 자체가 없다는 뜻은 아니다.

## 등록·변경 규칙

새 Claude 에이전트·Claude Skill·Codex Skill을 만들거나 없애기 전에 이 등록부와 열린 Issue/PR을
확인한다. 이름이 아니라 **사용자 산출물, 권위 문서, 비범위**가 겹치는지를 기준으로 판단한다.

1. 같은 산출물과 같은 권위 문서를 쓰면 새 capability를 만들지 않는다. 기존 ID에 해당 런타임의
   어댑터 경로만 추가한다.
2. 기존 산출물의 범위만 넓히면 기존 ID와 권위 문서를 갱신한다. 독립된 산출물·판정 기준·책임이
   생겼을 때만 새 ID를 등록한다.
3. 새 ID에는 책임, 명시적 비범위, 권위 문서, 상태, 소유 maintainer를 반드시 정한다. 상세 절차와
   트리거 문구는 여기서 복제하지 않고 authority와 각 진입점에 둔다.
4. 진입점을 추가·이동·제거하거나 capability를 폐기하면 **같은 PR에서** 이 표를 갱신한다.
   폐기 항목은 지우지 않고 `deprecated` 상태와 대체 capability를 남긴다.
5. capability의 책임·권위 문서·진입점 변경은 rhwp maintainer가 중복 여부와 이 표의 정확성을
   검토한다. 구현과 무관한 개인 프롬프트·일회성 조사 지침은 등록 대상이 아니다.

## 검증

PR 준비 시 변경한 등록 행의 authority와 진입점이 실제 파일을 가리키는지 확인한다. 문서 경로를
옮기거나 이 등록부의 구조를 바꾼 경우에는 다음 검사를 수행한다.

```bash
python3 scripts/check_markdown_links.py --changed-from upstream/devel
python3 scripts/check_document_metadata.py
```

향후 자동 검사를 추가할 때는 ID 중복, active 항목의 누락된 authority·진입점, 같은 파일을 두 ID가
가리키는 경우를 빠른 문서 검사로 검출한다. 산출물·권위·비범위가 실질적으로 중복되는지는 기계적으로
판정하지 않고 maintainer 리뷰로 결정한다.
