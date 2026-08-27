import type { RemovedParaMeta, WasmBridge } from '@/core/wasm-bridge';
import type { DocumentPosition, CharProperties, ParaProperties } from '@/core/types';
import type { EditCommand } from './types';
import { cellParaIndexOf, cellPathJsonForPara, isCell } from './cell-path';

// ─── 글자 서식 적용 명령 ─────────────────────────────

/** 문단 하나에 대한 서식 적용 정보 */
interface ParaFormatEntry {
  paraIndex: number;       // 본문: paragraphIndex, 셀: cellParaIndex
  startOffset: number;
  endOffset: number;
  /** undo용: 적용 전 charShapeId */
  beforeCharShapeId?: number;
  /** redo용: 적용 후 charShapeId */
  afterCharShapeId?: number;
}

export class ApplyCharFormatCommand implements EditCommand {
  readonly type = 'applyCharFormat';
  readonly timestamp = Date.now();

  private entries: ParaFormatEntry[] = [];

  constructor(
    private start: DocumentPosition,
    private end: DocumentPosition,
    private props: Partial<CharProperties>,
  ) {}

  execute(wasm: WasmBridge): DocumentPosition {
    if (this.entries.length > 0 && this.entries.every((entry) => entry.afterCharShapeId !== undefined)) {
      this.restoreCharShapeIds(wasm, 'after');
      return { ...this.start };
    }

    const { start, end } = this;
    const propsJson = JSON.stringify(this.props);

    if (isCell(start)) {
      // 중첩 셀 좌표 축 정합: flat controlIndex/cellIndex 는 cellPath[0](최외곽)이라 중첩
      // 셀에서 바깥 셀에 서식을 적용한다. 최내곽 셀을 ...ByPath 로 라우팅하고 문단 인덱스는
      // cellPath[last](cellParaIndexOf)에서 읽는다. undo(restoreCharShapeIds)도 같은 축.
      const sec = start.sectionIndex;
      const ppi = start.parentParaIndex!;
      const startPara = cellParaIndexOf(start);
      const endPara = cellParaIndexOf(end);

      this.entries = [];
      for (let p = startPara; p <= endPara; p++) {
        const pathP = cellPathJsonForPara(start, p);
        const from = p === startPara ? start.charOffset : 0;
        const to = p === endPara ? end.charOffset : wasm.getCellParagraphLengthByPath(sec, ppi, pathP);
        if (to <= from) continue;

        const prevProps = wasm.getCellCharPropertiesAtByPath(sec, ppi, pathP, from);
        this.entries.push({ paraIndex: p, startOffset: from, endOffset: to, beforeCharShapeId: prevProps.charShapeId });

        wasm.applyCharFormatInCellByPath(sec, ppi, pathP, from, to, propsJson);
        const afterProps = wasm.getCellCharPropertiesAtByPath(sec, ppi, pathP, from);
        this.entries[this.entries.length - 1].afterCharShapeId = afterProps.charShapeId;
      }
    } else {
      const sec = start.sectionIndex;
      const startPara = start.paragraphIndex;
      const endPara = end.paragraphIndex;

      this.entries = [];
      for (let p = startPara; p <= endPara; p++) {
        const from = p === startPara ? start.charOffset : 0;
        const to = p === endPara ? end.charOffset : wasm.getParagraphLength(sec, p);
        if (to <= from) continue;

        const prevProps = wasm.getCharPropertiesAt(sec, p, from);
        this.entries.push({ paraIndex: p, startOffset: from, endOffset: to, beforeCharShapeId: prevProps.charShapeId });

        wasm.applyCharFormat(sec, p, from, to, propsJson);
        const afterProps = wasm.getCharPropertiesAt(sec, p, from);
        this.entries[this.entries.length - 1].afterCharShapeId = afterProps.charShapeId;
      }
    }

    return { ...this.start };
  }

  undo(wasm: WasmBridge): DocumentPosition {
    this.restoreCharShapeIds(wasm, 'before');
    return { ...this.start };
  }

  private restoreCharShapeIds(wasm: WasmBridge, side: 'before' | 'after'): void {
    const { start } = this;
    for (const entry of this.entries) {
      const charShapeId = side === 'before' ? entry.beforeCharShapeId : entry.afterCharShapeId;
      if (charShapeId === undefined) continue;

      if (isCell(start)) {
        // 중첩 셀 축 정합: 최내곽 셀에 서식 ID 복원(execute 와 동일 축).
        wasm.setCharShapeIdInCellByPath(
          start.sectionIndex, start.parentParaIndex!, cellPathJsonForPara(start, entry.paraIndex),
          entry.startOffset, entry.endOffset, charShapeId,
        );
      } else {
        wasm.setCharShapeId(start.sectionIndex, entry.paraIndex, entry.startOffset, entry.endOffset, charShapeId);
      }
    }
  }

  mergeWith(): null { return null; }
}

// ─── 문단 서식 적용 명령 ─────────────────────────────

export type ParaFormatTarget =
  | { kind: 'body'; sec: number; para: number }
  | { kind: 'cell'; sec: number; parentPara: number; controlIdx: number; cellIdx: number; cellParaIdx: number };

interface ParaShapeHistoryEntry {
  target: ParaFormatTarget;
  beforeParaShapeId: number;
  afterParaShapeId?: number;
}

function getParaShapeId(wasm: WasmBridge, target: ParaFormatTarget): number {
  const props = target.kind === 'body'
    ? wasm.getParaPropertiesAt(target.sec, target.para)
    : wasm.getCellParaPropertiesAt(
        target.sec,
        target.parentPara,
        target.controlIdx,
        target.cellIdx,
        target.cellParaIdx,
      );
  const paraShapeId = props.paraShapeId;
  if (paraShapeId === undefined) {
    throw new Error('문단 모양 ID를 조회할 수 없습니다');
  }
  return paraShapeId;
}

function applyParaFormatToTarget(wasm: WasmBridge, target: ParaFormatTarget, propsJson: string): void {
  if (target.kind === 'body') {
    wasm.applyParaFormat(target.sec, target.para, propsJson);
    return;
  }
  wasm.applyParaFormatInCell(
    target.sec,
    target.parentPara,
    target.controlIdx,
    target.cellIdx,
    target.cellParaIdx,
    propsJson,
  );
}

function restoreParaShapeId(wasm: WasmBridge, target: ParaFormatTarget, paraShapeId: number): void {
  if (target.kind === 'body') {
    wasm.setParaShapeId(target.sec, target.para, paraShapeId);
    return;
  }
  wasm.setCellParaShapeId(
    target.sec,
    target.parentPara,
    target.controlIdx,
    target.cellIdx,
    target.cellParaIdx,
    paraShapeId,
  );
}

export class ApplyParaFormatCommand implements EditCommand {
  readonly type = 'applyParaFormat';
  readonly timestamp = Date.now();

  private entries: ParaShapeHistoryEntry[] = [];

  constructor(
    private targets: ParaFormatTarget[],
    private props: Partial<ParaProperties>,
    private cursorBefore: DocumentPosition,
  ) {}

  execute(wasm: WasmBridge): DocumentPosition {
    if (this.entries.length > 0 && this.entries.every(entry => entry.afterParaShapeId !== undefined)) {
      for (const entry of this.entries) {
        restoreParaShapeId(wasm, entry.target, entry.afterParaShapeId!);
      }
      return { ...this.cursorBefore };
    }

    const propsJson = JSON.stringify(this.props);
    const entries: ParaShapeHistoryEntry[] = this.targets.map(target => ({
      target,
      beforeParaShapeId: getParaShapeId(wasm, target),
    }));

    for (const entry of entries) {
      applyParaFormatToTarget(wasm, entry.target, propsJson);
    }
    for (const entry of entries) {
      entry.afterParaShapeId = getParaShapeId(wasm, entry.target);
    }

    this.entries = entries;
    return { ...this.cursorBefore };
  }

  undo(wasm: WasmBridge): DocumentPosition {
    for (const entry of this.entries) {
      restoreParaShapeId(wasm, entry.target, entry.beforeParaShapeId);
    }
    return { ...this.cursorBefore };
  }

  mergeWith(): null { return null; }
}

// ─── 문단 끝에서 Delete로 다음 문단 병합 ─────────────

export class MergeNextParagraphCommand implements EditCommand {
  readonly type = 'mergeNextParagraph';
  readonly timestamp = Date.now();

  /** 병합으로 사라진 문단의 스코프 메타데이터 — undo 분할이 되돌린다 (Task #2342) */
  private removedParaMeta?: RemovedParaMeta;

  constructor(private position: DocumentPosition) {}

  execute(wasm: WasmBridge): DocumentPosition {
    const { sectionIndex: sec, paragraphIndex: para } = this.position;
    this.removedParaMeta = JSON.parse(wasm.mergeParagraph(sec, para + 1)).removedParaMeta;
    return { ...this.position };
  }

  undo(wasm: WasmBridge): DocumentPosition {
    const { sectionIndex: sec, paragraphIndex: para, charOffset } = this.position;
    wasm.splitParagraph(sec, para, charOffset, this.removedParaMeta);
    return { ...this.position };
  }

  mergeWith(): null { return null; }
}
