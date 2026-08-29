import type { RemovedParaMeta, WasmBridge } from '@/core/wasm-bridge';
import type { DocumentPosition } from '@/core/types';
import {
  IMMEDIATE_TEXT_MUTATION_EFFECTS,
  NO_TEXT_MUTATION_EFFECTS,
  type TextMutationEffects,
} from './types';
import type { EditCommand } from './types';
import {
  cellParaIndexOf,
  cellParagraphPosition,
  cellPathJson,
  isNestedCell,
} from './cell-path';

// ─── 셀 내부 문단 분할 명령 (셀 내 Enter) ──────────────

export class SplitParagraphInCellCommand implements EditCommand {
  readonly type = 'splitParagraphInCell';
  readonly timestamp = Date.now();
  private lastMutationEffects: TextMutationEffects = NO_TEXT_MUTATION_EFFECTS;

  constructor(private position: DocumentPosition) {}

  execute(wasm: WasmBridge): DocumentPosition {
    this.lastMutationEffects = NO_TEXT_MUTATION_EFFECTS;
    const pos = this.position;
    const sec = pos.sectionIndex;
    const ppi = pos.parentParaIndex!;
    const cpi = cellParaIndexOf(pos);
    if (isNestedCell(pos)) {
      wasm.splitParagraphInCellByPath(sec, ppi, cellPathJson(pos), pos.charOffset);
    } else {
      wasm.splitParagraphInCell(sec, ppi, pos.controlIndex!, pos.cellIndex!, cpi, pos.charOffset);
    }
    // [#4031] 네이티브 split은 paginate_if_needed()로 최신 revision을 동기 계산한다.
    // 이 선언이 pending deferred 상태를 해소해 직후 before-full-edit flush가 no-op이 된다.
    this.lastMutationEffects = IMMEDIATE_TEXT_MUTATION_EFFECTS;
    return cellParagraphPosition(pos, cpi + 1, 0);
  }

  consumeTextMutationEffects(): TextMutationEffects {
    const effects = this.lastMutationEffects;
    this.lastMutationEffects = NO_TEXT_MUTATION_EFFECTS;
    return effects;
  }

  undo(wasm: WasmBridge): DocumentPosition {
    const pos = this.position;
    const sec = pos.sectionIndex;
    const ppi = pos.parentParaIndex!;
    const cpi = cellParaIndexOf(pos);
    if (isNestedCell(pos)) {
      // undo: 분할된 다음 문단을 병합 → cellPath의 cellParaIndex를 +1로 변경
      const undoPath = [...pos.cellPath!];
      undoPath[undoPath.length - 1] = { ...undoPath[undoPath.length - 1], cellParaIndex: cpi + 1 };
      wasm.mergeParagraphInCellByPath(sec, ppi, JSON.stringify(undoPath));
    } else {
      wasm.mergeParagraphInCell(sec, ppi, pos.controlIndex!, pos.cellIndex!, cpi + 1);
    }
    return { ...pos };
  }

  mergeWith(): null { return null; }
}

// ─── 셀 내부 문단 병합 명령 (셀 문단 시작에서 Backspace) ──

export class MergeParagraphInCellCommand implements EditCommand {
  readonly type = 'mergeParagraphInCell';
  readonly timestamp = Date.now();

  private mergePointOffset = 0;
  /** 사라진 문단의 스코프 메타 — undo(분할)가 되돌린다 (Task #2342). */
  private removedParaMeta?: RemovedParaMeta;

  constructor(private position: DocumentPosition) {}

  execute(wasm: WasmBridge): DocumentPosition {
    const pos = this.position;
    const sec = pos.sectionIndex;
    const ppi = pos.parentParaIndex!;
    const cpi = cellParaIndexOf(pos);
    // 병합 전 이전 셀 문단 길이 기억.
    // flat 필드(controlIndex/cellIndex)는 "외부 표 기준" 레거시 좌표라(types.ts DocumentPosition)
    // 중첩 셀에서는 안쪽 셀을 가리키지 못한다. 뮤테이션이 ByPath 로 분기하는 만큼 길이 조회도
    // 같은 축으로 맞춘다(cursor.ts 의 useCellPath 분기와 동형). 어긋나면 undo 가 바깥 셀에서
    // 읽은 길이로 안쪽 셀을 분할해 문단이 엉뚱한 지점에서 잘린다.
    if (isNestedCell(pos)) {
      const prevPath = pos.cellPath!.map((entry, index, path) =>
        index + 1 === path.length ? { ...entry, cellParaIndex: cpi - 1 } : entry,
      );
      this.mergePointOffset = wasm.getCellParagraphLengthByPath(sec, ppi, JSON.stringify(prevPath));
      this.removedParaMeta = JSON.parse(wasm.mergeParagraphInCellByPath(sec, ppi, cellPathJson(pos))).removedParaMeta;
    } else {
      this.mergePointOffset = wasm.getCellParagraphLength(sec, ppi, pos.controlIndex!, pos.cellIndex!, cpi - 1);
      this.removedParaMeta = JSON.parse(wasm.mergeParagraphInCell(sec, ppi, pos.controlIndex!, pos.cellIndex!, cpi)).removedParaMeta;
    }
    return cellParagraphPosition(pos, cpi - 1, this.mergePointOffset);
  }

  undo(wasm: WasmBridge): DocumentPosition {
    const pos = this.position;
    const sec = pos.sectionIndex;
    const ppi = pos.parentParaIndex!;
    const cpi = cellParaIndexOf(pos);
    if (isNestedCell(pos)) {
      const undoPath = [...pos.cellPath!];
      undoPath[undoPath.length - 1] = { ...undoPath[undoPath.length - 1], cellParaIndex: cpi - 1 };
      wasm.splitParagraphInCellByPath(sec, ppi, JSON.stringify(undoPath), this.mergePointOffset, this.removedParaMeta);
    } else {
      wasm.splitParagraphInCell(sec, ppi, pos.controlIndex!, pos.cellIndex!, cpi - 1, this.mergePointOffset, this.removedParaMeta);
    }
    return { ...pos };
  }

  mergeWith(): null { return null; }
}

// ─── 셀 내부 다음 문단 병합 명령 (셀 문단 끝에서 Delete) ──

export class MergeNextParagraphInCellCommand implements EditCommand {
  readonly type = 'mergeNextParagraphInCell';
  readonly timestamp = Date.now();

  /** 사라진 문단의 스코프 메타 — undo(분할)가 되돌린다 (Task #2342). */
  private removedParaMeta?: RemovedParaMeta;

  constructor(private position: DocumentPosition) {}

  execute(wasm: WasmBridge): DocumentPosition {
    const pos = this.position;
    const sec = pos.sectionIndex;
    const ppi = pos.parentParaIndex!;
    const cpi = cellParaIndexOf(pos);
    if (isNestedCell(pos)) {
      const nextPath = [...pos.cellPath!];
      nextPath[nextPath.length - 1] = { ...nextPath[nextPath.length - 1], cellParaIndex: cpi + 1 };
      this.removedParaMeta = JSON.parse(wasm.mergeParagraphInCellByPath(sec, ppi, JSON.stringify(nextPath))).removedParaMeta;
    } else {
      this.removedParaMeta = JSON.parse(wasm.mergeParagraphInCell(sec, ppi, pos.controlIndex!, pos.cellIndex!, cpi + 1)).removedParaMeta;
    }
    return { ...pos };
  }

  undo(wasm: WasmBridge): DocumentPosition {
    const pos = this.position;
    const sec = pos.sectionIndex;
    const ppi = pos.parentParaIndex!;
    const cpi = cellParaIndexOf(pos);
    if (isNestedCell(pos)) {
      wasm.splitParagraphInCellByPath(sec, ppi, cellPathJson(pos), pos.charOffset, this.removedParaMeta);
    } else {
      wasm.splitParagraphInCell(sec, ppi, pos.controlIndex!, pos.cellIndex!, cpi, pos.charOffset, this.removedParaMeta);
    }
    return { ...pos };
  }

  mergeWith(): null { return null; }
}
