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

제안 생성은 **게이트가 붙어 있다**(아래 "마커 게이트와 검색 범위" 참고): 역할 마커
(`#HEADER`/`#FOOTER`/`#PAGENO`/`#REPEAT-*:`)가 지정된 표에서만 제안을 만들고, 검색
범위는 표 전체가 아니라 **선택된 행**(셀 선택 모드 범위, 없으면 커서가 있는 행)이다.
제안 생성은 마커 authoring("태그 지정")의 다음 단계라는 것이 이 게이트의 의도다.

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

## 규칙 3 — label-above-blank (`ROW_PATTERN_RULES[1]`)

라벨 텍스트가 있는 셀 바로 **아래**(`row + 1`, 같은 col)에 그 라벨과 **정확히 같은
col/colSpan**을 가진 `rowSpan === 1` 빈 셀이 있으면, 그 빈 셀을 후보로 삼는다. 규칙
2(leaf-label-adjacent-blank)의 자매 규칙이다 — "오른쪽" 대신 "아래"만 다르다.

17856415.hwp(의무경찰 지원서)의 "그 밖의 특이사항"(전체 폭 라벨 행) 바로 아래
같은 전체 폭 빈 행이 이 모양이다 — 규칙 2는 라벨 오른쪽에 다른 셀이 없어(표 끝)
아무것도 찾지 못하지만, 규칙 3은 아래를 봐서 찾아낸다.

`suggestedName`은 규칙 2와 같은 공식(`sectionPrefix ? prefix_leafText : leafText`)이고,
채우는 대상도 똑같이 "빈 셀"이므로(`RowPatternCandidate.insertAt` 없음)
`applyFieldSuggestions`/`field-suggest:apply`에서 규칙 2의 후보와 구분 없이 처리된다.

**colSpan이 정확히 같아야** 한다는 조건은 의도적으로 엄격하다 — col만 같고 colSpan이
다르면(예: 라벨은 전체 폭인데 아래는 반으로 쪼개진 두 칸) 그 아래 칸이 정말 이
라벨의 "답변란"인지 그리드만으로 확신할 수 없다.

`MAX_PLAUSIBLE_LABEL_LEN`(20자, 공백 제거 후) 가드도 있다 — 전체 폭 문단(예: 작성방법
안내문) 바로 아래에 여백용 빈 행이 있는 경우, 그 문단 전체를 "라벨"로 오인해 빈
행을 답변란으로 오탐하는 것을 막는다. 실측 라벨 최대 길이(그 밖의 특이사항, 7자)에
여유를 둔 상한이다 — `field-edit-dialog.ts`의 `MAX_FIELD_NAME_LEN`(필드 이름 UI
입력 상한, 250자)과는 목적이 다른, "그럴듯한 라벨인가"를 가르는 휴리스틱 상한이다.

## 규칙 4 — label-inline-room (`ROW_PATTERN_RULES[2]`)

별도 빈 셀도, 바로 아래 빈 행도 없이, 라벨이 **넓은 셀 안에 혼자** 있고 답을 그
라벨 텍스트 바로 뒤(같은 셀 안 여백)에 인라인으로 써야 하는 모양이다. 17856415.hwp의
"인적사항"(rowSpan 3) 섹션 아래 성명/병적지청/주민등록번호/전자우편주소/전화번호/
휴대전화번호 6칸이 이 모양이다 — 예를 들어 "성명" 셀은 colSpan 9로 넓지만 텍스트는
"성명" 두 글자뿐이고, 오른쪽 셀은 빈 칸이 아니라 "병적지청"이라는 또 다른 라벨이다.

규칙 1/2/3과 달리 대상 셀(`cellIdx`)이 **빈 셀이 아니라 라벨 셀 자신**이다 —
`RowPatternCandidate`/`FieldNameSuggestion`에 `insertAt: { cellParaIndex, charOffset }`
(그 셀의 마지막 문단 끝)가 실리고, `applyFieldSuggestions`(`template-panel.ts`)는
`insertAt`이 있으면 `field-suggest:apply`에 `{kind:'cell', cellIdx}` 대신
`{kind:'selection', insertPos}`를 넘긴다 — `selection-text.ts`의 수동 경로가 이미
쓰는 것과 같은 삽입 방식(선택 영역 끝에 삽입)을 재사용하는 것이다. `field-suggest.ts`의
`'selection'` kind 처리는 이미 임의의 `DocumentPosition`을 받아들이도록 일반화돼
있어 이 경로를 추가하는 데 커맨드 쪽 변경은 필요 없었다.

같은 라벨-여백 모양이 "(서명 또는 인)" 같은 안내문이나, 열 경계가 어긋난 답변
그리드(아래 참고)에서도 그리드만 보면 구분이 안 되므로, 오탐을 막기 위한 가드를
순서대로 둔다:

1. **같은 행에 그리드 셀이 1개뿐이면 제외.** 전체 폭 제목/문단 행(예: "그 밖의
   특이사항", "작성방법")은 라벨-값 쌍이 아니다.
2. **column-0 rowSpan>1 앵커 자신은 제외.** 규칙 1과 같은 이유 — 그건 라벨이
   아니라 접두어 출처다.
3. **오른쪽 바로 옆이 빈 셀이면 제외.** 규칙 2가 이미 그 빈 셀을 후보로 삼는다 —
   중복 방지.
4. **왼쪽 바로 옆이 "다른 후보가 이미 채우기로 찜한 빈 셀"이면 제외.** 17856415.hwp의
   "지원자 [빈칸] (서명 또는 인)" 행에서, "지원자" 뒤 빈 칸은 규칙 2가 이미 채움
   대상으로 찜했다 — 그 뒤에 오는 "(서명 또는 인)"은 라벨이 아니라 그 빈 칸에 대한
   서명 안내문이다.
5. **아래 행에 이 라벨의 열 범위와 겹치는 셀이 2개 이상이면 제외.** 17856415.hwp의
   응시지역/모집분야/모집회차(row3, colSpan [4,6,4])는 바로 아래(row4)에 답변용
   서브셀 8개(colSpan [1,2,2,2,1,3,2,1])가 있지만, 그 경계가 라벨의 열 경계와
   어긋난다(예: col3 폭2 셀이 응시지역/모집분야 경계를 가로지름) — 어느 서브셀이
   어느 라벨의 답인지 안전하게 정할 수 없으므로 자동 인라인 후보로 삼지 않고
   사용자의 수동 "태그 지정"에 맡긴다. **겹치는 셀이 정확히 1개**인 경우(예:
   "전화번호"/"휴대전화번호" 바로 아래의 전체 폭 빈 여백 행)는 이 가드에 걸리지
   않는다 — "2개 이상"만 오정렬 답변 그리드로 본다.
6. **섹션 접두어 앵커가 덮는 행이 아니면 제외.** "라벨처럼 보이는 셀 여러 개가
   나란히 있는 행"만으로는 그중 하나가 진짜 라벨(뒤에 값을 채워야 함)인지 이미
   값이 채워진 일반 텍스트인지 그리드만으로 구분할 수 없다 — 예를 들어 단순 2열
   표에서 "전화번호" 옆에 "이미 채워짐"이 있으면, 후자는 라벨이 아니라 이미 입력된
   값이다. column-0 rowSpan>1 앵커가 있는 섹션(예: "인적사항")은 "라벨+값 쌍의
   묶음"이라는 저자 의도가 구조적으로 드러난 경우이므로, 이 신호가 있을 때만 인라인
   후보를 만든다.
7. **라벨치고 너무 길면 제외.** 규칙 3과 같은 `MAX_PLAUSIBLE_LABEL_LEN` 가드.

## 유일성 보정

후보 이름은 ① 문서에 이미 존재하는 필드명(`wasm.getFieldList()`), ② 같은 배치(batch) 안에서
먼저 배정된 이름과 비교해, 충돌하면 `_2`, `_3`, ... 접미어를 붙인다(출력 파일명 충돌 시 쓰는
`_2` 관행과 동일 — [`cli_commands.md`](cli_commands.md)). 후보가 결코 버려지지 않는다 —
review list는 항상 편집 가능하므로 접미어가 마음에 안 들면 사용자가 직접 고친다.

## 이미 필드가 있는 셀

후보 빈 셀에 이미 누름틀/셀 필드가 있으면(`getCellProperties(...).fieldName`) 그 후보는
`alreadyHasField: true`로 표시되고 "삽입" 대상에서 제외된다(review list에는 플래그로만
남는다) — v1은 기존 필드를 이름 변경(rename)하지 않는다, 오직 진짜 빈 칸에만 삽입한다.

규칙 4(label-inline-room)처럼 `insertAt`이 있는 후보는 빈 셀을 채우는 게 아니라 라벨
셀 자신에 인라인 삽입하므로, 셀 필드 API로는 재스캔 시 중복 제안을 막을 수 없다(그
셀은 삽입 후에도 여전히 텍스트가 있고, 규칙 4의 가드도 다시 통과한다) — 대신
`wasm.getFieldInfoAt(...)`을 삽입 지점(`insertAt`)에 직접 호출해 이미 필드 안인지
확인한다(`selection-text.ts`의 수동 경로가 이미 쓰는 것과 같은 API).

## 마커 게이트와 검색 범위

호출부(`template-panel.ts`의 `generateFieldSuggestions`)는 두 조건으로 제안 생성을
게이트한다:

1. **마커 게이트** — 현재 표의 첫 셀 텍스트(`readTableMarkerText`)가 역할 마커
   어휘(`#HEADER`/`#FOOTER`/`#PAGENO`/`#REPEAT-*:`, `isTemplateTableMarkerText`) 안에
   있을 때만 `suggestFieldNames`를 부른다. 단순 `#` 접두사가 아니라 어휘 전체로
   매칭하므로 첫 셀에 우연히 `#`으로 시작하는 원본 텍스트가 있어도 게이트가 열리지
   않는다. 마커가 없으면 "위 '태그 지정'으로 먼저 역할을 지정하세요" 안내만 나온다.
2. **행 범위** — 검색 범위는 선택된 행(셀 선택 모드 범위, 없으면 커서가 있는 행,
   `getSelectedRowRange`)이다. 힌트 영역("선택된 행: N~M")과 같은 정의를 공유하므로
   "힌트에 보이는 행 = 제안이 검색하는 행"이 항상 성립한다. 후보의 **대상 셀**(빈 칸
   또는 인라인 삽입의 라벨 셀, review list의 R 표시 행)이 범위 밖이면 그 후보는
   제외된다 — 규칙 3처럼 라벨 행과 빈 칸 행이 다른 행에 걸쳐 있을 때는 "채워질
   대상"이 있는 행을 기준으로 판정한다.

태깅은 마커 행(전체 폭 병합 셀)을 row 0에 삽입하므로(`setTableRoleMarker`,
`command/commands/template.ts`), 태깅된 표의 마커 셀 텍스트는 규칙 2/3/4의 라벨 후보에서
제외한다(`isTemplateTableMarkerText` 재사용) — 마커는 authoring 주석이지 라벨이 아니다.
이 제외가 없으면 규칙 3이 마커 행 아래 같은 폭의 빈 행을 발견했을 때 `#HEADER`나
`#REPEAT-BODY:품목내역`(공백 제거 17자 — `MAX_PLAUSIBLE_LABEL_LEN` 가드도 통과) 같은
마커 텍스트 자체를 이름으로 제안한다.

### 반복 블록(`#REPEAT-*:`) 표도 이제 제안된다

과거에는 `#REPEAT-*:` 표에서 제안 생성 자체를 막았다(`isRepeatTaggedTable` 조기 반환) —
행마다 같은 이름이 반복되는 게 의도된 설계인데(fill 시점에 `이름[N]`으로 disambiguate,
[`rhwp-form-fill`](cli_commands.md) 규약) 표 전체 스캔이 이 의도된 중복을 "충돌"로
오인해 불필요한 접미어를 붙였기 때문이다. 검색 범위가 "선택된 행"으로 좁아진 지금은
이 모호성이 발생하지 않는다 — 어느 행의 후보를 뽑을지 사용자가 행 선택으로 직접
정하므로, 반복 표도 게이트가 허용하는 표 중 하나로 남는다.

## 규칙과 무관한 별도 소스 — 선택 텍스트 기반 제안

위 규칙 1/2와 `ROW_PATTERN_RULES`는 모두 표 그리드를 스캔해 **여러** 후보를 한 번에 만들고
review list로 검토·적용하는 **자동** 소스다. 이와 별개로, `src/core/selection-text.ts` +
`src/core/field-name-dedup.ts`가 제공하는 **수동** 소스가 있다 — 사용자가 문서에서
텍스트("신청인")를 직접 드래그 선택하고 `#template-panel`의 "선택한 텍스트로 누름틀 만들기"
버튼을 누르면, 그 선택 텍스트를 그대로 이름/안내문으로 갖는 누름틀이 **review list를 거치지
않고 그 자리에서 즉시** 삽입된다. 후보가 항상 하나뿐이라 배치 검토가 필요 없기 때문이다 —
자동 스캔은 표 하나에서 여러 빈 칸을 한 번에 찾아내므로 review list(체크/이름 편집/일괄
적용)가 그대로 필요하지만, 선택 텍스트 경로는 사용자가 이미 정확히 어느 텍스트를 어떤
이름으로 쓸지 직접 골랐으므로 한 번 더 확인받을 이유가 없다.

이 소스가 필요한 이유: "신청인" 텍스트 뒤에 긴 공백, 그 뒤에 "(인)"이 오는 서명란 패턴은
표의 두 셀로 나뉘어 있지 않고 본문 문단 또는 표 셀 하나 안에 통짜 텍스트로 들어있는 경우가
많다 — 규칙 2(leaf-label-adjacent-blank)는 "라벨 셀 + 인접 빈 셀"이라는 두 개의 별도 셀
모양을 전제하므로 이런 통짜 텍스트 패턴을 잡아내지 못한다.

이 소스는 `ROW_PATTERN_RULES`의 새 항목이 **아니다** — 표 그리드(`readTableGrid`)를 전혀
읽지 않고, `ih.getSelection()`이 반환하는 임의의 `DocumentPosition` 범위 하나만 다룬다.
따라서 표 안(단일 셀)뿐 아니라 본문 문단에서도 동작한다. 삽입 위치는 선택 영역의 끝
(`insertPos = end`)이다 — 라벨 텍스트 자체는 그대로 두고 그 바로 뒤(공백이 시작되는 지점)에
누름틀을 삽입한다.

이 삽입 방식(라벨 텍스트 뒤에 인라인 삽입)은 규칙 4(label-inline-room)가 재사용한다 —
둘 다 최종적으로 `field-suggest:apply`에 `{kind:'selection', insertPos}`를 넘긴다.

**알려진 비대칭**: 위 규칙 1~4는 모두 섹션 접두어(`buildSectionPrefixMap`)를 적용하지만,
이 수동 선택 텍스트 경로는 여전히 적용하지 않는다 — `extracted.text`를 그대로
`resolveUniqueName`에 넘긴다(`insertFieldFromSelection`, `template-panel.ts`). 사용자가
"인적사항" 섹션 안에서 "성명"을 드래그 선택해 삽입해도 이름은 `인적사항_성명`이 아니라
그냥 `성명`이 된다. 이번 규칙 3/4 작업 범위 밖으로 남겨둔 후속 과제다.

몇 가지 지점에서 자동 스캔과 의도적으로 다르게 동작한다:

- **`#REPEAT-*:` 태그된 표 안에서도 허용한다.** `isRepeatTaggedTable`은 자동 스캔에서만
  호출된다 — "행마다 라벨이 반복되는 게 의도된 설계"라는 자동 스캔의 모호성 회피 근거가,
  사용자가 정확히 이 위치를 골라 선택한 수동 후보에는 애초에 적용되지 않는다.
- **문단/셀 경계를 넘는 선택, 중첩 표(`cellPath.length > 1`), 글상자 선택은 후보로 만들지
  않는다**(`singleParagraphSelectionQuery`가 `null` 반환) — 지원 범위는 "본문의 한 문단"
  또는 "단일 깊이 표 셀의 한 문단" 하나뿐이다.
- **이름 충돌 해소(`_2`, `_3`, ...)는 `resolveUniqueName`(`field-name-dedup.ts`)으로
  두 소스가 공유한다** — 자동 스캔의 `suggestFieldNames`도 같은 함수를 호출한다. 다만
  선택 텍스트 경로는 즉시 삽입 한 건뿐이라 "같은 배치 안 다른 대기 후보"라는 개념이
  없다 — `wasm.getFieldList()`(문서에 이미 있는 이름)와만 비교하며, 조정된 최종 이름은
  삽입 직후 메시지로 보여준다.

## 확장 방법 — 새 레이아웃이 나오면

`ROW_PATTERN_RULES`는 순서 있는 `RowPatternRule[]`이다. "라벨+콜론 한 셀" 같은 새 모양이
실제 서식에서 관찰되면, 같은 시그니처(`(grid, prefixMap) => RowPatternCandidate[]`)로
규칙 함수를 하나 더 만들어 이 배열에 추가한다 — 기존 규칙이나 파이프라인
(`suggestFieldNames`)을 바꿀 필요는 없다. 새 규칙을 추가하면 이 문서에도 규칙 번호와
판정 조건을 추가한다.

`RowPatternCandidate`(따라서 `FieldNameSuggestion`도)에는 선택 필드 `insertAt?: {
cellParaIndex, charOffset }`가 있다 — 규칙 1~3처럼 "빈 셀을 채우는" 후보는 이 필드를
비워 두고(`cellIdx`가 그 빈 셀), 규칙 4처럼 "이미 텍스트가 있는 셀 자신에 인라인
삽입하는" 후보는 이 필드에 삽입 지점을 채운다(`cellIdx`가 그 텍스트 셀 자신). UI
(`applyFieldSuggestions`, `template-panel.ts`)는 `insertAt` 유무로 `field-suggest:apply`에
`{kind:'cell', cellIdx}`와 `{kind:'selection', insertPos}` 중 무엇을 넘길지 정한다 —
새 규칙이 "빈 셀 채우기"가 아니라 "기존 텍스트 뒤 인라인 삽입" 모양이면 `insertAt`을
채우면 된다.

## v1 스코프

- 분석 범위는 커서가 있는 "현재 표"뿐이다 — 문서 전체 다중 표 스캔은 하지 않는다
  (`#template-panel`의 기존 UX와 동일 전제). 그리고 그 표 안에서도 **마커 게이트를
  통과한 표의 선택된 행**만 검색한다("마커 게이트와 검색 범위" 참고).
- 자동 스캔(표 인접 셀)은 실제 삽입 전에는 아무것도 쓰지 않는다 — review list에서
  체크/이름 편집을 마친 뒤 "적용"을 눌러야 `field-suggest:apply` 커맨드가 한 번의 undo
  단위로 실행된다. 선택 텍스트 경로는 후보가 항상 하나뿐이므로 review 단계 없이 버튼
  클릭 한 번이 곧 `field-suggest:apply` 단일 아이템 호출이다(undo는 여전히 한 단위).

## 관련 코드

- 감지(표 인접 셀, 자동): `rhwp-studio/src/core/field-name-suggest.ts`
- 감지(선택 텍스트, 수동): `rhwp-studio/src/core/selection-text.ts`
- 이름 유일성 공유 로직: `rhwp-studio/src/core/field-name-dedup.ts`
- UI: `rhwp-studio/src/ui/template-panel.ts` ("누름틀 이름 제안" 그룹)
- 적용 커맨드: `rhwp-studio/src/command/commands/field-suggest.ts`
- 단위 테스트: `rhwp-studio/tests/field-name-suggest.test.ts`(규칙 1~4 및 각 가드의
  positive/negative 케이스, 17856415.hwp 표 전체를 옮긴 회귀 테스트, 마커 게이트
  어휘/행 범위/마커 셀 제외 케이스 포함),
  `rhwp-studio/tests/selection-text.test.ts`, `rhwp-studio/tests/field-name-dedup.test.ts`
- e2e: `rhwp-studio/e2e/field-suggest-panel.test.mjs`(표 인접 셀 — 게이트 메시지 TC-1b,
  #HEADER 태깅 후 규칙 1/2 TC-1c~4, 규칙 3/4 TC-5~7),
  `rhwp-studio/e2e/field-suggest-selection.test.mjs`(선택 텍스트)

이 문서는 **감지 규칙**(무엇을 어떻게 이름으로 바꾸는가)만 다룬다. wasm 필드 API 시그니처,
`DocumentPosition` 조립, 길이 상한, undo 라우팅, 테스트 fixture 재사용 등 **엔지니어링
디테일**은 [`rhwp_studio_clickhere_field_guide.md`](rhwp_studio_clickhere_field_guide.md)를 본다.
