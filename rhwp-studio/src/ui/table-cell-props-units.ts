/** 표/셀 속성 대화상자 공유 단위 변환·미리보기 상수 (table-cell-props-dialog.ts에서 분리) */

const HWPUNIT_PER_MM = 7200 / 25.4;

export function hwpunitToMm(hu: number): number {
  return Math.round(hu * 25.4 / 7200 * 10) / 10;
}

export function mmToHwpunit(mm: number): number {
  return Math.round(mm * HWPUNIT_PER_MM);
}

/** HWP16 (i16) → mm */
export function hwp16ToMm(hu: number): number {
  return Math.round(hu * 25.4 / 7200 * 10) / 10;
}

export function mmToHwp16(mm: number): number {
  return Math.round(mm * HWPUNIT_PER_MM);
}

export const DOC_PAPER_COLOR = 'var(--doc-paper)';
export const PREVIEW_GUIDE_STROKE = 'var(--ui-border-light)';
export const LINE_SAMPLE_STROKE = 'currentColor';
