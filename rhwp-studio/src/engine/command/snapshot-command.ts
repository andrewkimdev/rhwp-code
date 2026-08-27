import type { WasmBridge } from '@/core/wasm-bridge';
import type { DocumentPosition } from '@/core/types';
import type { EditCommand, EditContext } from './types';

// ─── 스냅샷 기반 명령 (복잡한 작업의 Undo/Redo) ─────

/**
 * Document 스냅샷을 이용한 Undo/Redo 명령.
 *
 * 역연산 구현이 복잡한 작업(붙여넣기, 객체 삭제 등)에 사용한다.
 * - 최초 실행: before 스냅샷 저장 → 작업 수행 → after 스냅샷 저장
 * - Undo: before 스냅샷으로 복원
 * - Redo: after 스냅샷으로 복원
 * - Discard: 양쪽 스냅샷 메모리 해제
 */
export class SnapshotCommand implements EditCommand {
  readonly type: string;
  readonly timestamp = Date.now();

  private beforeId: number | null = null;
  private afterId: number | null = null;
  private noOp = false;

  /**
   * @param operationType 작업 종류 (예: 'pasteInternal', 'deleteControl')
   * @param cursorBefore 작업 전 커서 위치
   * @param operation 실제 작업을 수행하는 함수. 작업 후 커서 위치를 반환하며,
   *   문서를 전혀 바꾸지 않았으면 `null` 을 반환해 기록을 취소한다([Task #2370]).
   */
  constructor(
    operationType: string,
    private cursorBefore: DocumentPosition,
    private cursorAfter: DocumentPosition,
    private operation: ((wasm: WasmBridge) => DocumentPosition | null) | null,
  ) {
    this.type = `snapshot:${operationType}`;
  }

  execute(wasm: WasmBridge): DocumentPosition {
    if (this.afterId !== null) {
      // Redo: after 스냅샷으로 복원
      wasm.restoreSnapshot(this.afterId);
      return { ...this.cursorAfter };
    }

    // 최초 실행: before 저장 → 작업 수행 → after 저장
    this.beforeId = wasm.saveSnapshot();
    // [Task #2328] operation 또는 after-save 중 어느 것이 throw 하든 커맨드가
    // 히스토리에 등록되지 못해 discard 주체가 사라진다 → 스냅샷 영구 누수(orphan
    // → WASM 무통보 축출 재발). after-save(대용량 문서 클론 시 메모리 압박 등)까지
    // try 범위에 포함해 before/after 를 대칭적으로 해제한다.
    try {
      if (this.operation) {
        const result = this.operation(wasm);
        if (result === null) {
          // [Task #2370] 문서 무변경 — after 를 저장하지 않고 before 를 즉시 반환한다.
          // 히스토리는 isNoOp() 를 보고 이 명령을 스택에 넣지 않는다.
          this.noOp = true;
          this.operation = null;
          this.discard(wasm);
          return { ...this.cursorBefore };
        }
        this.cursorAfter = result;
      }
      this.afterId = wasm.saveSnapshot();
    } catch (operationError) {
      // [#3350] 최초 execute 가 실패하면 명령 전체를 원자적으로 되돌린다. 이 커맨드는
      // history 에 push 되기 전이므로 before 스냅샷을 가진 SnapshotCommand만 rollback을
      // 수행할 수 있다. after-save 실패도 execute 실패이므로 같은 계약을 따른다.
      try {
        if (this.beforeId !== null) {
          wasm.restoreSnapshot(this.beforeId);
        }
      } catch (rollbackError) {
        this.discard(wasm);
        throw new AggregateError(
          [operationError, rollbackError],
          `${this.type} 실행 실패 후 rollback도 실패했습니다`,
        );
      }
      this.discard(wasm);
      throw operationError;
    }

    // operation 참조 해제 (클로저에 캡처된 리소스 해제)
    this.operation = null;

    return { ...this.cursorAfter };
  }

  /** [Task #2370] operation 이 `null` 을 반환해 기록이 취소된 명령인가. */
  isNoOp(): boolean {
    return this.noOp;
  }

  undo(wasm: WasmBridge): DocumentPosition {
    if (this.beforeId !== null) {
      wasm.restoreSnapshot(this.beforeId);
    }
    return { ...this.cursorBefore };
  }

  mergeWith(): null { return null; }

  /** [Task #2328] 현재 살아있는 before/after 스냅샷 id 개수. */
  snapshotResourceCount(): number {
    return (this.beforeId !== null ? 1 : 0) + (this.afterId !== null ? 1 : 0);
  }

  discard(wasm: WasmBridge): void {
    if (this.beforeId !== null) {
      wasm.discardSnapshot(this.beforeId);
      this.beforeId = null;
    }
    if (this.afterId !== null) {
      wasm.discardSnapshot(this.afterId);
      this.afterId = null;
    }
  }
}

/**
 * 머리말/꼬리말·각주 안에서만 쓰는 스냅샷 명령.
 *
 * 일반 SnapshotCommand는 구조 편집처럼 undo 뒤 본문으로 돌아가야 하는 작업도 담당한다.
 * 그래서 편집 문맥을 일반 클래스에 붙이지 않고, 서브모드를 보존해야 하는 호출부만 이 타입을
 * 명시적으로 선택한다.
 */
export class SubmodeSnapshotCommand extends SnapshotCommand {
  constructor(
    operationType: string,
    cursorBefore: DocumentPosition,
    cursorAfter: DocumentPosition,
    operation: ((wasm: WasmBridge) => DocumentPosition | null) | null,
    private readonly context: EditContext,
  ) {
    super(operationType, cursorBefore, cursorAfter, operation);
  }

  editContext(): EditContext { return this.context; }
}
