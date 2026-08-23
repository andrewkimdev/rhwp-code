---
kind: reference
status: active
canonical: mydocs/manual/rhwp_studio_ui_conventions.md
last_verified: 2026-08-23
---

# rhwp-studio UI 명칭과 CSS 접두어

코드, 이슈, PR, 검증 문서에서 rhwp-studio의 UI 영역을 아래 명칭으로 통일한다.

| 한국어 명칭 | HTML id | 설명 |
| --- | --- | --- |
| 메뉴바 | `#menu-bar` | 파일·편집·보기·입력·서식·쪽·표 메뉴 |
| 도구 상자 | `#icon-toolbar` | 명령 아이콘과 라벨 버튼 모음 |
| 서식 도구 모음 | `#style-bar` | 스타일·글꼴·크기·정렬 등 서식 제어 |
| 편집 영역 | `#scroll-container` | 문서 페이지 렌더링과 스크롤 영역 |
| 상태 표시줄 | `#status-bar` | 쪽·구역·편집 모드·확대 배율 표시 |
| 템플릿 패널 | `#template-panel` | hwpx-template-engine 마커 authoring 도킹 패널 (표 개요·역할 지정) + 누름틀 만들기 — 버튼 하나(`tp-fieldsuggest-btn`)가 표 인접 셀 자동 스캔(`field-name-suggest.ts`)과 선택 텍스트 기반 삽입(`selection-text.ts`) 두 소스를 모두 review list 없이 클릭 1회로 즉시 생성한다(셀 선택 모드 → 행 스캔, 텍스트 선택 → 그 텍스트로 단건 생성, 둘 다 없으면 커서 행 스캔) + Java 엔티티 생성(`template-entity-window.ts` 오버레이 창을 여는 코드/패키지 입력 + 버튼, `template_entity.rs`의 `TemplateEntityGenerator` 클라이언트 포트를 서버 왕복 없이 호출) 두 소스가 별도 fieldset으로 공존 |

## CSS 접두어

| 접두어 | 대상 |
| --- | --- |
| `tb-` | 도구 상자 요소 |
| `sb-` | 서식 도구 모음 요소 |
| `stb-` | 상태 표시줄 요소 |
| `md-` | 메뉴바 드롭다운 요소 |
| `dialog-` | 대화상자 공통 요소 |
| `cs-` | 글자 모양 대화상자 |
| `ps-` | 문단 모양 대화상자 |
| `tp-` | 템플릿 패널 요소 |
| `entity-` | Java 엔티티 초안 오버레이 창(`template-entity-window.ts`) 요소 — `compare-inspector-` 와 같은 인앱 오버레이 패턴, 모달이 아니라 `document.body`에 직접 붙는다 |

새 UI 영역이나 접두어를 도입할 때는 기존 DOM과 CSS에서 실제 사용 여부를 확인하고 이 표를 함께
갱신한다.
