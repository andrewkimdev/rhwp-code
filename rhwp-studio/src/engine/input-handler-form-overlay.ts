/** input-handler form-object overlay methods — extracted from InputHandler class */
/* eslint-disable @typescript-eslint/no-explicit-any */

import { SetFormValueCommand } from './command';
import type { FormValueTarget } from './command';
import type { FormObjectHitResult } from '@/core/types';

/**
 * [Task #2374] 이미 적용된 양식 값 변경을 역연산 커맨드로 기록한다(no-op 제외).
 * 미기록 시 이후 스냅샷 undo 가 값 변경 이전 문서를 복원해 양식 값을 무언 파괴한다
 * (#2337 계급). 양식 모드에서는 snapshot 이 게이트에서 드롭되므로 record 가 유일한
 * 기록 경로다. before==after(이미 선택된 라디오 재클릭 등)는 유령 엔트리 방지를 위해
 * 기록하지 않는다.
 */
export function recordFormValueChanges(this: any, targets: FormValueTarget[]): void {
  const changed = targets.filter((t) => t.beforeJson !== t.afterJson);
  if (changed.length === 0) return;
  this.executeOperation({
    kind: 'record',
    command: new SetFormValueCommand(changed, this.cursor.getPosition()),
  });
}

/**
 * 셀 내부 컨트롤 locator (뮤테이션 분기와 record 대상이 같은 조건을 공유).
 *
 * 셀 안 양식 개체는 hit 결과의 para 가 "표를 담은 최상위 문단" 이고 ci 는 "셀 문단 안의
 * 컨트롤 인덱스" 다(form_query.rs get_form_object_at_native). 따라서 flat
 * setFormValue(sec, para, ci) 로 쓰면 표 컨트롤 슬롯을 가리켜 항상 실패한다
 * (set_form_value_native 의 `not a form object`). 셀 안이면 반드시 이 locator 로
 * setFormValueInCell 을 쓰고, 기록에도 inCell 을 실어야 undo 가 같은 슬롯을 되돌린다.
 */
export function formInCellLoc(this: any, formHit: FormObjectHitResult):
  { tablePara: number; tableCi: number; cellIdx: number; cellPara: number } | undefined {
  return (formHit.inCell && formHit.tablePara !== undefined && formHit.tableCi !== undefined
      && formHit.cellIdx !== undefined && formHit.cellPara !== undefined)
    ? { tablePara: formHit.tablePara, tableCi: formHit.tableCi, cellIdx: formHit.cellIdx, cellPara: formHit.cellPara }
    : undefined;
}

/** 양식 개체 클릭 처리 */
export function handleFormObjectClick(this: any, formHit: FormObjectHitResult, pageIdx: number, _zoom: number): void {
  if (!formHit.found || formHit.sec === undefined || formHit.para === undefined || formHit.ci === undefined) return;

  const { sec, para, ci, formType } = formHit;

  const inCellLoc = this.formInCellLoc(formHit);

  // 셀 내부 폼 값 설정 헬퍼
  const setFormVal = (valueJson: string) => {
    if (inCellLoc) {
      this.wasm.setFormValueInCell(sec, inCellLoc.tablePara, inCellLoc.tableCi,
        inCellLoc.cellIdx, inCellLoc.cellPara, ci, valueJson);
    } else {
      this.wasm.setFormValue(sec, para, ci, valueJson);
    }
  };

  switch (formType) {
    case 'CheckBox': {
      // 체크박스 토글: value 0↔1
      const oldValue = formHit.value ?? 0;
      const newValue = oldValue === 0 ? 1 : 0;
      const afterJson = JSON.stringify({ value: newValue });
      setFormVal(afterJson);
      this.recordFormValueChanges([{
        sec, para, ci, inCell: inCellLoc,
        beforeJson: JSON.stringify({ value: oldValue }),
        afterJson,
      }]);
      this.afterEdit();
      break;
    }
    case 'RadioButton': {
      // 라디오 버튼: 같은 그룹 내 다른 라디오 버튼 해제 후 선택
      this.handleRadioButtonClick(sec, para, ci);
      break;
    }
    case 'PushButton': {
      // 명령 단추: 웹 환경에서는 보안상 비활성 (클릭 무시)
      break;
    }
    case 'ComboBox': {
      this.showComboBoxOverlay(sec, para, ci, formHit, pageIdx);
      break;
    }
    case 'Edit': {
      this.showEditOverlay(sec, para, ci, formHit, pageIdx);
      break;
    }
  }
}

/** 라디오 버튼 클릭: 같은 그룹 내 다른 라디오 버튼 해제 */
export function handleRadioButtonClick(this: any, sec: number, para: number, ci: number): void {
  // 현재 클릭된 라디오 버튼의 그룹 이름 조회
  const info = this.wasm.getFormObjectInfo(sec, para, ci);
  if (!info.ok) return;

  const groupName = info.properties?.['GroupName'] ?? '';
  // [Task #2374] 그룹 해제+선택은 다중 쓰기 — 이전 값을 캡처해 1 엔트리로 원자 기록
  // (개별 기록 시 undo 가 해제만 복원하는 반쪽 상태를 만든다).
  const changes: FormValueTarget[] = [];

  // 같은 문단 내 다른 라디오 버튼 찾아서 해제
  // (HWP 양식에서 라디오 버튼은 보통 같은 문단에 배치됨)
  const section = sec;
  // 동일 문단의 모든 컨트롤을 순회하여 같은 그룹의 라디오 버튼 해제
  for (let i = 0; i < 50; i++) { // 최대 50개 컨트롤 검사
    if (i === ci) continue;
    const otherInfo = this.wasm.getFormObjectInfo(section, para, i);
    if (!otherInfo.ok || otherInfo.formType !== 'RadioButton') continue;
    const otherGroup = otherInfo.properties?.['GroupName'] ?? '';
    if (otherGroup === groupName && otherInfo.value !== 0) {
      this.wasm.setFormValue(section, para, i, JSON.stringify({ value: 0 }));
      changes.push({
        sec: section, para, ci: i,
        beforeJson: JSON.stringify({ value: otherInfo.value }),
        afterJson: JSON.stringify({ value: 0 }),
      });
    }
  }

  // 클릭된 라디오 버튼 선택
  this.wasm.setFormValue(sec, para, ci, JSON.stringify({ value: 1 }));
  changes.push({
    sec, para, ci,
    beforeJson: JSON.stringify({ value: info.value ?? 0 }),
    afterJson: JSON.stringify({ value: 1 }),
  });
  this.recordFormValueChanges(changes);
  this.afterEdit();
}

/** 양식 개체 bbox를 scroll-content 내 절대 좌표로 변환 */
export function formBboxToOverlayRect(this: any, bbox: { x: number; y: number; w: number; h: number }, pageIdx: number): { left: number; top: number; width: number; height: number } {
  const zoom = this.viewportManager.getZoom();
  const pageOffset = this.virtualScroll.getPageOffset(pageIdx);
  const scrollContent = this.container.querySelector('#scroll-content');
  const contentWidth = scrollContent?.clientWidth ?? 0;
  const pageLeft = this.virtualScroll.getPageLeftResolved(pageIdx, contentWidth);

  return {
    left: pageLeft + bbox.x * zoom,
    top: pageOffset + bbox.y * zoom,
    width: bbox.w * zoom,
    height: bbox.h * zoom,
  };
}

/** 기존 양식 오버레이 제거 */
export function removeFormOverlay(this: any): void {
  if (this.formOverlay) {
    try { this.formOverlay.remove(); } catch { /* 이미 제거됨 */ }
    this.formOverlay = null;
  }
}

/** ComboBox 드롭다운 오버레이 */
export function showComboBoxOverlay(this: any, sec: number, para: number, ci: number, formHit: FormObjectHitResult, pageIdx: number): void {
  this.removeFormOverlay();
  if (!formHit.bbox) return;

  const info = this.wasm.getFormObjectInfo(sec, para, ci);
  if (!info.ok) return;

  // 항목 목록: 스크립트 InsertString 추출 결과 (WASM에서 제공)
  const items: string[] = info.items ?? [];
  const currentText = formHit.text ?? '';

  if (items.length === 0) {
    // 항목 없으면 Edit 오버레이로 대체
    this.showEditOverlay(sec, para, ci, formHit, pageIdx);
    return;
  }

  const rect = this.formBboxToOverlayRect(formHit.bbox, pageIdx);
  const fontSize = Math.max(rect.height * 0.6, 10);
  const itemHeight = fontSize * 1.6;

  // 컨테이너 (콤보박스 위치에 드롭다운 리스트 표시)
  const dropdown = document.createElement('div');
  dropdown.className = 'form-combo-dropdown';
  dropdown.style.left = `${rect.left}px`;
  dropdown.style.top = `${rect.top + rect.height}px`;
  dropdown.style.width = `${rect.width}px`;

  for (const item of items) {
    const row = document.createElement('div');
    row.className = 'form-combo-item' + (item === currentText ? ' selected' : '');
    row.textContent = item;
    row.style.fontSize = `${fontSize}px`;
    row.style.lineHeight = `${itemHeight}px`;
    row.addEventListener('mousedown', (e) => {
      e.preventDefault();
      this.wasm.setFormValue(sec, para, ci, JSON.stringify({ text: item }));
      // [Task #2374] 콤보 선택 기록(동일 항목 재선택은 no-op 제외).
      this.recordFormValueChanges([{
        sec, para, ci,
        beforeJson: JSON.stringify({ text: currentText }),
        afterJson: JSON.stringify({ text: item }),
      }]);
      this.removeFormOverlay();
      this.afterEdit();
    });
    dropdown.appendChild(row);
  }

  // 외부 클릭 시 닫기
  const onDocClick = (e: MouseEvent) => {
    if (!dropdown.contains(e.target as Node)) {
      this.removeFormOverlay();
      document.removeEventListener('mousedown', onDocClick, true);
    }
  };
  // 다음 프레임에 등록 (현재 클릭 이벤트 무시)
  requestAnimationFrame(() => {
    document.addEventListener('mousedown', onDocClick, true);
  });

  const scrollContent = this.container.querySelector('#scroll-content');
  (scrollContent ?? this.container).appendChild(dropdown);
  this.formOverlay = dropdown;
}

/** Edit 입력 오버레이 */
export function showEditOverlay(this: any, sec: number, para: number, ci: number, formHit: FormObjectHitResult, pageIdx: number): void {
  this.removeFormOverlay();
  if (!formHit.bbox) return;

  const rect = this.formBboxToOverlayRect(formHit.bbox, pageIdx);

  const input = document.createElement('input');
  input.type = 'text';
  input.value = formHit.text ?? '';
  input.className = 'form-edit-input';
  input.style.left = `${rect.left}px`;
  input.style.top = `${rect.top}px`;
  input.style.width = `${rect.width}px`;
  input.style.height = `${rect.height}px`;
  input.style.fontSize = `${rect.height * 0.6}px`;

  // Enter 커밋의 오버레이 제거가 blur 커밋을 재유발해도 이중 적용·이중 기록되지 않게 1회 가드.
  let committed = false;
  const commit = () => {
    if (committed) return;
    committed = true;
    // 셀 안 Edit 필드는 flat setFormValue 로 쓰면 표 컨트롤 슬롯을 가리켜 조용히 실패한다
    // (CheckBox 분기와 동일 조건 — formInCellLoc 참고). 기록에도 inCell 을 실어야 undo 가
    // 같은 슬롯을 되돌린다(SetFormValueCommand.apply 가 inCell 로 분기).
    const inCellLoc = this.formInCellLoc(formHit);
    const afterJson = JSON.stringify({ text: input.value });
    if (inCellLoc) {
      this.wasm.setFormValueInCell(sec, inCellLoc.tablePara, inCellLoc.tableCi,
        inCellLoc.cellIdx, inCellLoc.cellPara, ci, afterJson);
    } else {
      this.wasm.setFormValue(sec, para, ci, afterJson);
    }
    // [Task #2374] 편집 필드 커밋 기록(동일 텍스트는 no-op 제외).
    this.recordFormValueChanges([{
      sec, para, ci, inCell: inCellLoc,
      beforeJson: JSON.stringify({ text: formHit.text ?? '' }),
      afterJson,
    }]);
    this.removeFormOverlay();
    this.afterEdit();
  };

  input.addEventListener('keydown', (e) => {
    if (e.key === 'Enter') {
      e.preventDefault();
      commit();
    } else if (e.key === 'Escape') {
      e.preventDefault();
      // 취소는 blur가 뒤따라도 값을 적용하거나 히스토리를 기록하지 않아야 한다.
      committed = true;
      this.removeFormOverlay();
    }
  });
  input.addEventListener('blur', () => {
    commit();
  });

  const scrollContent = this.container.querySelector('#scroll-content');
  (scrollContent ?? this.container).appendChild(input);
  this.formOverlay = input;

  requestAnimationFrame(() => {
    input.focus();
    input.select();
  });
}
