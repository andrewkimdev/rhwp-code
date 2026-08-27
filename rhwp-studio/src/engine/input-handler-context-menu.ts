/** input-handler context-menu builder methods — extracted from InputHandler class */
/* eslint-disable @typescript-eslint/no-explicit-any */

import type { ContextMenuItem } from '@/ui/context-menu';

/** 표 객체 선택 상태 컨텍스트 메뉴 항목 */
export function getTableObjectContextMenuItems(this: any): ContextMenuItem[] {
  return [
    { type: 'command', commandId: 'edit:cut' },
    { type: 'command', commandId: 'edit:copy' },
    { type: 'command', commandId: 'edit:paste' },
    { type: 'separator' },
    { type: 'command', commandId: 'table:caption-toggle', label: '캡션 넣기(A)' },
    { type: 'separator' },
    { type: 'command', commandId: 'table:cell-props', label: '표 속성...' },
    { type: 'separator' },
    // 표 나누기는 커서 행이 분할 기준이라 셀 내부 메뉴에만 둔다 —
    // 객체 선택 상태에는 기준 행이 없다.
    { type: 'command', commandId: 'table:attach', label: '표 붙이기' },
    { type: 'separator' },
    { type: 'command', commandId: 'table:delete' },
  ];
}

/** 그림 객체 선택 컨텍스트 메뉴 항목 */
export function getPictureObjectContextMenuItems(this: any): ContextMenuItem[] {
  const ref = this.cursor.getSelectedPictureRef();

  // 다중 선택: 개체 묶기 메뉴
  if (this.cursor.isMultiPictureSelection()) {
    return [
      { type: 'command', commandId: 'insert:group-shapes', label: '개체 묶기(G)' },
      { type: 'separator' },
      { type: 'command', commandId: 'insert:picture-delete', label: '지우기(D)' },
    ];
  }

  const items: ContextMenuItem[] = [
    { type: 'command', commandId: 'edit:cut' },
    { type: 'command', commandId: 'edit:copy' },
    { type: 'command', commandId: 'edit:paste' },
    { type: 'separator' },
  ];
  // 수식 객체: "수식 편집..." 항목 추가
  if (ref?.type === 'equation') {
    items.push(
      { type: 'command', commandId: 'insert:equation-edit', label: '수식 편집...' },
      { type: 'separator' },
    );
  }
  items.push(
    { type: 'command', commandId: 'insert:arrange-front', label: '맨 앞으로' },
    { type: 'command', commandId: 'insert:arrange-forward', label: '앞으로' },
    { type: 'command', commandId: 'insert:arrange-backward', label: '뒤로' },
    { type: 'command', commandId: 'insert:arrange-back', label: '맨 뒤로' },
    { type: 'separator' },
  );
  // 그룹 개체: 개체 풀기
  if (ref?.type === 'group') {
    items.push(
      { type: 'command', commandId: 'insert:ungroup-shapes', label: '개체 풀기(U)' },
      { type: 'separator' },
    );
  }
  // 그림/도형 객체: 캡션 넣기
  if (ref?.type === 'image' || ref?.type === 'shape') {
    items.push(
      { type: 'command', commandId: 'insert:caption-toggle', label: '캡션 넣기(A)' },
    );
  }
  items.push(
    { type: 'command', commandId: 'insert:picture-props', label: '개체 속성(P)...' },
    { type: 'separator' },
    { type: 'command', commandId: 'insert:picture-delete', label: '지우기(D)' },
  );
  return items;
}

/** 표 셀 내부 컨텍스트 메뉴 항목 */
export function getTableContextMenuItems(this: any): ContextMenuItem[] {
  return [
    { type: 'command', commandId: 'edit:cut' },
    { type: 'command', commandId: 'edit:copy' },
    { type: 'command', commandId: 'edit:paste' },
    { type: 'command', commandId: 'edit:format-copy' },
    { type: 'command', commandId: 'edit:format-paste' },
    { type: 'separator' },
    { type: 'command', commandId: 'table:cell-props', label: '셀 속성...' },
    { type: 'separator' },
    { type: 'command', commandId: 'table:insert-row-col' },
    { type: 'separator' },
    { type: 'command', commandId: 'table:delete-row-col' },
    { type: 'separator' },
    { type: 'command', commandId: 'table:cell-height-equal' },
    { type: 'command', commandId: 'table:cell-width-equal' },
    { type: 'separator' },
    { type: 'command', commandId: 'table:cell-merge' },
    { type: 'command', commandId: 'table:cell-split' },
    { type: 'command', commandId: 'table:transpose-copy' },
    { type: 'command', commandId: 'table:transpose-paste' },
    { type: 'separator' },
    { type: 'command', commandId: 'table:border-each', label: '셀 테두리/배경 - 각 셀마다 적용(E)...' },
    { type: 'command', commandId: 'table:border-one', label: '셀 테두리/배경 - 하나의 셀처럼 적용(Z)...' },
    { type: 'separator' },
    { type: 'command', commandId: 'table:caption-toggle', label: '캡션 넣기(A)' },
    { type: 'separator' },
    { type: 'command', commandId: 'table:formula', label: '계산식(F)...' },
    { type: 'separator' },
    { type: 'command', commandId: 'table:split', label: '표 나누기' },
    { type: 'command', commandId: 'table:attach', label: '표 붙이기' },
    { type: 'command', commandId: 'table:delete' },
  ];
}

/** 일반 컨텍스트 메뉴 항목 */
export function getDefaultContextMenuItems(this: any): ContextMenuItem[] {
  return [
    { type: 'command', commandId: 'edit:cut' },
    { type: 'command', commandId: 'edit:copy' },
    { type: 'command', commandId: 'edit:paste' },
    { type: 'command', commandId: 'edit:format-copy' },
    { type: 'command', commandId: 'edit:format-paste' },
    { type: 'command', commandId: 'table:transpose-paste' },
    { type: 'separator' },
    { type: 'command', commandId: 'format:char-shape', label: '글자 모양' },
    { type: 'command', commandId: 'format:para-shape', label: '문단 모양' },
    { type: 'separator' },
    { type: 'command', commandId: 'format:para-num-shape', label: '문단 번호 모양(N)...' },
  ];
}
