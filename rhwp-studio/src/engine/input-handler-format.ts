/** input-handler char/para formatting methods — extracted from InputHandler class */
/* eslint-disable @typescript-eslint/no-explicit-any */

import { CursorState } from './cursor';
import { ApplyCharFormatCommand, ApplyParaFormatCommand, applyCharShapeModsToRange } from './command';
import type { ParaFormatTarget } from './command';
import { selectCellIndicesInRange, paraFormatTargetsForCellBlock, withCellPathTarget } from './cell-block-format';
import type { SelectedCellBlock } from './cell-block-format';
import type { DocumentPosition, CharProperties, ParaProperties, CursorRect, CellBbox } from '@/core/types';
import type { WasmBridge } from '@/core/wasm-bridge';
import { computeHangingIndentPx } from './hanging-indent';

const PX_TO_RAW_2X = 150;

function pxToRaw2x(px: number): number {
  return Math.round(px * PX_TO_RAW_2X);
}

/** 선택 범위에 글자 서식을 적용한다. 선택이 없으면 캐럿 대기 서식으로 예약한다. */
export function applyCharFormat(this: any, props: Partial<CharProperties>): void {
  // [#4271 리뷰] cursor.getPosition() 은 머리말/꼬리말·각주 모드 진입 전 본문 위치에
  // 고정돼(Cursor 편집 위치는 hfCharOffset/fnCharOffset 로 별도 추적) 예약 앵커로 쓸 수
  // 없고, 전용 삽입 분기(insertTextInHeaderFooter/insertTextInFootnote)도 예약을 소비하지
  // 않는다 — 그대로 두면 이 모드에서 고른 서식이 모드를 나온 뒤 본문으로 샌다. 아직 지원
  // 범위 밖이므로 예약 자체를 차단한다.
  if (this.cursor.isInHeaderFooter() || this.cursor.isInFootnote()) return;
  const block = this.getSelectedCellBlock();
  if (block) {
    // F5 블록에서 Ctrl+클릭으로 모든 셀을 제외한 경우다. 빈 블록을 일반 텍스트
    // 선택 없음으로 fallback하면 앵커 셀 하나를 바꾸므로, history도 만들지 않고 끝낸다.
    if (block.cellIndices.length === 0) return;
    this.applyCharFormatToCellBlock(block, props);
    return;
  }
  // [#4162] getSelectionOrdered() 는 anchor 만 있어도(빈 range) non-null 을 돌려줘
  // ApplyCharFormatCommand 가 to<=from 으로 조용히 no-op 됐다. 실제 범위가 있을 때만
  // 즉시 적용하고, 그 외(선택 없음/빈 선택)는 한컴처럼 다음 삽입 런에 예약한다.
  const sel = this.getNonEmptySelection();
  if (!sel) {
    this.stagePendingCharShape(props);
    return;
  }
  const cmd = new ApplyCharFormatCommand(sel.start, sel.end, props);
  this.executeOperation({ kind: 'command', command: cmd });
}

/** [#4162][#4271 리뷰] 선택 없이 지정한 글자 서식을 다음 삽입 런에 적용하도록 예약한다.
 *
 * 새 props 를 병합하기 전에 getPendingCharShape() 로 낡은 예약을 먼저 걷어낸다 — 안 그러면
 * A 에서 예약한 서식이 B 로 캐럿이 실제로 이동한 뒤에도 raw 필드에 남아 있다가, B 에서
 * 새로 예약할 때 그대로 병합돼(굵게@A + 색@B) 요청한 적 없는 서식이 B 로 샌다. */
export function stagePendingCharShape(this: any, props: Partial<CharProperties>): void {
  this.getPendingCharShape();
  this.pendingCharShape = { ...this.pendingCharShape, ...props };
  this.pendingCharShapeAnchor = this.cursor.getPosition();
}

/**
 * 예약된 캐럿 대기 서식을 반환한다. 캐럿이 예약 지점에서 실제로 벗어났으면(탐색·클릭 등
 * 진짜 이동) 예약을 버리고 undefined 를 돌려준다 — 매 이동 지점을 일일이 후킹하는 대신
 * 조회 시점에 위치를 대조하는 지연 검증이다.
 *
 * [#4271 리뷰 후속] 머리말/꼬리말·각주 모드 중에는 앵커가 그대로 유효해도(진입 전 본문
 * 위치와 cursor.getPosition() 이 여전히 같으므로) undefined 를 돌려준다. IME 조합 소비
 * 경로(applyPendingCharShapeToRange)는 모드를 가리지 않고 이 값을 그대로 실제 wasm 범위
 * 적용에 쓰는데, 그 범위는 hfCharOffset/fnCharOffset(모드 내부 오프셋)을 본문 charOffset
 * 인 것처럼 anchor 에 실어 온다 — 걸러내지 않으면 모드 진입 직전 본문에서 예약한 서식이
 * 엉뚱한 본문 오프셋에 실제로 적용된다. 예약 자체는 지우지 않으므로, 모드에 들어갔다
 * 나오기만 하고 진짜 이동이 없었으면(캐럿이 예약 지점 그대로면) 본문 삽입에는 여전히
 * 정상 적용된다.
 */
export function getPendingCharShape(this: any): Partial<CharProperties> | undefined {
  if (!this.pendingCharShape || !this.pendingCharShapeAnchor) return undefined;
  if (this.cursor.isInHeaderFooter() || this.cursor.isInFootnote()) return undefined;
  if (CursorState.comparePositions(this.cursor.getPosition(), this.pendingCharShapeAnchor) !== 0) {
    this.pendingCharShape = null;
    this.pendingCharShapeAnchor = null;
    return undefined;
  }
  return this.pendingCharShape;
}

/** [#4162] Command 를 거치지 않는 삽입(IME 조합)에 예약 서식을 직접 적용한다. */
export function applyPendingCharShapeToRange(this: any, anchor: DocumentPosition, count: number): void {
  const props = this.getPendingCharShape();
  if (!props) return;
  const to = anchor.charOffset + count;
  applyCharShapeModsToRange(this.wasm, anchor, anchor.charOffset, to, props);
  this.advancePendingCharShapeAnchor(anchor, { ...anchor, charOffset: to });
}

/**
 * [#4162][#4271 리뷰 후속] 삽입으로 캐럿이 전진한 것은 "이동"이 아니므로 예약을 새
 * 위치로 이어간다 — 단, 이번 삽입이 실제로 예약 지점(oldPos)에서 시작했을 때만이다.
 *
 * `desc.command.type === 'insertText'`이기만 하면 호출부(executeOperation)가 무조건
 * 이 메서드를 부르는데, 붙여넣기(pastePlainText)처럼 예약 서식과 무관한 삽입도
 * `insertText` 타입이다. raw `pendingCharShape` 필드만 보고(옛 구현) 무조건 새 위치로
 * 옮기면, A 에서 예약한 뒤 커서가 실제로 C 로 이동해(예약은 이미 낡았지만 아직
 * getPendingCharShape() 로 걸러진 적 없어 필드엔 남아 있는 상태) C 에서 서식과 무관한
 * 삽입(붙여넣기 등)을 해도 그 예약이 삽입 뒤 캐럿 위치로 그대로 딸려가 살아난다.
 * oldPos 가 예약 지점과 다르면 이미 낡은 것이므로 이어가지 않고 버린다.
 */
export function advancePendingCharShapeAnchor(this: any, oldPos: DocumentPosition, newPos: DocumentPosition): void {
  if (!this.pendingCharShape || !this.pendingCharShapeAnchor) return;
  if (CursorState.comparePositions(oldPos, this.pendingCharShapeAnchor) !== 0) {
    this.pendingCharShape = null;
    this.pendingCharShapeAnchor = null;
    return;
  }
  this.pendingCharShapeAnchor = { ...newPos };
}

/**
 * 셀 블록 안 모든 셀의 모든 문단 전체 범위에 글자 서식을 적용한다.
 *
 * ApplyCharFormatCommand 는 한 셀 안의 문단만 순회한다(cellPathJsonForPara 가 start 의
 * 셀 경로를 재사용). 여러 셀에 걸친 글자 서식 커맨드가 없어서, 같은 블록을 대상으로 하는
 * applyCopiedCellPropsToSelection 과 같은 스냅샷 경로를 쓴다.
 * 근본 해결: ParaFormatEntry 에 셀 좌표를 실어 ApplyCharFormatCommand 가 셀 목록을
 * 받게 하면 셀별 charShapeId 되돌리기가 되고 스냅샷이 필요 없어진다.
 *
 * 빈 문단(len 0)은 건너뛴다 — 본문 텍스트 선택에서도 ApplyCharFormatCommand 가 같은
 * 조건(to <= from)으로 건너뛴다.
 */
export function applyCharFormatToCellBlock(this: any, block: SelectedCellBlock, props: Partial<CharProperties>): void {
  const propsJson = JSON.stringify(props);
  const cursorBefore = this.cursor.getPosition();
  this.executeOperation({
    kind: 'snapshot',
    operationType: 'applyCharFormatCellBlock',
    operation: (wasm: WasmBridge) => {
      for (const cellIdx of block.cellIndices) {
        if (block.cellPath) {
          const path = block.cellPath;
          const paraCount = wasm.getCellParagraphCountByPath(block.sec, block.ppi, JSON.stringify(withCellPathTarget(path, cellIdx)));
          for (let cellParaIdx = 0; cellParaIdx < paraCount; cellParaIdx++) {
            const pathJson = JSON.stringify(withCellPathTarget(path, cellIdx, cellParaIdx));
            const len = wasm.getCellParagraphLengthByPath(block.sec, block.ppi, pathJson);
            if (len <= 0) continue;
            wasm.applyCharFormatInCellByPath(block.sec, block.ppi, pathJson, 0, len, propsJson);
          }
          continue;
        }
        const paraCount = wasm.getCellParagraphCount(block.sec, block.ppi, block.ci, cellIdx);
        for (let cellParaIdx = 0; cellParaIdx < paraCount; cellParaIdx++) {
          const len = wasm.getCellParagraphLength(block.sec, block.ppi, block.ci, cellIdx, cellParaIdx);
          if (len <= 0) continue;
          wasm.applyCharFormatInCell(block.sec, block.ppi, block.ci, cellIdx, cellParaIdx, 0, len, propsJson);
        }
      }
      return { ...cursorBefore };
    },
  });
  // [#4151] 블록 적용 경로는 텍스트 선택 경로의 "적용 → 상태 재조회·방출" 후처리를 타지
  // 않아 툴바 눌림 상태가 이전 값으로 남는다. 적용 직후 앵커 셀 기준으로 방출해 동기화한다.
  try {
    this.eventBus.emit('cursor-format-changed', this.getCharPropertiesAtCellBlockAnchor(block));
  } catch {
    // 문서 상태 경합 시 다음 캐럿 이동에서 자연 동기화
  }
}

/** [#4151] 셀 블록 서식의 토글 방향·툴바 상태 기준: 블록 첫 셀의 첫 글자 서식. */
export function getCharPropertiesAtCellBlockAnchor(this: any, block: SelectedCellBlock): CharProperties {
  if (block.cellPath) {
    const pathJson = JSON.stringify(withCellPathTarget(block.cellPath, block.cellIndices[0], 0));
    return this.wasm.getCellCharPropertiesAtByPath(block.sec, block.ppi, pathJson, 0);
  }
  return this.wasm.getCellCharPropertiesAt(block.sec, block.ppi, block.ci, block.cellIndices[0], 0, 0);
}

/** 토글 서식 적용 (상호 배타 처리 포함) */
export function applyToggleFormat(this: any, prop: 'bold' | 'italic' | 'underline' | 'strikethrough' | 'emboss' | 'engrave' | 'outline' | 'superscript' | 'subscript'): void {
  // [#4162] 선택·셀 블록이 없어도(캐럿만) applyCharFormat 이 캐럿 대기 서식으로 예약한다 —
  // 여기서 조기 종료하면 Ctrl+B 등이 다시 무언 no-op 이 된다.
  // 셀 블록에서는 앵커 셀 텍스트의 현재 값이 토글 방향을 정한다. 칸마다 값이 다를 때
  // 블록 전체를 한 방향으로 맞추려면 기준이 하나여야 하고, 텍스트 선택도 같은 기준이다.
  // [#4151] 커서 위치 조회는 셀 블록 모드에서 블록 밖(호스트 문단 등)을 읽어 방금 적용한
  // 서식이 보이지 않는다 — 두 번째 클릭이 해제가 아니라 재적용이 되는 원인. 블록 모드에선
  // 블록 첫 셀의 첫 글자 서식을 기준으로 삼는다. 빈 블록(전 셀 Ctrl+클릭 제외)은 앵커
  // 셀이 없으므로 커서 기준 폴백 — applyCharFormat 이 어차피 빈 블록에서 조기 종료한다.
  const toggleBlock = this.getSelectedCellBlock();
  const current = toggleBlock && toggleBlock.cellIndices.length > 0
    ? this.getCharPropertiesAtCellBlockAnchor(toggleBlock)
    : this.getCharPropertiesAtCursor();

  if (prop === 'emboss') {
    const newVal = !current.emboss;
    const mods: Partial<CharProperties> = { emboss: newVal };
    if (newVal) mods.engrave = false;
    this.applyCharFormat(mods);
  } else if (prop === 'engrave') {
    const newVal = !current.engrave;
    const mods: Partial<CharProperties> = { engrave: newVal };
    if (newVal) mods.emboss = false;
    this.applyCharFormat(mods);
  } else if (prop === 'outline') {
    const curOutline = current.outlineType ?? 0;
    this.applyCharFormat({ outlineType: curOutline ? 0 : 1 });
  } else if (prop === 'superscript') {
    const newVal = !current.superscript;
    const mods: Partial<CharProperties> = { superscript: newVal };
    if (newVal) mods.subscript = false;
    this.applyCharFormat(mods);
  } else if (prop === 'subscript') {
    const newVal = !current.subscript;
    const mods: Partial<CharProperties> = { subscript: newVal };
    if (newVal) mods.superscript = false;
    this.applyCharFormat(mods);
  } else {
    this.applyCharFormat({ [prop]: !current[prop] });
  }
}

/**
 * [#4162] 실제로 문자가 선택된 범위만 돌려준다. anchor 만 있고 focus 와 같은 위치
 * (빈 선택, 드래그 없이 클릭만 한 상태)는 선택 없음으로 접는다 — getSelectionOrdered()
 * 는 anchor 유무만 보고 non-null 을 돌려줘, 그대로 쓰면 서식 커맨드가 빈 range 로
 * 조용히 no-op 된다.
 */
export function getNonEmptySelection(this: any): { start: DocumentPosition; end: DocumentPosition } | null {
  const sel = this.cursor.getSelectionOrdered();
  if (!sel) return null;
  if (CursorState.comparePositions(sel.start, sel.end) === 0) return null;
  return sel;
}

/** 커서 위치의 글자 서식을 조회한다. 선택이 있으면 선택 첫 글자, 없으면 캐럿 앞 글자 기준. */
export function getCharPropertiesAtCursor(this: any): CharProperties {
  const sel = this.getNonEmptySelection();
  const pos = sel ? sel.start : this.cursor.getPosition();
  // 선택 시작 offset 은 그 자리 글자가 곧 선택 첫 글자다(offset-1 이면 선택 밖을 읽는다).
  // 선택이 없으면 offset이 0인 경우만 그 위치, 아니면 offset-1 위치(커서 앞 글자 기준).
  const queryOffset = sel ? pos.charOffset : (pos.charOffset > 0 ? pos.charOffset - 1 : 0);
  if (pos.parentParaIndex !== undefined) {
    // [#2756] 중첩 표는 최내곽 셀 대상 ...ByPath 로 조회한다. flat controlIndex/cellIndex/
    // cellParaIndex 는 hit-test 가 cellPath[0](최외곽)에서 채우므로 그대로 넘기면 **바깥
    // 셀**의 서식을 읽는다. applyToggleFormat 이 이 값에서 !current[prop] 로 토글 방향을
    // 정하므로(그리고 실제 적용 ApplyCharFormatCommand 는 이미 ...ByPath 로 안쪽 셀에
    // 적용) 방향이 어긋나 Ctrl+B/I 가 거꾸로 동작하고 툴바 표시도 오답이 된다.
    if ((pos.cellPath?.length ?? 0) > 0) {
      return this.wasm.getCellCharPropertiesAtByPath(
        pos.sectionIndex, pos.parentParaIndex, JSON.stringify(pos.cellPath), queryOffset,
      );
    }
    return this.wasm.getCellCharPropertiesAt(
      pos.sectionIndex, pos.parentParaIndex, pos.controlIndex!,
      pos.cellIndex!, pos.cellParaIndex!, queryOffset,
    );
  }
  return this.wasm.getCharPropertiesAt(pos.sectionIndex, pos.paragraphIndex, queryOffset);
}

/** 커서 위치 문단에 문단 서식을 적용한다 */
export function applyParaFormat(this: any, props: Record<string, unknown>): void {
  try {
    if (this.applyParaFormatInNoteOrHeader(props)) return;
    const targets = this.getParaFormatTargetsAtCursor();
    this.executeParaFormatCommand(targets, props);
  } catch (err) {
    console.warn('[InputHandler] applyParaFormat 실패:', err);
  }
}

/**
 * 머리말/꼬리말·각주 문단에 문단 서식을 적용한다. 해당 문맥이 아니면 false.
 *
 * 코어에는 `applyParaFormatInHf` / `applyParaFormatInFootnote` 가 이미 있는데 호출하는
 * 곳이 없었다 — `getParaFormatTargetsForRange` 가 두 문맥에서 빈 배열을 반환해 정렬·줄
 * 간격이 아무 반응 없이 끝났다. 조회 쪽(`getParaProperties`)은 두 문맥을 정확히 분기하고
 * 있어 툴바 표시만 맞고 적용은 안 되는 상태였다.
 *
 * `ApplyParaFormatCommand` 의 되돌리기는 문단 모양 ID 를 `setParaShapeId` /
 * `setCellParaShapeId` 로 복원하는데 이 두 문맥용 setter 가 코어에 없다. 되돌리기를
 * 포기하지 않으려고 표 구조 변경과 같은 스냅샷 경로를 쓴다.
 * 근본 해결: 코어에 `setParaShapeIdInHf` / `setParaShapeIdInFootnote` 를 추가하고
 * `ParaFormatTarget` 에 두 갈래를 넣어 네 문맥(본문/셀/머리말/각주)을 한 커맨드로 통일한다.
 */
export function applyParaFormatInNoteOrHeader(this: any, props: Record<string, unknown>): boolean {
  const cur = this.cursor;
  const propsJson = JSON.stringify(props);
  const cursorBefore = cur.getPosition();

  if (cur.isInHeaderFooter()) {
    const isHeader = cur.headerFooterMode === 'header';
    const sectionIdx = cur.hfSectionIdx;
    const applyTo = cur.hfApplyTo;
    const hfParaIdx = cur.hfParaIdx;
    const hfCharOffset = cur.hfCharOffset;
    this.executeOperation({
      kind: 'snapshot',
      operationType: 'applyParaFormatInHf',
      editContext: {
        mode: 'headerFooter',
        sectionIdx,
        isHeader,
        applyTo,
        paraIdx: hfParaIdx,
        charOffset: hfCharOffset,
      },
      operation: (wasm: WasmBridge) => {
        wasm.applyParaFormatInHf(sectionIdx, isHeader, applyTo, hfParaIdx, propsJson);
        return { ...cursorBefore };
      },
    });
    return true;
  }

  if (cur.isInFootnote()) {
    // 인자 축은 조회 쪽(getParaProperties)과 같다 — sec / para / controlIdx / innerParaIdx.
    const sectionIdx = cur.fnSectionIdx;
    const paraIdx = cur.fnParaIdx;
    const controlIdx = cur.fnControlIdx;
    const innerParaIdx = cur.fnInnerParaIdx;
    const charOffset = cur.fnCharOffset;
    const footnoteIndex = cur.fnFootnoteIndex;
    const pageNum = cur.fnPageNum;
    this.executeOperation({
      kind: 'snapshot',
      operationType: 'applyParaFormatInFootnote',
      editContext: {
        mode: 'footnote',
        sectionIdx,
        paraIdx,
        controlIdx,
        footnoteIndex,
        pageNum,
        innerParaIdx,
        charOffset,
      },
      operation: (wasm: WasmBridge) => {
        wasm.applyParaFormatInFootnote(sectionIdx, paraIdx, controlIdx, innerParaIdx, propsJson);
        return { ...cursorBefore };
      },
    });
    return true;
  }

  return false;
}

export function executeParaFormatCommand(this: any, targets: ParaFormatTarget[], props: Record<string, unknown>): boolean {
  if (targets.length === 0) {
    console.info('[InputHandler] 문단 서식 Undo/Redo: unsupported context');
    return false;
  }
  const cmd = new ApplyParaFormatCommand(targets, props as Partial<ParaProperties>, this.cursor.getPosition());
  this.executeOperation({ kind: 'command', command: cmd });
  return true;
}

/**
 * F5 셀 블록 선택에 든 셀 목록을 만든다. 블록 선택이 아니면 null.
 *
 * 셀 블록 선택은 cellAnchor/cellFocus 축이라 텍스트 선택(anchor)을 만들지 않는다.
 * 그래서 서식 경로가 getSelectionOrdered() 만 보면 커서가 있는 앵커 셀 하나만 대상이
 * 된다 — 여러 칸을 골라도 첫 칸만 바뀌는 증상.
 *
 * 셀 산출 축은 같은 블록을 대상으로 하는 applyCopiedCellPropsToSelection 과 같게 맞춘다
 * (getCellTableContext + getSelectedCellRange + getExcludedCells, 중첩 표 제외).
 */
export function getSelectedCellBlock(this: any): SelectedCellBlock | null {
  if (!this.cursor.isInCellSelectionMode()) return null;
  const ctx = this.cursor.getCellTableContext();
  const range = this.cursor.getSelectedCellRange();
  if (!ctx || !range) return null;
  const excluded = this.cursor.getExcludedCells();

  if (ctx.cellPath && ctx.cellPath.length > 1) {
    const path = ctx.cellPath;
    const dims = this.wasm.getTableDimensionsByPath(ctx.sec, ctx.ppi, JSON.stringify(path));
    const cellIndices = selectCellIndicesInRange(
      dims.cellCount,
      (cellIdx) => this.wasm.getCellInfoByPath(ctx.sec, ctx.ppi, JSON.stringify(withCellPathTarget(path, cellIdx))),
      range,
      excluded,
    );
    return { sec: ctx.sec, ppi: ctx.ppi, ci: ctx.ci, cellIndices, cellPath: path };
  }

  const dims = this.wasm.getTableDimensions(ctx.sec, ctx.ppi, ctx.ci);
  const cellIndices = selectCellIndicesInRange(
    dims.cellCount,
    (cellIdx) => this.wasm.getCellInfo(ctx.sec, ctx.ppi, ctx.ci, cellIdx),
    range,
    excluded,
  );
  return { sec: ctx.sec, ppi: ctx.ppi, ci: ctx.ci, cellIndices };
}

export function getParaFormatTargetsAtCursor(this: any): ParaFormatTarget[] {
  const block = this.getSelectedCellBlock();
  if (block) return this.getParaFormatTargetsForCellBlock(block);
  const sel = this.cursor.getSelectionOrdered();
  if (sel) return this.getParaFormatTargetsForRange(sel.start, sel.end);
  const pos = this.cursor.getPosition();
  return this.getParaFormatTargetsForRange(pos, pos);
}

/** 셀 블록 안 모든 셀의 모든 문단을 문단 서식 대상으로 만든다 */
export function getParaFormatTargetsForCellBlock(this: any, block: SelectedCellBlock): ParaFormatTarget[] {
  // 중첩 표 문단 서식은 목표 밖(getParaFormatTargetsForRange 도 동일 하계)이다.
  if (block.cellPath) return [];
  return paraFormatTargetsForCellBlock(
    block,
    (cellIdx) => this.wasm.getCellParagraphCount(block.sec, block.ppi, block.ci, cellIdx),
  );
}

export function getParaFormatTargetsForRange(this: any, start: DocumentPosition, end: DocumentPosition): ParaFormatTarget[] {
  if (this.cursor.isInHeaderFooter() || this.cursor.isInFootnote()) return [];
  if (start.isTextBox || end.isTextBox) return [];
  if ((start.cellPath?.length ?? 0) > 1 || (end.cellPath?.length ?? 0) > 1) return [];

  const startInCell = start.parentParaIndex !== undefined;
  const endInCell = end.parentParaIndex !== undefined;
  if (startInCell || endInCell) {
    if (!startInCell || !endInCell) return [];
    if (start.sectionIndex !== end.sectionIndex) return [];
    if (start.parentParaIndex !== end.parentParaIndex) return [];
    const startPath = start.cellPath?.[0];
    const endPath = end.cellPath?.[0];
    const startControl = startPath?.controlIndex ?? start.controlIndex;
    const endControl = endPath?.controlIndex ?? end.controlIndex;
    const startCell = startPath?.cellIndex ?? start.cellIndex;
    const endCell = endPath?.cellIndex ?? end.cellIndex;
    const startCellPara = startPath?.cellParaIndex ?? start.cellParaIndex;
    const endCellPara = endPath?.cellParaIndex ?? end.cellParaIndex;
    if (
      startControl === undefined ||
      endControl === undefined ||
      startCell === undefined ||
      endCell === undefined ||
      startCellPara === undefined ||
      endCellPara === undefined ||
      startControl !== endControl ||
      startCell !== endCell
    ) {
      return [];
    }
    const from = Math.min(startCellPara, endCellPara);
    const to = Math.max(startCellPara, endCellPara);
    const targets: ParaFormatTarget[] = [];
    for (let cp = from; cp <= to; cp++) {
      targets.push({
        kind: 'cell',
        sec: start.sectionIndex,
        parentPara: start.parentParaIndex!,
        controlIdx: startControl,
        cellIdx: startCell,
        cellParaIdx: cp,
      });
    }
    return targets;
  }

  if (start.sectionIndex !== end.sectionIndex) return [];
  const from = Math.min(start.paragraphIndex, end.paragraphIndex);
  const to = Math.max(start.paragraphIndex, end.paragraphIndex);
  const targets: ParaFormatTarget[] = [];
  for (let p = from; p <= to; p++) {
    targets.push({ kind: 'body', sec: start.sectionIndex, para: p });
  }
  return targets;
}

/** 한컴식 Shift+Tab: 첫 줄 시작 위치를 기준으로 문단 내어쓰기를 설정한다. */
export function applyHangingIndentAtCursor(this: any): boolean {
  if (this.cursor.isInHeaderFooter() || this.cursor.isInFootnote()) {
    console.info('[InputHandler] Shift+Tab hanging indent: unsupported note/header context');
    return false;
  }

  const pos = this.cursor.getPosition();
  if (pos.isTextBox || (pos.cellPath?.length ?? 0) > 1) {
    console.info('[InputHandler] Shift+Tab hanging indent: unsupported nested/textbox context');
    return false;
  }

  try {
    let cursorRect: CursorRect | null = this.cursor.getRect();
    let firstLineStartRect: CursorRect;

    if (pos.parentParaIndex !== undefined) {
      const pathEntry = pos.cellPath?.[0];
      const controlIndex = pathEntry?.controlIndex ?? pos.controlIndex;
      const cellIndex = pathEntry?.cellIndex ?? pos.cellIndex;
      const cellParaIndex = pathEntry?.cellParaIndex ?? pos.cellParaIndex;

      if (controlIndex === undefined || cellIndex === undefined || cellParaIndex === undefined) {
        console.warn('[InputHandler] Shift+Tab hanging indent: incomplete cell position', pos);
        return false;
      }

      const firstLineInfo = this.wasm.getLineInfoInCell(
        pos.sectionIndex,
        pos.parentParaIndex,
        controlIndex,
        cellIndex,
        cellParaIndex,
        0,
      );

      if (pos.cellPath?.length === 1) {
        const pathJson = JSON.stringify(pos.cellPath);
        firstLineStartRect = this.wasm.getCursorRectByPath(
          pos.sectionIndex,
          pos.parentParaIndex,
          pathJson,
          firstLineInfo.charStart,
        );
        cursorRect ??= this.wasm.getCursorRectByPath(
          pos.sectionIndex,
          pos.parentParaIndex,
          pathJson,
          pos.charOffset,
        ) as CursorRect;
      } else {
        firstLineStartRect = this.wasm.getCursorRectInCell(
          pos.sectionIndex,
          pos.parentParaIndex,
          controlIndex,
          cellIndex,
          cellParaIndex,
          firstLineInfo.charStart,
        );
        cursorRect ??= this.wasm.getCursorRectInCell(
          pos.sectionIndex,
          pos.parentParaIndex,
          controlIndex,
          cellIndex,
          cellParaIndex,
          pos.charOffset,
        ) as CursorRect;
      }

      const hangingPx = computeHangingIndentPx(cursorRect.x, firstLineStartRect.x);
      this.executeParaFormatCommand(
        [{
          kind: 'cell',
          sec: pos.sectionIndex,
          parentPara: pos.parentParaIndex,
          controlIdx: controlIndex,
          cellIdx: cellIndex,
          cellParaIdx: cellParaIndex,
        }],
        { indent: -pxToRaw2x(hangingPx) },
      );
      return true;
    }

    const firstLineInfo = this.wasm.getLineInfo(pos.sectionIndex, pos.paragraphIndex, 0);
    firstLineStartRect = this.wasm.getCursorRect(
      pos.sectionIndex,
      pos.paragraphIndex,
      firstLineInfo.charStart,
    );
    cursorRect ??= this.wasm.getCursorRect(pos.sectionIndex, pos.paragraphIndex, pos.charOffset) as CursorRect;

    const hangingPx = computeHangingIndentPx(cursorRect.x, firstLineStartRect.x);
    this.executeParaFormatCommand(
      [{ kind: 'body', sec: pos.sectionIndex, para: pos.paragraphIndex }],
      { indent: -pxToRaw2x(hangingPx) },
    );
    return true;
  } catch (err) {
    console.warn('[InputHandler] Shift+Tab hanging indent 실패:', err);
    return false;
  }
}

/** 커서 위치 서식 상태를 Toolbar에 알린다 */
export function emitCursorFormatState(this: any): void {
  if (!this.active) return;
  try {
    const props = this.getCharPropertiesAtCursor();
    this.eventBus.emit('cursor-format-changed', props);
  } catch {
    // 문서 없거나 위치 초과 시 무시
  }
  // 문단 속성 (눈금자 마커용) + 스타일
  try {
    const pos = this.cursor.getPosition();
    const inFootnote = this.cursor.isInFootnote();
    const inCell = !inFootnote && pos.parentParaIndex !== undefined;
    // 문단 모양 대화상자와 같은 리더를 쓴다. 여기에 갈래를 따로 두면 문맥이 하나 빠져도
    // 컴파일이 통과하고, 실제로 머리말/꼬리말 갈래가 빠져 있었다 — 머리말 편집 중 툴바와
    // 눈금자가 본문 문단 값을 보여줬다(대화상자는 머리말 값을 정확히 읽는데).
    const paraProps = this.getParaProperties();
    this.eventBus.emit('cursor-para-changed', paraProps);

    // 스타일 드롭다운 갱신용
    try {
      const styleInfo = inCell
        ? this.wasm.getCellStyleAt(
            pos.sectionIndex, pos.parentParaIndex!, pos.controlIndex!,
            pos.cellIndex!, pos.cellParaIndex!,
          )
        : this.wasm.getStyleAt(pos.sectionIndex, pos.paragraphIndex);
      this.eventBus.emit('cursor-style-changed', styleInfo);
    } catch { /* 스타일 조회 실패 시 무시 */ }

    // 셀 영역 정보 (눈금자 셀 너비 표시용)
    // getTableCellBboxes는 대형/중첩 표에서 수 초 동안 main thread를 막을 수 있다.
    // 일반 커서 이동/텍스트 입력 경로에서는 새 bbox 조회를 하지 않고, 표 hover/resize 경로에서
    // 이미 확보한 캐시가 있을 때만 재사용한다.
    if (inCell) {
      const cellKey = `${pos.sectionIndex}:${pos.parentParaIndex}:${pos.controlIndex}:${pos.cellIndex}`;
      if (cellKey !== this.lastCellKey) {
        this.lastCellKey = cellKey;
        const sec = pos.sectionIndex;
        const ppi = pos.parentParaIndex!;
        const ci = pos.controlIndex!;
        const cellIdx = pos.cellIndex!;
        const cached = this.cachedTableRef?.sec === sec
          && this.cachedTableRef.ppi === ppi
          && this.cachedTableRef.ci === ci
          ? this.cachedCellBboxes
          : null;
        const bbox = cached?.find((b: CellBbox) => b.cellIdx === cellIdx);
        if (bbox) {
          this.eventBus.emit('cursor-cell-changed', {
            inCell: true, cellX: bbox.x, cellWidth: bbox.w,
          });
        } else {
          this.eventBus.emit('cursor-cell-changed', { inCell: false });
        }
      }
    } else if (this.lastCellKey !== null) {
      this.lastCellKey = null;
      this.eventBus.emit('cursor-cell-changed', { inCell: false });
    }
  } catch {
    // 무시
  }
}
