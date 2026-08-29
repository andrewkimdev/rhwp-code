import type {
  DeferredFocusedPagePatch,
  WasmBridge,
} from '@/core/wasm-bridge';
import type { DocumentPosition } from '@/core/types';

/** 편집 명령 공통 인터페이스 */
export interface EditCommand {
  readonly type: string;
  readonly timestamp: number;
  /** 명령 실행 — 실행 후 커서 위치 반환 */
  execute(wasm: WasmBridge): DocumentPosition;
  /** 역실행 — 실행 전 커서 위치 반환 */
  undo(wasm: WasmBridge): DocumentPosition;
  /** 연속 명령 병합 시도 */
  mergeWith(other: EditCommand): EditCommand | null;
  /** 리소스 해제 (스냅샷 명령의 메모리 반환 등). 스택에서 제거될 때 호출. */
  discard?(wasm: WasmBridge): void;
  /**
   * [Task #2328] 이 명령이 현재 점유한 WASM 스냅샷 id 개수(없으면 0).
   * CommandHistory 가 스냅샷 예산을 WASM 상한과 정합시키는 데 쓴다.
   */
  snapshotResourceCount?(): number;
  /**
   * [Task #2370 클러스터 A] execute() 가 문서를 전혀 바꾸지 않았는가.
   * true 면 CommandHistory 가 이 명령을 undo 스택에 넣지 않는다 — 되돌릴 것이 없는
   * 엔트리는 Ctrl+Z 를 무효과로 소모하고, redo 스택을 파기하며(`execute` 는 새 명령마다
   * redo 를 버린다), 스냅샷 명령이면 예산 2슬롯을 점유해 진짜 undo 이력을 축출한다.
   * 구현하지 않으면 종전대로 항상 기록된다.
   */
  isNoOp?(): boolean;
  /** page-local refresh 판정을 위한 가벼운 텍스트 편집 payload. */
  getPageLocalTextEditOptions?(): { insertedText?: string; deleteCount?: number };
  /** 방금 실행한 mutation effect를 한 번만 반환한다. */
  consumeTextMutationEffects?(): TextMutationEffects;
  /**
   * [Task #2337] 이 커맨드의 마지막 execute/undo 후 복원할 머리말/꼬리말·각주 편집
   * 컨텍스트. 본문 커맨드는 미구현(→ 반환 없음 = 본문 모드). InputHandler 가 undo/redo
   * 시 이 값을 읽어 HF/FN 모드 재진입 + 커서 위치를 복원하고 본문 moveTo 를 건너뛴다.
   */
  editContext?(): EditContext | null;
}

/**
 * [Task #2337] 머리말/꼬리말·각주 편집 커맨드가 undo/redo 후 복원할 편집 컨텍스트.
 * 본문 DocumentPosition 과 별개인 HF/FN 커서 모드를 서술한다(cursor.ts 의
 * enterHeaderFooterMode/enterFootnoteMode + set{Hf,Fn}CursorPosition 인자에 대응).
 */
export type EditContext =
  | {
      readonly mode: 'headerFooter';
      readonly sectionIdx: number;
      readonly isHeader: boolean;
      readonly applyTo: number;
      readonly paraIdx: number;
      readonly charOffset: number;
    }
  | {
      readonly mode: 'footnote';
      readonly sectionIdx: number;
      readonly paraIdx: number;
      readonly controlIdx: number;
      readonly footnoteIndex: number;
      readonly pageNum: number;
      readonly innerParaIdx: number;
      readonly charOffset: number;
    };

/** text mutation의 document pagination/flow 경계와 immediate 완료를 함께 전달한다. */
export interface FocusedCellCursorGeometry {
  readonly baseRevision: number;
  readonly revision: number;
  readonly source: DocumentPosition;
  readonly target: DocumentPosition;
  readonly deltaX: number;
}

export interface TextMutationEffects {
  readonly documentPaginationPending: boolean;
  readonly flowChanged: boolean;
  readonly paginationCompleted: boolean;
  readonly focusedCursorGeometry?: FocusedCellCursorGeometry;
  readonly focusedPagePatch?: DeferredFocusedPagePatch;
}

export const NO_TEXT_MUTATION_EFFECTS: TextMutationEffects = Object.freeze({
  documentPaginationPending: false,
  flowChanged: false,
  paginationCompleted: false,
});

export const IMMEDIATE_TEXT_MUTATION_EFFECTS: TextMutationEffects = Object.freeze({
  documentPaginationPending: false,
  flowChanged: false,
  paginationCompleted: true,
});

/** raw IME/iOS 묶음에서 effect를 OR 누적하고 한 번만 소비한다. */
export class TextMutationEffectAccumulator {
  private effects: TextMutationEffects = NO_TEXT_MUTATION_EFFECTS;

  add(effects: TextMutationEffects): void {
    const accumulatedMutation = this.effects.documentPaginationPending
      || this.effects.flowChanged
      || this.effects.paginationCompleted;
    const incomingMutation = effects.documentPaginationPending
      || effects.flowChanged
      || effects.paginationCompleted;
    // 두 mutation을 한 번에 묶으면 중간 source rect를 보장할 수 없다. 단일 mutation이거나
    // 앞뒤가 NO effect인 경우에만 focused geometry를 전달한다.
    const focusedCursorGeometry = accumulatedMutation
      ? (incomingMutation ? undefined : this.effects.focusedCursorGeometry)
      : (incomingMutation ? effects.focusedCursorGeometry : undefined);
    const focusedPagePatch = accumulatedMutation
      ? (
          incomingMutation
            ? mergeFocusedPagePatches(this.effects.focusedPagePatch, effects.focusedPagePatch)
            : this.effects.focusedPagePatch
        )
      : (incomingMutation ? effects.focusedPagePatch : undefined);
    this.effects = {
      documentPaginationPending:
        this.effects.documentPaginationPending || effects.documentPaginationPending,
      flowChanged: this.effects.flowChanged || effects.flowChanged,
      paginationCompleted: this.effects.paginationCompleted || effects.paginationCompleted,
      ...(focusedCursorGeometry ? { focusedCursorGeometry } : {}),
      ...(focusedPagePatch ? { focusedPagePatch } : {}),
    };
  }

  consume(): TextMutationEffects {
    const effects = this.effects;
    this.effects = NO_TEXT_MUTATION_EFFECTS;
    return effects;
  }

  clear(): void {
    this.effects = NO_TEXT_MUTATION_EFFECTS;
  }
}

function mergeFocusedPagePatches(
  first: DeferredFocusedPagePatch | undefined,
  second: DeferredFocusedPagePatch | undefined,
): DeferredFocusedPagePatch | undefined {
  if (!first || !second || first.pageIndex !== second.pageIndex) return undefined;
  const x = Math.min(first.x, second.x);
  const y = Math.min(first.y, second.y);
  const right = Math.max(first.x + first.width, second.x + second.width);
  const bottom = Math.max(first.y + first.height, second.y + second.height);
  return {
    pageIndex: first.pageIndex,
    x,
    y,
    width: right - x,
    height: bottom - y,
  };
}

// ─── 편집 작업 서술자 (라우팅 통합) ────────────────────

export type EditDomain =
  | 'text'
  | 'charFormat'
  | 'paraFormat'
  | 'table'
  | 'object'
  | 'page'
  | 'field'
  | 'view'
  | 'unknown';

export type RefreshPolicy = 'auto' | 'full' | 'pageLocal' | 'selectionOnly' | 'none';

export type DirtyScope =
  | 'document'
  | 'section'
  | 'page'
  | 'paragraph'
  | 'table'
  | 'object'
  | 'none';

export type SelectionPolicy =
  | 'auto'
  | 'keep'
  | 'moveToResult'
  | 'restoreObjectSelection'
  | 'none';

export interface OperationMetadata {
  /** 메뉴/툴바/단축키 action id. */
  actionId?: string;
  /** 편집 도메인. 직접 wasm mutation을 audit 할 때 분류 기준으로 사용한다. */
  domain?: EditDomain;
  /** mutation 후 렌더링 갱신 정책. 생략하면 kind 별 기존 기본값을 따른다. */
  refresh?: RefreshPolicy;
  /** 장기적으로 renderer invalidation 최적화에 사용할 dirty 범위. */
  dirtyScope?: DirtyScope;
  /** selection/caret 복원 정책. 현재는 문서화용 metadata로만 사용한다. */
  selection?: SelectionPolicy;
}

/**
 * 편집 작업 서술자 — 호출부가 "무엇을 하려는가"만 기술하고,
 * 라우터(executeOperation)가 적절한 Undo 전략을 자동 선택한다.
 *
 * - command: 정밀 커맨드 (텍스트 삽입/삭제, 문단 분할/병합, 서식)
 * - snapshot: 스냅샷 기반 커맨드 (붙여넣기, 객체 삭제 등)
 * - record:  WASM 직접 호출 후 히스토리에만 기록 (IME, 객체 이동).
 *            아키텍처 문서에서는 recordApplied 계약으로 정의한다.
 */
export type OperationDescriptor =
  | { kind: 'command'; command: EditCommand; meta?: OperationMetadata }
  // [Task #2370] snapshot 의 operation 은 아무것도 바꾸지 않았을 때 `null` 을 반환해
  // "기록하지 말 것"을 알린다(그 경우 커서 이동·리프레시도 건너뛴다).
  | {
      kind: 'snapshot';
      operationType: string;
      operation: (wasm: WasmBridge) => DocumentPosition | null;
      /** 본문 좌표와 분리된 HF/FN 편집 문맥. undo/redo 뒤 같은 문맥으로 돌아간다. */
      editContext?: EditContext;
      meta?: OperationMetadata;
    }
  | { kind: 'record'; command: EditCommand; meta?: OperationMetadata };
