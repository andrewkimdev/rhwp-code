import type { RemovedParaMeta, WasmBridge } from '@/core/wasm-bridge';
import type { DocumentPosition, CharProperties } from '@/core/types';
import { NO_TEXT_MUTATION_EFFECTS, type TextMutationEffects } from './types';
import type { EditCommand } from './types';
import {
  cellParaIndexOf,
  cellPathJson,
  charCount,
  isCell,
} from './cell-path';
import {
  applyCharShapeModsToRange,
  deleteTextWithMutationEffects,
  doDeleteTextImmediate,
  doInsertTextImmediate,
  doGetTextRange,
  insertTextWithMutationEffects,
} from './text-mutation';
import { SnapshotCommand } from './snapshot-command';

function sameCharFormat(a: Partial<CharProperties> | undefined, b: Partial<CharProperties> | undefined): boolean {
  return JSON.stringify(a ?? null) === JSON.stringify(b ?? null);
}

// ─── 텍스트 삽입 명령 ─────────────────────────────────

export class InsertTextCommand implements EditCommand {
  readonly type = 'insertText';
  readonly timestamp: number;
  private lastMutationEffects: TextMutationEffects = NO_TEXT_MUTATION_EFFECTS;

  constructor(
    private position: DocumentPosition,
    private text: string,
    timestamp?: number,
    /** [#4162] 선택 없이 지정한 예약 글자 모양 — 삽입된 텍스트에 그대로 건다. */
    private charFormat?: Partial<CharProperties>,
  ) {
    this.timestamp = timestamp ?? Date.now();
  }

  getCharFormat(): Partial<CharProperties> | undefined {
    return this.charFormat;
  }

  execute(wasm: WasmBridge): DocumentPosition {
    this.lastMutationEffects = NO_TEXT_MUTATION_EFFECTS;
    this.lastMutationEffects = insertTextWithMutationEffects(wasm, this.position, this.text);
    const after = { ...this.position, charOffset: this.position.charOffset + this.text.length };
    if (this.charFormat) {
      applyCharShapeModsToRange(wasm, this.position, this.position.charOffset, after.charOffset, this.charFormat);
    }
    return after;
  }

  consumeTextMutationEffects(): TextMutationEffects {
    const effects = this.lastMutationEffects;
    this.lastMutationEffects = NO_TEXT_MUTATION_EFFECTS;
    return effects;
  }

  getPageLocalTextEditOptions(): { insertedText: string } {
    return { insertedText: this.text };
  }

  undo(wasm: WasmBridge): DocumentPosition {
    this.lastMutationEffects = NO_TEXT_MUTATION_EFFECTS;
    // [#2337-review] 삭제 count 는 char(Unicode scalar) 단위다. UTF-16 length 를 넘기면
    // astral 문자에서 실제보다 많이 지워 인접 문자를 잃는다 → HF/FN 과 동일하게 charCount.
    doDeleteTextImmediate(wasm, this.position, charCount(this.text));
    return { ...this.position };
  }

  mergeWith(other: EditCommand): EditCommand | null {
    if (!(other instanceof InsertTextCommand)) return null;
    // 같은 문단/셀인지 확인
    if (other.position.sectionIndex !== this.position.sectionIndex) return null;
    if (other.position.paragraphIndex !== this.position.paragraphIndex) return null;
    if (isCell(this.position) !== isCell(other.position)) return null;
    if (isCell(this.position)) {
      if (other.position.parentParaIndex !== this.position.parentParaIndex) return null;
      if (other.position.controlIndex !== this.position.controlIndex) return null;
      if (other.position.cellIndex !== this.position.cellIndex) return null;
      if (other.position.cellParaIndex !== this.position.cellParaIndex) return null;
    }
    // 연속 위치 확인
    const expectedOffset = this.position.charOffset + this.text.length;
    if (other.position.charOffset !== expectedOffset) return null;
    // 300ms 이내
    if (other.timestamp - this.timestamp > 300) return null;
    // 줄바꿈/탭 포함 시 병합 불가
    if (other.text.includes('\n') || other.text.includes('\t')) return null;
    // [#4162] 예약 글자 모양이 다르면 하나의 undo 단위로 묶지 않는다
    if (!sameCharFormat(this.charFormat, other.charFormat)) return null;

    return new InsertTextCommand(this.position, this.text + other.text, this.timestamp, this.charFormat);
  }
}

// ─── 텍스트 삭제 명령 ─────────────────────────────────

export class DeleteTextCommand implements EditCommand {
  readonly type = 'deleteText';
  readonly timestamp: number;

  /** undo용 삭제된 텍스트 (execute 시 보존) */
  private deletedText: string;
  private lastMutationEffects: TextMutationEffects = NO_TEXT_MUTATION_EFFECTS;

  constructor(
    private position: DocumentPosition,
    private count: number,
    private direction: 'forward' | 'backward',
    deletedText?: string,
    timestamp?: number,
  ) {
    this.deletedText = deletedText ?? '';
    this.timestamp = timestamp ?? Date.now();
  }

  execute(wasm: WasmBridge): DocumentPosition {
    this.lastMutationEffects = NO_TEXT_MUTATION_EFFECTS;
    // 삭제 전 텍스트 보존
    if (!this.deletedText) {
      this.deletedText = doGetTextRange(wasm, this.position, this.count);
    }
    this.lastMutationEffects = deleteTextWithMutationEffects(wasm, this.position, this.count);
    return { ...this.position };
  }

  consumeTextMutationEffects(): TextMutationEffects {
    const effects = this.lastMutationEffects;
    this.lastMutationEffects = NO_TEXT_MUTATION_EFFECTS;
    return effects;
  }

  getPageLocalTextEditOptions(): { deleteCount: number } {
    return { deleteCount: this.count };
  }

  undo(wasm: WasmBridge): DocumentPosition {
    this.lastMutationEffects = NO_TEXT_MUTATION_EFFECTS;
    doInsertTextImmediate(wasm, this.position, this.deletedText);
    const restoredLen = this.deletedText.length;
    return { ...this.position, charOffset: this.position.charOffset + restoredLen };
  }

  mergeWith(other: EditCommand): EditCommand | null {
    if (!(other instanceof DeleteTextCommand)) return null;
    if (other.direction !== this.direction) return null;
    if (other.timestamp - this.timestamp > 300) return null;
    // 같은 문단/셀 확인
    if (other.position.sectionIndex !== this.position.sectionIndex) return null;
    if (other.position.paragraphIndex !== this.position.paragraphIndex) return null;
    if (isCell(this.position) !== isCell(other.position)) return null;
    if (isCell(this.position)) {
      if (other.position.parentParaIndex !== this.position.parentParaIndex) return null;
      if (other.position.controlIndex !== this.position.controlIndex) return null;
      if (other.position.cellIndex !== this.position.cellIndex) return null;
      if (other.position.cellParaIndex !== this.position.cellParaIndex) return null;
    }

    if (this.direction === 'backward') {
      // Backspace: 연속 앞쪽 삭제
      if (other.position.charOffset === this.position.charOffset - other.count) {
        return new DeleteTextCommand(
          other.position, this.count + other.count, 'backward',
          other.deletedText + this.deletedText, this.timestamp,
        );
      }
    } else {
      // Delete: 같은 위치에서 연속 삭제
      if (other.position.charOffset === this.position.charOffset) {
        return new DeleteTextCommand(
          this.position, this.count + other.count, 'forward',
          this.deletedText + other.deletedText, this.timestamp,
        );
      }
    }
    return null;
  }
}

// ─── 강제 줄바꿈 명령 (Shift+Enter) ─────────────────────

export class InsertLineBreakCommand implements EditCommand {
  readonly type = 'insertLineBreak';
  readonly timestamp = Date.now();

  constructor(private position: DocumentPosition) {}

  execute(wasm: WasmBridge): DocumentPosition {
    doInsertTextImmediate(wasm, this.position, '\n');
    const newPos = { ...this.position, charOffset: this.position.charOffset + 1 };
    return newPos;
  }

  undo(wasm: WasmBridge): DocumentPosition {
    doDeleteTextImmediate(wasm, this.position, 1);
    return { ...this.position };
  }

  mergeWith(): null { return null; }
}

// ─── 탭 삽입 명령 (Tab) ──────────────────────────────

export class InsertTabCommand implements EditCommand {
  readonly type = 'insertTab';
  readonly timestamp = Date.now();

  constructor(private position: DocumentPosition) {}

  execute(wasm: WasmBridge): DocumentPosition {
    doInsertTextImmediate(wasm, this.position, '\t');
    const newPos = { ...this.position, charOffset: this.position.charOffset + 1 };
    return newPos;
  }

  undo(wasm: WasmBridge): DocumentPosition {
    doDeleteTextImmediate(wasm, this.position, 1);
    return { ...this.position };
  }

  mergeWith(): null { return null; }
}

// ─── 문단 분할 명령 (Enter) ───────────────────────────

export class SplitParagraphCommand implements EditCommand {
  readonly type = 'splitParagraph';
  readonly timestamp = Date.now();

  constructor(private position: DocumentPosition) {}

  execute(wasm: WasmBridge): DocumentPosition {
    const { sectionIndex: sec, paragraphIndex: para, charOffset } = this.position;
    const result = JSON.parse(wasm.splitParagraph(sec, para, charOffset));
    if (result.ok) {
      return { sectionIndex: sec, paragraphIndex: result.paraIdx, charOffset: 0 };
    }
    return this.position;
  }

  undo(wasm: WasmBridge): DocumentPosition {
    const { sectionIndex: sec, paragraphIndex: para } = this.position;
    wasm.mergeParagraph(sec, para + 1);
    return { ...this.position };
  }

  mergeWith(): null { return null; }
}

// ─── 문단 병합 명령 (문단 시작에서 Backspace) ─────────

export class MergeParagraphCommand implements EditCommand {
  readonly type = 'mergeParagraph';
  readonly timestamp = Date.now();

  /** undo 시 분할 위치 (이전 문단의 원래 길이) */
  private mergePointOffset = 0;
  /** 병합으로 사라진 문단의 스코프 메타데이터 — undo 분할이 되돌린다 (Task #2342) */
  private removedParaMeta?: RemovedParaMeta;

  constructor(private position: DocumentPosition) {}

  execute(wasm: WasmBridge): DocumentPosition {
    const { sectionIndex: sec, paragraphIndex: para } = this.position;
    // 병합 전 이전 문단 길이 기억
    this.mergePointOffset = wasm.getParagraphLength(sec, para - 1);
    this.removedParaMeta = JSON.parse(wasm.mergeParagraph(sec, para)).removedParaMeta;
    return { sectionIndex: sec, paragraphIndex: para - 1, charOffset: this.mergePointOffset };
  }

  undo(wasm: WasmBridge): DocumentPosition {
    const { sectionIndex: sec, paragraphIndex: para } = this.position;
    wasm.splitParagraph(sec, para - 1, this.mergePointOffset, this.removedParaMeta);
    return { ...this.position };
  }

  mergeWith(): null { return null; }
}

// ─── 선택 영역 삭제 명령 ─────────────────────────────

export class DeleteSelectionCommand implements EditCommand {
  readonly type = 'deleteSelection';
  readonly timestamp = Date.now();

  /**
   * 삭제 범위의 복원은 문서 스냅샷에 맡긴다 (Task #2418).
   *
   * 평문만 저장해 되돌리던 이전 방식은 글자 모양·문단 메타·인라인 컨트롤을 되살리지
   * 못했다 — 재삽입은 삽입 지점의 현재 글자 모양을 쓰고, 다문단 복원은 문단 메타를 앞
   * 문단에서 상속하는 `splitParagraph` 를 타며(#2342), 셀 다문단은 문단 구조 대신
   * `'\n'` 이어붙이기로 대체됐다. 선택 범위의 서식·컨트롤을 평문 밖에서 따로 캡처하려면
   * 글자모양 run·문단 메타·컨트롤을 읽고 되돌리는 API 가 새로 필요한데, 그것은 스냅샷이
   * 이미 하는 일이다. 같은 이유로 붙여넣기(`pasteInternal` 등)가 스냅샷을 쓰므로 그
   * 역연산인 선택 삭제도 같은 방식으로 맞춘다.
   *
   * `kind:'command'` 로 남는다 — 양식 모드 게이트(`isOperationAllowedInEditMode` 의
   * `'deleteSelection'` 분기)가 커맨드 타입에 걸려 있어, `kind:'snapshot'` 으로 바꾸면
   * 양식 모드 선택 삭제가 게이트에서 드롭돼 무언 폐기가 된다.
   */
  private readonly snapshot: SnapshotCommand;

  constructor(start: DocumentPosition, end: DocumentPosition) {
    // 삭제 후 커서는 선택 시작으로 모이고, undo 후에는 선택 끝으로 되돌아간다.
    this.snapshot = new SnapshotCommand('deleteSelection', end, start, (wasm) => {
      if (isCell(start)) {
        // 중첩 셀 좌표 축 정합: flat controlIndex/cellIndex 는 cellPath[0](최외곽)이라
        // 중첩 셀에서 바깥 셀을 지운다. 최내곽 셀을 대상으로 ...ByPath 로 라우팅하고,
        // 셀 문단 인덱스는 cellPath[last] 에서 읽는다(cellParaIndexOf).
        wasm.deleteRangeInCellByPath(
          start.sectionIndex, start.parentParaIndex!, cellPathJson(start),
          cellParaIndexOf(start), start.charOffset, cellParaIndexOf(end), end.charOffset,
        );
      } else {
        wasm.deleteRange(
          start.sectionIndex, start.paragraphIndex, start.charOffset,
          end.paragraphIndex, end.charOffset,
        );
      }
      return { ...start };
    });
  }

  execute(wasm: WasmBridge): DocumentPosition {
    return this.snapshot.execute(wasm);
  }

  undo(wasm: WasmBridge): DocumentPosition {
    return this.snapshot.undo(wasm);
  }

  mergeWith(): null { return null; }

  snapshotResourceCount(): number {
    return this.snapshot.snapshotResourceCount();
  }

  discard(wasm: WasmBridge): void {
    this.snapshot.discard(wasm);
  }
}
