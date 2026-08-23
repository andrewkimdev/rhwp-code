---
kind: reference
status: active
canonical: mydocs/manual/field_naming_heuristics.md
last_verified: 2026-08-23
---

# 누름틀 이름 자동 제안 — 감지 규칙

rhwp-studio `#template-panel`의 "누름틀 이름 제안" 그룹(`src/core/field-name-suggest.ts`)이
표의 라벨/빈 칸 모양에서 self-describing한 누름틀 이름을 어떻게 뽑아내는지 정리한다. 이 규칙
자체가 확장 대상이므로(새 서식에서 다른 레이아웃이 관찰될 때마다), 이름 하드코딩이 아니라
"어떤 모양을 어떻게 이름으로 바꾸는가"를 순서 있는 규칙 목록으로 유지한다.

## 왜 라벨을 그대로 이름으로 쓰지 않는가

누름틀 이름은 fill 시점에 소비자가 참조하는 JSON 키가 되므로 라벨 텍스트와 같아야
self-describing하다("주소" 칸 → 누름틀 "주소"). 하지만 같은 라벨이 문서에 두 번 이상
나오면(예: "신청인" 섹션과 "법인명" 섹션에 각각 "전화번호") 이름이 충돌한다. 이를 매번
trial-and-error로 푸는 대신, 표 구조에서 "어느 섹션 소속인가"를 읽어 접두어로 구조적으로
해소한다.

## 규칙 1 — 섹션 접두어 (`buildSectionPrefixMap`)

column-0에 있고 `rowSpan > 1`인 셀(예: "신\n청\n인" — 음절별 줄바꿈은 레이아웃 잡음이므로
공백 제거 후 비교)은 자신이 덮는 모든 행에 자신의 텍스트를 접두어로 제공한다. 앵커가 없는
행(문서에 섹션 구분이 없는 단순 표)은 접두어 없이 라벨을 그대로 쓴다.

## 규칙 2 — leaf-label-adjacent-blank (`ROW_PATTERN_RULES[0]`)

비어 있지 않은 텍스트를 가진 셀(단, column-0의 `rowSpan > 1` 앵커 자신은 제외 — 그건 라벨이
아니라 접두어 출처다)의 바로 오른쪽(`col + colSpan`)에 있는 셀이 `rowSpan === 1`이고 완전히
비어 있으면, 그 빈 셀을 "라벨 텍스트로 채울 후보"로 삼는다.

`suggestedName = sectionPrefix ? \`${sectionPrefix}_${leafText}\` : leafText`

## 유일성 보정

후보 이름은 ① 문서에 이미 존재하는 필드명(`wasm.getFieldList()`), ② 같은 배치(batch) 안에서
먼저 배정된 이름과 비교해, 충돌하면 `_2`, `_3`, ... 접미어를 붙인다(출력 파일명 충돌 시 쓰는
`_2` 관행과 동일 — [`cli_commands.md`](cli_commands.md)). 후보가 결코 버려지지 않는다 —
review list는 항상 편집 가능하므로 접미어가 마음에 안 들면 사용자가 직접 고친다.

## 이미 필드가 있는 셀

후보 빈 셀에 이미 누름틀/셀 필드가 있으면(`getCellProperties(...).fieldName`) 그 후보는
`alreadyHasField: true`로 표시되고 "삽입" 대상에서 제외된다(review list에는 플래그로만
남는다) — v1은 기존 필드를 이름 변경(rename)하지 않는다, 오직 진짜 빈 칸에만 삽입한다.

## 반복 블록(`#REPEAT-*:`) 표 제외

`#REPEAT-BODY:`/`-HEADER:`/`-FOOTER:`/`-TITLE:`(및 `-NESTED:`)로 태깅된 표는 행마다 같은
이름이 반복되는 게 의도된 설계이고, fill 시점에 `이름[N]`으로 disambiguate된다
([`rhwp-form-fill`](cli_commands.md) 규약). 이 표에서는 제안 생성 자체를 막는다
(`isRepeatTaggedTable`) — 그렇지 않으면 이 기능이 반복 블록의 의도된 중복을 "충돌"로 오인해
불필요한 접미어를 붙이게 된다.

## 확장 방법 — 새 레이아웃이 나오면

`ROW_PATTERN_RULES`는 순서 있는 `RowPatternRule[]`이다. "라벨 아래 빈 칸", "라벨+콜론 한 셀"
같은 새 모양이 실제 서식에서 관찰되면, 같은 시그니처
(`(grid, prefixMap) => RowPatternCandidate[]`)로 규칙 함수를 하나 더 만들어 이 배열에
추가한다 — 기존 규칙이나 파이프라인(`suggestFieldNames`)을 바꿀 필요는 없다. 새 규칙을
추가하면 이 문서에도 규칙 번호와 판정 조건을 추가한다.

## v1 스코프

- 분석 범위는 커서가 있는 "현재 표"뿐이다 — 문서 전체 다중 표 스캔은 하지 않는다
  (`#template-panel`의 기존 UX와 동일 전제).
- 실제 삽입 전에는 아무것도 쓰지 않는다 — review list에서 체크/이름 편집을 마친 뒤
  "적용"을 눌러야 `field-suggest:apply` 커맨드가 한 번의 undo 단위로 실행된다.

## 관련 코드

- 감지: `rhwp-studio/src/core/field-name-suggest.ts`
- UI: `rhwp-studio/src/ui/template-panel.ts` ("누름틀 이름 제안" 그룹)
- 적용 커맨드: `rhwp-studio/src/command/commands/field-suggest.ts`
- 단위 테스트: `rhwp-studio/tests/field-name-suggest.test.ts`

이 문서는 **감지 규칙**(무엇을 어떻게 이름으로 바꾸는가)만 다룬다. wasm 필드 API 시그니처,
`DocumentPosition` 조립, 길이 상한, undo 라우팅, 테스트 fixture 재사용 등 **엔지니어링
디테일**은 [`rhwp_studio_clickhere_field_guide.md`](rhwp_studio_clickhere_field_guide.md)를 본다.
