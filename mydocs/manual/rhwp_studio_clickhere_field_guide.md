---
kind: reference
status: active
canonical: mydocs/manual/rhwp_studio_clickhere_field_guide.md
last_verified: 2026-08-23
---

# rhwp-studio 누름틀(ClickHere 필드) 편집 엔지니어링 가이드

rhwp-studio에서 누름틀(ClickHere 필드) 삽입·조회·수정·삭제 기능을 다룰 때 매번 wasm-bridge
전체를 grep하지 않도록, 관련 API·파일·규약을 한 곳에 모은다. **소비자가 CLI로 서식을 채우는
방법**(mail merge, `이름[N]` 반복 필드 등)은 여기가 아니라 [`form_filling_guide.md`](form_filling_guide.md)와
[`rhwp-form-fill` skill](cli_commands.md)을 본다 — 이 문서는 **rhwp-studio 안에서 누름틀을
만들고 고치는 TS 엔지니어링**만 다룬다.

## 파일 지도

| 역할 | 파일 |
| --- | --- |
| wasm 필드 API 전체 (조회/삽입/수정/삭제) | `rhwp-studio/src/core/wasm-bridge.ts` (`// ─── 필드 API (Task 230) ───` 섹션, `getFieldList`부터) |
| 누름틀 삽입 대화상자 | `rhwp-studio/src/ui/field-insert-dialog.ts` |
| 누름틀 고치기 대화상자 (+ `ClickHereProps`, 길이 상한 3종) | `rhwp-studio/src/ui/field-edit-dialog.ts` |
| `insert:field`/`field:edit`/`field:remove` 커맨드 | `rhwp-studio/src/command/commands/insert.ts`, `edit.ts` |
| 누름틀 **이름 자동 제안**(라벨→빈칸 감지, 접두어, 유일성) | `rhwp-studio/src/core/field-name-suggest.ts` |
| 이름 제안 review list UI (`#template-panel` "누름틀 이름 제안" 그룹) | `rhwp-studio/src/ui/template-panel.ts` |
| 이름 제안 일괄 삽입 커맨드 | `rhwp-studio/src/command/commands/field-suggest.ts` |
| 이름 자동 제안 규칙 설명(왜 접두어를 붙이는가) | [`field_naming_heuristics.md`](field_naming_heuristics.md) |
| Rust 쪽 필드 조회/설정 엔진 | `src/document_core/queries/field_query.rs` (`FieldLocation`, `NestedEntry`, `collect_all_fields` 계열) |

## WasmBridge 필드 API 시그니처 (다시 찾지 말 것)

모두 `rhwp-studio/src/core/wasm-bridge.ts`, `// ─── 필드 API (Task 230) ───` 섹션 안:

- `getFieldList(): Array<{ fieldId, fieldType, cellField, name, guide, command, value, location: { sectionIndex, paraIndex, path? }, startCharIdx?, endCharIdx?, editableInForm? }>`
  — **문서 전체** 필드 스캔. 이름 유일성 검사(`field-name-suggest.ts`가 하는 것)에 이걸 쓴다.
  느릴 수 있으니 표 하나 스캔할 때마다 부르지 말고 배치 시작 시 한 번만.
- `getCellProperties(sec, ppi, ci, cellIdx).fieldName` — **셀 하나**에 이미 필드가 있는지
  빠르게 확인. "이 특정 셀이 비어 있나?"는 이걸 쓴다(`getFieldList()`를 스캔해 위치를 맞추는
  것보다 훨씬 싸다).
- `getFieldInfoAt(pos: DocumentPosition): FieldInfoResult` — **커서 위치** 기준 조회
  (`{ inField, fieldId?, fieldType?, ... }`). `field:edit`/`field:remove`의 `canExecute`가
  거저 얻는 `ctx.inField`도 결국 `inputHandler.isInField()`(내부적으로 이걸 씀)에서 온다.
- `insertClickHereField(pos: DocumentPosition, guide, memo, name, editable): { ok, fieldId?, charOffset? }`
  — 3갈래로 분기한다(`pos.cellPath?.length > 1`이면 `insertClickHereFieldByPath`,
  `pos.parentParaIndex`+`controlIndex`가 있으면 `insertClickHereFieldInCell`, 아니면 본문
  `insertClickHereField`). **중첩 표(depth>1)는 `cellPath`를 채워야만 올바른 갈래로 간다** —
  단순 표만 다루는 커맨드(아래 "중첩 표는 아직" 참고)는 `cellPath`를 만들 필요 없이 그냥
  `parentParaIndex`/`controlIndex`/`cellIndex`/`cellParaIndex`만 채우면 된다.
- `updateClickHereProps(fieldId, guide, memo, name, editable): { ok }` — **이미 존재하는
  필드의 이름/안내문/메모/편집가능여부를 전부 바꿀 수 있다.** `field:edit` 커맨드
  (`command/commands/edit.ts`)가 이미 이 경로로 "필드 이름"까지 바꾸고 있다 — **즉 누름틀
  rename에 새 wasm export는 필요 없다.** (`field-name-suggest`의 v1 계획서가 "rename엔 새
  wasm-bridge wrapper가 필요하다"고 가정했던 건 틀렸다 — `updateClickHereProps`가 이미 있다.
  v1이 그래도 rename을 안 한 진짜 이유는 범위를 "진짜 빈 칸에만 삽입"으로 좁히기 위해서였다.)
  fieldId를 모르면 `getFieldList()`를 돌며 `location.path`의 `TableCell{control_index,
  cell_index}`가 목표 (ppi/ci 아래) cellIdx와 일치하는 항목을 찾는다 — Rust
  `FieldLocation`/`NestedEntry`(`field_query.rs:14-33`) 구조가 그대로 JSON `location.path`로
  나온다.
- `removeFieldAt(pos)` / `getClickHereProps(fieldId)` — 나머지 CRUD. `field:remove`가
  `ih.confirmRemoveCurrentField()`를 통해 전자를 쓴다(양식모드/일반모드 분기는
  `input-handler.ts` 쪽).

## `DocumentPosition`으로 임의 셀 주소 만들기

새 코드가 (커서가 아니라) **알고 있는 특정 셀**에 삽입/조회하려면(예: 배치 삽입, 이름 제안
적용) 이렇게 채운다:

```ts
{
  sectionIndex: sec,
  paragraphIndex: 0,      // 표 안이면 무시되는 필드 — 안 쓰이지만 타입상 필수라 0 채움
  charOffset: 0,          // 대상이 빈 셀이면 0
  parentParaIndex: ppi,
  controlIndex: ci,
  cellIndex: cellIdx,
  cellParaIndex: 0,
}
```

`cellPath`는 **단순(비중첩) 표에서는 절대 채우지 않는다** — 채우면 `insertClickHereField`가
`insertClickHereFieldByPath` 갈래로 새서 실패한다. 중첩 표(depth>1)는 이 문서의 범위 밖이다 —
`template:tag-selection`/`field-suggest:apply` 둘 다 `(pos.cellPath?.length ?? 0) > 1`이면
그냥 리턴하고 지원하지 않는다(동일 전례, 이유는 `table:split`과 같은 이유로 표 나누기 계열이
중첩을 아직 못 다뤄서).

## 셀 텍스트 전체(여러 문단) 읽기 관용구

한 셀에 문단이 여러 개일 수 있으므로 항상 `getCellParagraphCount` → 루프 →
`getCellParagraphLength` → `getTextInCell` 순서로 읽는다(한 문단만 읽고 끝내면 나머지를
놓친다). 이 관용구가 이미 세 곳에 있다 — `field-name-suggest.ts`의 `readTableGrid()`,
`diff-engine.ts:576` 부근, `table-outline.ts`의 `readTableMarkerText`(단, 이건 첫 문단만
필요해서 루프 없이 1회만 읽음). 텍스트에서 레이아웃 잡음(음절별 줄바꿈 등)을 없애려면
`text.replace(/\s+/g, '')`를 쓴다(`trim()`보다 강함 — 내부 공백까지 지운다) —
`suggestBlockNameFromCurrentCell`(template-panel.ts)과 `readTableGrid`가 같은 관용구를 쓴다.

## 길이 상한 (건드리지 말고 재사용)

`field-edit-dialog.ts`에 세 상수가 있다 — `MAX_FIELD_NAME_LEN=250`,
`MAX_FIELD_GUIDE_LEN=250`, `MAX_FIELD_MEMO_LEN=1000`. 근거는 Rust 직렬화기
(`src/serializer/control.rs`)가 길이를 `u16`으로 기록해서 65536 근처에서 랩어라운드로 레코드가
손상되는 것(#2851)을 프런트에서 미리 훨씬 낮은 상한으로 막는 것 — `.rs`를 고치는 게 아니라
입력 단계에서 막는 방어. 새 코드가 이름/안내문/메모를 다루면 이 상수를 import해서 쓴다, 새로
매직넘버를 만들지 않는다.

## 안내문(guide) = 이름(name) 동기화 규약

`field-suggest:apply`(2026-08-23 수정)는 `insertClickHereField`의 `guide` 인자에 범용 문구
("입력하세요") 대신 **필드 이름 자체**를 넣는다 — 여러 필드를 한 번에 만들 때 셀에 찍히는
안내문이 전부 같은 문구면 채우기 전에는 어떤 칸인지 구별이 안 되기 때문. **배치 삽입(한 번에
여러 필드를 만드는 새 기능)을 추가할 때는 이 관례를 따른다** — `guide: item.name`. 반대로
사용자가 대화상자(`field-insert-dialog.ts`)로 **하나씩** 만들 때는 여전히 기본값
`'입력하세요'`를 유지한다(사용자가 그 필드 하나에 뭘 적을지 직접 안내문을 쓸 기회가 있으므로
동기화가 필요 없다) — 일괄/자동 생성 경로에서만 이름과 동기화한다는 게 핵심 구분이다.

## 새 필드-뮤테이션 커맨드를 추가할 때 (잊으면 CI에서 걸림)

1. wasm 뮤테이터 호출은 **반드시** `ih.executeOperation({kind:'snapshot', operation: (wasm) => {...}})`
   안에서 해야 undo에 기록된다(`insert:field`/`field:edit`/`field-suggest:apply` 전부 이 패턴).
2. 새 커맨드 파일을 `src/command/commands/`에 추가하면
   `rhwp-studio/tests/mutation-routing-guard.test.ts`의 `BASELINE`에 그 파일의 뮤테이터
   **호출 사이트 개수**(런타임 반복 횟수가 아니라 소스상 호출문 개수)를 등록해야 한다 — 안 하면
   "뮤테이션 표면 원장" 테스트가 실패한다. `insertClickHereField`처럼 브리지 메서드가 이미
   `MUTATING_METHODS`(`src/core/mutation-method-registry.ts`)에 있으면 그 목록 자체는 안 바꿔도
   된다 — 새 호출부만 BASELINE에 등록.
3. 표 하나를 다루는 커맨드는 `(pos.cellPath?.length ?? 0) > 1`로 중첩 표를 걸러내는 게
   기존 전례(`template:tag-selection`, `field-suggest:apply`) — 새로 만들 이유 없으면 같은
   가드를 복붙한다.

## 테스트 작성 시 재사용할 것

- **단위 테스트 fake WasmBridge**: 표 하나만 다루는 순수 함수를 테스트할 땐
  `tests/field-name-suggest.test.ts`의 `makeFakeTableWasm()`처럼 최소 메서드만 구현한 객체로
  충분하다(`getTableDimensions`/`getCellInfo`/`getCellParagraphCount`/`getCellParagraphLength`/
  `getTextInCell`/`getCellProperties`/`getFieldList`). 여러 표를 문서 순서로 찾아야 하면
  (`listTopLevelTables`처럼) `tests/template-marker-authoring.test.ts`의 `makeFakeWasm()` —
  `findNearestControlForward`의 same-paragraph inclusive/exclusive 경계까지 재현한 더 무거운
  버전 — 을 쓴다.
- **e2e**: `e2e/field-suggest-panel.test.mjs`가 이 계열의 첫 실물 브라우저 테스트다 — 패턴은
  `createNewDocument` → `window.__wasm.createTable`+`mergeTableCells`+`insertTextInCell`로
  합성 표 구성(`ih.executeOperation({kind:'snapshot', ...})` 안에서) → `ih.cursor.moveTo({...})`
  로 커서를 셀에 두고 `window.__eventBus.emit('command-state-changed')`로 패널을 갱신시킴 →
  실제 DOM 버튼(`document.querySelector('#template-panel .tp-fieldsuggest-generate-btn').click()`)
  클릭 → `window.__wasm.getFieldList()`로 결과 검증. 이 흐름 전체가 재사용 가능한 템플릿이다.
- **실물 파일로 수동 검증**: e2e 헬퍼의 `loadHwpFile()`은 `rhwp-studio/public/samples/` 상대
  경로만 받는다 — 그 밖의 절대경로 fixture(`kr-gov-form-harvester/data/files/...`)를 브라우저에
  직접 로드하려면, 로컬에서 파일을 읽어 base64로 인코딩한 뒤
  `window.__wasm.loadDocument(new Uint8Array(...), fname)`을 `page.evaluate`로 직접 호출한다
  (`/samples` 제약을 우회). 이런 임시 확인 스크립트는 `e2e/tmp-*.mjs`로 만들고 실행 후
  삭제한다 — 영구 스위트에 넣지 않는다(HWP-STUDIO-CONTEXT.md의 기존 관례).

## 관련 문서

- [`field_naming_heuristics.md`](field_naming_heuristics.md) — 이름 자동 제안의 규칙(접두어,
  유일성, 확장 방법) 자체. 이 문서와 겹치지 않게 "규칙"은 저기, "엔지니어링/API"는 여기.
- [`rhwp_studio_ui_conventions.md`](rhwp_studio_ui_conventions.md) — `#template-panel`,
  `tp-` 접두어 등 UI 명칭 규약.
- [`form_filling_guide.md`](form_filling_guide.md), [`cli_commands.md`](cli_commands.md) —
  CLI로 서식을 채우는 소비자 쪽(mail merge, `이름[N]`) — rhwp-studio 저작과는 다른 계층.
