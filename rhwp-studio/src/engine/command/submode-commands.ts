import type { RemovedParaMeta, WasmBridge } from '@/core/wasm-bridge';
import type { DocumentPosition } from '@/core/types';
import { charCount } from './cell-path';
import type { EditCommand, EditContext } from './types';

// ─── [Task #2337] 머리말/꼬리말·각주 편집 커맨드 ───────────────────────────
//
// HF/FN 편집은 본문과 별개 WASM 경로(insert/delete/merge/split × HeaderFooter/Footnote)
// 를 쓰고 커서도 별도 모드다. 본문 텍스트/문단 커맨드(역연산 경량)를 미러링해 히스토리에
// 기록함으로써, 본문 스냅샷 undo 가 미기록 HF/FN 편집을 무언 파괴하던 데이터 손실을 막는다.
// 최초 적용은 InputHandler 가 인라인 뮤테이션 후 kind:'record' 로 기록하므로 execute()
// 는 redo 시에만 호출된다. undo()/execute() 는 lastContext 를 각각 실행 전/후 좌표로
// 갱신하며, InputHandler 가 editContext() 로 읽어 HF/FN 커서 모드를 복원한다(커맨드는 순수).

export interface HeaderFooterEditTarget {
  readonly sectionIdx: number;
  readonly isHeader: boolean;
  readonly applyTo: number;
}

export interface FootnoteEditTarget {
  readonly sectionIdx: number;
  /** 각주 컨트롤을 담은 본문 문단 인덱스 */
  readonly paraIdx: number;
  readonly controlIdx: number;
  /** 모드 재진입용 (enterFootnoteMode 인자) */
  readonly footnoteIndex: number;
  readonly pageNum: number;
}

function hfEditContext(t: HeaderFooterEditTarget, paraIdx: number, charOffset: number): EditContext {
  return { mode: 'headerFooter', sectionIdx: t.sectionIdx, isHeader: t.isHeader, applyTo: t.applyTo, paraIdx, charOffset };
}

function fnEditContext(t: FootnoteEditTarget, innerParaIdx: number, charOffset: number): EditContext {
  return {
    mode: 'footnote', sectionIdx: t.sectionIdx, paraIdx: t.paraIdx, controlIdx: t.controlIdx,
    footnoteIndex: t.footnoteIndex, pageNum: t.pageNum, innerParaIdx, charOffset,
  };
}

/**
 * HF/FN 커맨드의 execute/undo 반환 위치는 형식상 값이다 — InputHandler 는 editContext()
 * 로 커서를 복원하며 이 본문 위치로 moveTo 하지 않는다(단, 반환은 non-null 이어야
 * history.undo/redo 가 성공으로 간주한다).
 */
function hfFnStubPosition(sectionIdx: number): DocumentPosition {
  return { sectionIndex: sectionIdx, paragraphIndex: 0, charOffset: 0 };
}

// ── 머리말/꼬리말 ──────────────────────────────────────────

export class InsertTextInHeaderFooterCommand implements EditCommand {
  readonly type = 'insertTextInHeaderFooter';
  readonly timestamp = Date.now();
  private lastContext: EditContext;

  constructor(
    private target: HeaderFooterEditTarget,
    private paraIdx: number,
    private charOffset: number,
    private text: string,
  ) {
    this.lastContext = hfEditContext(target, paraIdx, charOffset + text.length);
  }

  execute(wasm: WasmBridge): DocumentPosition {
    wasm.insertTextInHeaderFooter(this.target.sectionIdx, this.target.isHeader, this.target.applyTo, this.paraIdx, this.charOffset, this.text);
    this.lastContext = hfEditContext(this.target, this.paraIdx, this.charOffset + this.text.length);
    return hfFnStubPosition(this.target.sectionIdx);
  }

  undo(wasm: WasmBridge): DocumentPosition {
    wasm.deleteTextInHeaderFooter(this.target.sectionIdx, this.target.isHeader, this.target.applyTo, this.paraIdx, this.charOffset, charCount(this.text));
    this.lastContext = hfEditContext(this.target, this.paraIdx, this.charOffset);
    return hfFnStubPosition(this.target.sectionIdx);
  }

  editContext(): EditContext { return this.lastContext; }
  mergeWith(): null { return null; }
}

/**
 * [Task #3212] 머리말/꼬리말 필드(쪽 번호·전체 쪽수·파일 이름) 삽입의 역연산 명령.
 *
 * 필드는 HF 문단에 제어문자 마커로 들어가므로 역연산은 그 문자 범위 삭제다. HF 모드
 * '내부' 편집이라 snapshot 으로 기록하면 undo 가 본문 분기를 타 HF 밖으로 튕겨나가므로,
 * editContext 를 노출하는 이 명령으로 기록해 undo/redo 가 HF 모드와 오프셋을 유지한다.
 */
export class InsertFieldInHeaderFooterCommand implements EditCommand {
  readonly type = 'insertFieldInHeaderFooter';
  readonly timestamp = Date.now();
  private lastContext: EditContext;

  constructor(
    private target: HeaderFooterEditTarget,
    private paraIdx: number,
    /** redo 시 native에 다시 넘길 원래 cursor 좌표 */
    private requestedCharOffset: number,
    /** undo가 marker를 지워야 하는 실제 모델 텍스트 좌표 */
    private insertedAt: number,
    private fieldType: number,
    /** 필드 마커가 실제 모델 텍스트에서 차지한 문자 수. */
    private markerLength: number,
    /** 삽입 직후 cursor가 돌아갈 좌표. */
    private cursorAfterOffset: number,
  ) {
    this.lastContext = hfEditContext(target, paraIdx, cursorAfterOffset);
  }

  execute(wasm: WasmBridge): DocumentPosition {
    const result = wasm.insertFieldInHf(
      this.target.sectionIdx, this.target.isHeader, this.target.applyTo,
      this.paraIdx, this.requestedCharOffset, this.fieldType,
    );
    if (result.ok) {
      this.insertedAt = result.insertedAt;
      this.markerLength = result.insertedLength;
      this.cursorAfterOffset = result.charOffset;
    }
    this.lastContext = hfEditContext(this.target, this.paraIdx, this.cursorAfterOffset);
    return hfFnStubPosition(this.target.sectionIdx);
  }

  undo(wasm: WasmBridge): DocumentPosition {
    wasm.deleteTextInHeaderFooter(this.target.sectionIdx, this.target.isHeader, this.target.applyTo, this.paraIdx, this.insertedAt, this.markerLength);
    this.lastContext = hfEditContext(this.target, this.paraIdx, this.requestedCharOffset);
    return hfFnStubPosition(this.target.sectionIdx);
  }

  editContext(): EditContext { return this.lastContext; }
  mergeWith(): null { return null; }
}

export class DeleteTextInHeaderFooterCommand implements EditCommand {
  readonly type = 'deleteTextInHeaderFooter';
  readonly timestamp = Date.now();
  private lastContext: EditContext;

  constructor(
    private target: HeaderFooterEditTarget,
    private paraIdx: number,
    private charOffset: number,
    private deletedText: string,
    /** undo(재삽입) 후 커서 오프셋 — 삭제 방향에 따라 호출부가 정한다(Backspace: charOffset+len, Delete: charOffset). */
    private cursorBeforeOffset: number,
  ) {
    this.lastContext = hfEditContext(target, paraIdx, charOffset);
  }

  execute(wasm: WasmBridge): DocumentPosition {
    wasm.deleteTextInHeaderFooter(this.target.sectionIdx, this.target.isHeader, this.target.applyTo, this.paraIdx, this.charOffset, charCount(this.deletedText));
    this.lastContext = hfEditContext(this.target, this.paraIdx, this.charOffset);
    return hfFnStubPosition(this.target.sectionIdx);
  }

  undo(wasm: WasmBridge): DocumentPosition {
    wasm.insertTextInHeaderFooter(this.target.sectionIdx, this.target.isHeader, this.target.applyTo, this.paraIdx, this.charOffset, this.deletedText);
    this.lastContext = hfEditContext(this.target, this.paraIdx, this.cursorBeforeOffset);
    return hfFnStubPosition(this.target.sectionIdx);
  }

  editContext(): EditContext { return this.lastContext; }
  mergeWith(): null { return null; }
}

export class SplitParagraphInHeaderFooterCommand implements EditCommand {
  readonly type = 'splitParagraphInHeaderFooter';
  readonly timestamp = Date.now();
  private lastContext: EditContext;

  constructor(
    private target: HeaderFooterEditTarget,
    private paraIdx: number,
    private charOffset: number,
    /** 분할로 생긴 다음 문단 인덱스(인라인 결과의 hfParaIndex). */
    private newParaIdx: number,
  ) {
    this.lastContext = hfEditContext(target, newParaIdx, 0);
  }

  execute(wasm: WasmBridge): DocumentPosition {
    wasm.splitParagraphInHeaderFooter(this.target.sectionIdx, this.target.isHeader, this.target.applyTo, this.paraIdx, this.charOffset);
    this.lastContext = hfEditContext(this.target, this.newParaIdx, 0);
    return hfFnStubPosition(this.target.sectionIdx);
  }

  undo(wasm: WasmBridge): DocumentPosition {
    wasm.mergeParagraphInHeaderFooter(this.target.sectionIdx, this.target.isHeader, this.target.applyTo, this.newParaIdx);
    this.lastContext = hfEditContext(this.target, this.paraIdx, this.charOffset);
    return hfFnStubPosition(this.target.sectionIdx);
  }

  editContext(): EditContext { return this.lastContext; }
  mergeWith(): null { return null; }
}

export class MergeParagraphInHeaderFooterCommand implements EditCommand {
  readonly type = 'mergeParagraphInHeaderFooter';
  readonly timestamp = Date.now();
  private lastContext: EditContext;

  constructor(
    private target: HeaderFooterEditTarget,
    /** 병합되는 문단 M (M 을 M-1 로 합침). */
    private paraIdx: number,
    /** 병합 후 이전 문단 인덱스(인라인 결과 hfParaIndex, = paraIdx-1). */
    private prevParaIdx: number,
    /** 병합 지점 오프셋(이전 문단의 원래 길이, 인라인 결과 charOffset). undo 분할점 + 병합 후 커서. */
    private mergeOffset: number,
    /** 병합 전 커서(undo 복원용) — Backspace: (paraIdx,0), Delete: (prevParaIdx,mergeOffset). */
    private beforeParaIdx: number,
    private beforeOffset: number,
    /** 병합으로 사라진 문단의 스코프 메타데이터 — undo 분할이 되돌린다 (Task #2342). */
    private removedParaMeta?: RemovedParaMeta,
  ) {
    this.lastContext = hfEditContext(target, prevParaIdx, mergeOffset);
  }

  execute(wasm: WasmBridge): DocumentPosition {
    wasm.mergeParagraphInHeaderFooter(this.target.sectionIdx, this.target.isHeader, this.target.applyTo, this.paraIdx);
    this.lastContext = hfEditContext(this.target, this.prevParaIdx, this.mergeOffset);
    return hfFnStubPosition(this.target.sectionIdx);
  }

  undo(wasm: WasmBridge): DocumentPosition {
    wasm.splitParagraphInHeaderFooter(this.target.sectionIdx, this.target.isHeader, this.target.applyTo, this.prevParaIdx, this.mergeOffset, this.removedParaMeta);
    this.lastContext = hfEditContext(this.target, this.beforeParaIdx, this.beforeOffset);
    return hfFnStubPosition(this.target.sectionIdx);
  }

  editContext(): EditContext { return this.lastContext; }
  mergeWith(): null { return null; }
}

// ── 각주/미주 ──────────────────────────────────────────────

export class InsertTextInFootnoteCommand implements EditCommand {
  readonly type = 'insertTextInFootnote';
  readonly timestamp = Date.now();
  private lastContext: EditContext;

  constructor(
    private target: FootnoteEditTarget,
    private innerParaIdx: number,
    private charOffset: number,
    private text: string,
  ) {
    this.lastContext = fnEditContext(target, innerParaIdx, charOffset + text.length);
  }

  execute(wasm: WasmBridge): DocumentPosition {
    wasm.insertTextInFootnote(this.target.sectionIdx, this.target.paraIdx, this.target.controlIdx, this.innerParaIdx, this.charOffset, this.text);
    this.lastContext = fnEditContext(this.target, this.innerParaIdx, this.charOffset + this.text.length);
    return hfFnStubPosition(this.target.sectionIdx);
  }

  undo(wasm: WasmBridge): DocumentPosition {
    wasm.deleteTextInFootnote(this.target.sectionIdx, this.target.paraIdx, this.target.controlIdx, this.innerParaIdx, this.charOffset, charCount(this.text));
    this.lastContext = fnEditContext(this.target, this.innerParaIdx, this.charOffset);
    return hfFnStubPosition(this.target.sectionIdx);
  }

  editContext(): EditContext { return this.lastContext; }
  mergeWith(): null { return null; }
}

export class DeleteTextInFootnoteCommand implements EditCommand {
  readonly type = 'deleteTextInFootnote';
  readonly timestamp = Date.now();
  private lastContext: EditContext;

  constructor(
    private target: FootnoteEditTarget,
    private innerParaIdx: number,
    private charOffset: number,
    private deletedText: string,
    private cursorBeforeOffset: number,
  ) {
    this.lastContext = fnEditContext(target, innerParaIdx, charOffset);
  }

  execute(wasm: WasmBridge): DocumentPosition {
    wasm.deleteTextInFootnote(this.target.sectionIdx, this.target.paraIdx, this.target.controlIdx, this.innerParaIdx, this.charOffset, charCount(this.deletedText));
    this.lastContext = fnEditContext(this.target, this.innerParaIdx, this.charOffset);
    return hfFnStubPosition(this.target.sectionIdx);
  }

  undo(wasm: WasmBridge): DocumentPosition {
    wasm.insertTextInFootnote(this.target.sectionIdx, this.target.paraIdx, this.target.controlIdx, this.innerParaIdx, this.charOffset, this.deletedText);
    this.lastContext = fnEditContext(this.target, this.innerParaIdx, this.cursorBeforeOffset);
    return hfFnStubPosition(this.target.sectionIdx);
  }

  editContext(): EditContext { return this.lastContext; }
  mergeWith(): null { return null; }
}

export class SplitParagraphInFootnoteCommand implements EditCommand {
  readonly type = 'splitParagraphInFootnote';
  readonly timestamp = Date.now();
  private lastContext: EditContext;

  constructor(
    private target: FootnoteEditTarget,
    private innerParaIdx: number,
    private charOffset: number,
    private newInnerParaIdx: number,
  ) {
    this.lastContext = fnEditContext(target, newInnerParaIdx, 0);
  }

  execute(wasm: WasmBridge): DocumentPosition {
    wasm.splitParagraphInFootnote(this.target.sectionIdx, this.target.paraIdx, this.target.controlIdx, this.innerParaIdx, this.charOffset);
    this.lastContext = fnEditContext(this.target, this.newInnerParaIdx, 0);
    return hfFnStubPosition(this.target.sectionIdx);
  }

  undo(wasm: WasmBridge): DocumentPosition {
    wasm.mergeParagraphInFootnote(this.target.sectionIdx, this.target.paraIdx, this.target.controlIdx, this.newInnerParaIdx);
    this.lastContext = fnEditContext(this.target, this.innerParaIdx, this.charOffset);
    return hfFnStubPosition(this.target.sectionIdx);
  }

  editContext(): EditContext { return this.lastContext; }
  mergeWith(): null { return null; }
}

export class MergeParagraphInFootnoteCommand implements EditCommand {
  readonly type = 'mergeParagraphInFootnote';
  readonly timestamp = Date.now();
  private lastContext: EditContext;

  constructor(
    private target: FootnoteEditTarget,
    private innerParaIdx: number,
    private prevInnerParaIdx: number,
    private mergeOffset: number,
    /** 병합 전 커서(undo 복원) — Backspace: (innerParaIdx,0), Delete: (prevInnerParaIdx,mergeOffset). */
    private beforeInnerParaIdx: number,
    private beforeOffset: number,
    /** 병합으로 사라진 문단의 스코프 메타데이터 — undo 분할이 되돌린다 (Task #2342). */
    private removedParaMeta?: RemovedParaMeta,
  ) {
    this.lastContext = fnEditContext(target, prevInnerParaIdx, mergeOffset);
  }

  execute(wasm: WasmBridge): DocumentPosition {
    wasm.mergeParagraphInFootnote(this.target.sectionIdx, this.target.paraIdx, this.target.controlIdx, this.innerParaIdx);
    this.lastContext = fnEditContext(this.target, this.prevInnerParaIdx, this.mergeOffset);
    return hfFnStubPosition(this.target.sectionIdx);
  }

  undo(wasm: WasmBridge): DocumentPosition {
    wasm.splitParagraphInFootnote(this.target.sectionIdx, this.target.paraIdx, this.target.controlIdx, this.prevInnerParaIdx, this.mergeOffset, this.removedParaMeta);
    this.lastContext = fnEditContext(this.target, this.beforeInnerParaIdx, this.beforeOffset);
    return hfFnStubPosition(this.target.sectionIdx);
  }

  editContext(): EditContext { return this.lastContext; }
  mergeWith(): null { return null; }
}

