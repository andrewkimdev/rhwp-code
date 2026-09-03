/**
 * hwpx-template-engine 표 역할 마커 문법(순수 텍스트 빌더).
 *
 * 표 마커 텍스트는 hwpx-template-engine 이 이미 읽는 그대로의 평문 텍스트
 * (`#BLOCK`/`#REPEAT-BODY:<name>` 등)이다. 이 모듈은 새 마커를 authoring할 때 항상
 * 정식 표기 `#BLOCK`만 쓴다 — 폐지된 동의어 `#HEADER`/`#FOOTER`는 엔진이 기존 문서를
 * 읽을 때만 인식한다. 이 모듈은 "무엇을 쓸지"만 결정하고
 * 실제 authoring 규칙(TITLE→HEADER→BODY→FOOTER 순서, `-NESTED:` 부모 존재
 * 등)의 최종 권위는 여전히 엔진의 `TableRoleMarkerLintValidator`다 — 여기서는
 * 명백히 잘못된 입력만 막는다.
 *
 * `command/commands/template.ts`에서 패널 미리보기(ui/template/roles.ts)가
 * 쓰는 순수 부분을 분리한 것이다 — wasm 호출이 전혀 없으므로 UI가 임포트해도
 * 의존 방향을 오염시키지 않고, mutation-routing-guard 원장과도 무관하다.
 */
export type TemplateTableRole =
  | 'BLOCK' | 'PAGENO' | 'BLOCK_BOTTOM' | 'NOTE'
  | 'REPEAT_TITLE' | 'REPEAT_HEADER' | 'REPEAT_BODY' | 'REPEAT_FOOTER'
  | 'REPEAT_TITLE_NESTED' | 'REPEAT_HEADER_NESTED' | 'REPEAT_BODY_NESTED' | 'REPEAT_FOOTER_NESTED'
  | 'REPEAT_BODY_BOTTOM';

export interface TagSelectionParams {
  role: TemplateTableRole;
  /** REPEAT_* / REPEAT_*_NESTED 에서 자식(비중첩이면 그 블록 자체) 블록명. */
  blockName?: string;
  /** REPEAT_*_NESTED 에서만 필요한 부모 블록명. */
  nestedParent?: string;
}

/** `#PAGENO` 마커 텍스트 — `template-ops.ts`가 태깅된 마커가 PAGENO인지 판정할 때도 재사용한다. */
export const PAGENO_MARKER_TEXT = '#PAGENO';

/**
 * `#PAGENO` 표 안에 authoring해야 하는 예약 누름틀 이름 — hwpx-template-engine이
 * `FieldSchemaExtractor`에서 하드코딩한 이름과 정확히 일치해야 한다
 * (`src/document_core/queries/template_entity.rs`의 `CURRENT_PAGE_FIELD`/`TOTAL_PAGES_FIELD` 포트,
 * TEMPLATE_MARKER_SYNTAX.md §3a).
 */
export const CURRENT_PAGE_FIELD_NAME = '현재_페이지';
export const TOTAL_PAGES_FIELD_NAME = '전체_페이지';

function requireBlockName(blockName: string | undefined, role: string): string {
  if (!blockName) throw new Error(`[template] ${role} 마커에는 블록명이 필요합니다`);
  return blockName;
}

function requireNestedPair(nestedParent: string | undefined, blockName: string | undefined, role: string): string {
  if (!nestedParent || !blockName) {
    throw new Error(`[template] ${role} 마커에는 부모 블록명과 자식 블록명이 모두 필요합니다`);
  }
  return `${nestedParent}/${blockName}`;
}

/** hwpx-template-engine `docs/TEMPLATE_MARKER_SYNTAX.md` §3/§3e 문법 그대로. */
export function buildTableRoleMarkerText(params: TagSelectionParams): string {
  const { role, blockName, nestedParent } = params;
  switch (role) {
    case 'BLOCK': return '#BLOCK';
    case 'PAGENO': return PAGENO_MARKER_TEXT;
    case 'BLOCK_BOTTOM': return '#BLOCK-BOTTOM';
    case 'NOTE': return '#NOTE';
    case 'REPEAT_TITLE': return `#REPEAT-TITLE:${requireBlockName(blockName, role)}`;
    case 'REPEAT_HEADER': return `#REPEAT-HEADER:${requireBlockName(blockName, role)}`;
    case 'REPEAT_BODY': return `#REPEAT-BODY:${requireBlockName(blockName, role)}`;
    case 'REPEAT_FOOTER': return `#REPEAT-FOOTER:${requireBlockName(blockName, role)}`;
    case 'REPEAT_BODY_BOTTOM': return `#REPEAT-BODY-BOTTOM:${requireBlockName(blockName, role)}`;
    case 'REPEAT_TITLE_NESTED': return `#REPEAT-TITLE-NESTED:${requireNestedPair(nestedParent, blockName, role)}`;
    case 'REPEAT_HEADER_NESTED': return `#REPEAT-HEADER-NESTED:${requireNestedPair(nestedParent, blockName, role)}`;
    case 'REPEAT_BODY_NESTED': return `#REPEAT-BODY-NESTED:${requireNestedPair(nestedParent, blockName, role)}`;
    case 'REPEAT_FOOTER_NESTED': return `#REPEAT-FOOTER-NESTED:${requireNestedPair(nestedParent, blockName, role)}`;
    default: throw new Error(`[template] 알 수 없는 역할: ${role as string}`);
  }
}
