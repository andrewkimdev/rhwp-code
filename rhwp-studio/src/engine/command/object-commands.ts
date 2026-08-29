import type { WasmBridge } from '@/core/wasm-bridge';
import type { DocumentPosition, CellPathLike } from '@/core/types';
import type { LineEndpoints as LineEndpointsLike } from '../object-drag-record';
import type { EditCommand } from './types';

// ─── 표 이동 명령 ─────────────────────────────────────

export class MoveTableCommand implements EditCommand {
  readonly type = 'moveTable';
  readonly timestamp: number;

  private resultPpi: number;
  private resultCi: number;

  constructor(
    private sec: number,
    private ppi: number,
    private ci: number,
    private deltaH: number,
    private deltaV: number,
    resultPpi: number,
    resultCi: number,
    timestamp?: number,
  ) {
    this.resultPpi = resultPpi;
    this.resultCi = resultCi;
    this.timestamp = timestamp ?? Date.now();
  }

  execute(wasm: WasmBridge): DocumentPosition {
    const result = wasm.moveTableOffset(this.sec, this.ppi, this.ci, this.deltaH, this.deltaV);
    this.resultPpi = result.ppi;
    this.resultCi = result.ci;
    return { sectionIndex: this.sec, paragraphIndex: this.resultPpi, charOffset: 0 };
  }

  undo(wasm: WasmBridge): DocumentPosition {
    // [Task #2903] execute() 는 moveTableOffset 반환값(result.ppi/ci)을 권위 소스로 삼아
    // this.resultPpi/resultCi 를 갱신하는데, undo() 는 동일한 반환값을 버리고 생성 시점의
    // stale this.ppi/this.ci 를 그대로 반환했다. 표 이동과 문단 구조 변경(삽입/삭제/병합)이
    // 같은 세션에서 섞이면 undo 후 커서가 존재하지 않거나 엉뚱한 문단을 가리킬 수 있다 —
    // execute() 와 대칭으로 반환값을 캡처해 this.ppi/this.ci 를 갱신한다.
    const result = wasm.moveTableOffset(this.sec, this.resultPpi, this.resultCi, -this.deltaH, -this.deltaV);
    this.ppi = result.ppi;
    this.ci = result.ci;
    return { sectionIndex: this.sec, paragraphIndex: this.ppi, charOffset: 0 };
  }

  mergeWith(other: EditCommand): EditCommand | null {
    if (!(other instanceof MoveTableCommand)) return null;
    if (other.sec !== this.sec) return null;
    // 연속 이동: 이전 결과 위치 == 다음 시작 위치
    if (other.ppi !== this.resultPpi || other.ci !== this.resultCi) return null;
    if (other.timestamp - this.timestamp > 500) return null;

    return new MoveTableCommand(
      this.sec, this.ppi, this.ci,
      this.deltaH + other.deltaH,
      this.deltaV + other.deltaV,
      other.resultPpi, other.resultCi,
      this.timestamp,
    );
  }
}

// ─── 그림 이동 명령 ─────────────────────────────────────

/** 두 cellPath 가 동일한지 비교 (undefined/빈배열은 본문(body-level)로 동일 취급) */
function sameCellPath(a?: CellPathLike, b?: CellPathLike): boolean {
  return JSON.stringify(a ?? []) === JSON.stringify(b ?? []);
}

/** 개체 이동 명령용 속성 조회 — cellPath 존재 시 by-path API 로 분기 */
function moveGetProps(
  wasm: WasmBridge, kind: 'image' | 'shape',
  sec: number, ppi: number, ci: number, cellPath?: CellPathLike,
): { horzOffset: number; vertOffset: number } {
  const nested = !!cellPath && cellPath.length > 0;
  if (kind === 'shape') {
    return nested ? wasm.getCellShapePropertiesByPath(sec, ppi, cellPath!, ci) : wasm.getShapeProperties(sec, ppi, ci);
  }
  return nested ? wasm.getCellPicturePropertiesByPath(sec, ppi, cellPath!, ci) : wasm.getPictureProperties(sec, ppi, ci);
}

/** 개체 이동 명령용 속성 변경 — cellPath 존재 시 by-path API 로 분기 */
function moveSetProps(
  wasm: WasmBridge, kind: 'image' | 'shape',
  sec: number, ppi: number, ci: number, cellPath: CellPathLike | undefined,
  props: Record<string, unknown>,
): void {
  const nested = !!cellPath && cellPath.length > 0;
  if (kind === 'shape') {
    if (nested) { wasm.setCellShapePropertiesByPath(sec, ppi, cellPath!, ci, props); return; }
    wasm.setShapeProperties(sec, ppi, ci, props);
    return;
  }
  if (nested) { wasm.setCellPicturePropertiesByPath(sec, ppi, cellPath!, ci, props); return; }
  wasm.setPictureProperties(sec, ppi, ci, props);
}

export class MovePictureCommand implements EditCommand {
  readonly type = 'movePicture';
  readonly timestamp: number;

  constructor(
    private sec: number,
    private ppi: number,
    private ci: number,
    private deltaH: number,
    private deltaV: number,
    private origHorzOffset: number,
    private origVertOffset: number,
    private cellPath?: CellPathLike,
    timestamp?: number,
  ) {
    this.timestamp = timestamp ?? Date.now();
  }

  execute(wasm: WasmBridge): DocumentPosition {
    const props = moveGetProps(wasm, 'image', this.sec, this.ppi, this.ci, this.cellPath);
    moveSetProps(wasm, 'image', this.sec, this.ppi, this.ci, this.cellPath, {
      horzOffset: props.horzOffset + this.deltaH,
      vertOffset: props.vertOffset + this.deltaV,
    });
    return { sectionIndex: this.sec, paragraphIndex: this.ppi, charOffset: 0 };
  }

  undo(wasm: WasmBridge): DocumentPosition {
    moveSetProps(wasm, 'image', this.sec, this.ppi, this.ci, this.cellPath, {
      horzOffset: this.origHorzOffset,
      vertOffset: this.origVertOffset,
    });
    return { sectionIndex: this.sec, paragraphIndex: this.ppi, charOffset: 0 };
  }

  mergeWith(other: EditCommand): EditCommand | null {
    if (!(other instanceof MovePictureCommand)) return null;
    if (other.sec !== this.sec || other.ppi !== this.ppi || other.ci !== this.ci) return null;
    if (!sameCellPath(other.cellPath, this.cellPath)) return null;
    if (other.timestamp - this.timestamp > 500) return null;

    return new MovePictureCommand(
      this.sec, this.ppi, this.ci,
      this.deltaH + other.deltaH,
      this.deltaV + other.deltaV,
      this.origHorzOffset,
      this.origVertOffset,
      this.cellPath,
      this.timestamp,
    );
  }
}

export class MoveShapeCommand implements EditCommand {
  readonly type = 'moveShape';
  readonly timestamp: number;

  constructor(
    private sec: number,
    private ppi: number,
    private ci: number,
    private deltaH: number,
    private deltaV: number,
    private origHorzOffset: number,
    private origVertOffset: number,
    private cellPath?: CellPathLike,
    timestamp?: number,
  ) {
    this.timestamp = timestamp ?? Date.now();
  }

  execute(wasm: WasmBridge): DocumentPosition {
    const props = moveGetProps(wasm, 'shape', this.sec, this.ppi, this.ci, this.cellPath);
    moveSetProps(wasm, 'shape', this.sec, this.ppi, this.ci, this.cellPath, {
      horzOffset: props.horzOffset + this.deltaH,
      vertOffset: props.vertOffset + this.deltaV,
    });
    return { sectionIndex: this.sec, paragraphIndex: this.ppi, charOffset: 0 };
  }

  undo(wasm: WasmBridge): DocumentPosition {
    moveSetProps(wasm, 'shape', this.sec, this.ppi, this.ci, this.cellPath, {
      horzOffset: this.origHorzOffset,
      vertOffset: this.origVertOffset,
    });
    return { sectionIndex: this.sec, paragraphIndex: this.ppi, charOffset: 0 };
  }

  mergeWith(other: EditCommand): EditCommand | null {
    if (!(other instanceof MoveShapeCommand)) return null;
    if (other.sec !== this.sec || other.ppi !== this.ppi || other.ci !== this.ci) return null;
    if (!sameCellPath(other.cellPath, this.cellPath)) return null;
    if (other.timestamp - this.timestamp > 500) return null;

    return new MoveShapeCommand(
      this.sec, this.ppi, this.ci,
      this.deltaH + other.deltaH,
      this.deltaV + other.deltaV,
      this.origHorzOffset,
      this.origVertOffset,
      this.cellPath,
      this.timestamp,
    );
  }
}


// ─── 개체 크기/위치 속성 변경 명령 ─────────────────────

export type ObjectResizeTarget = {
  sec: number;
  ppi: number;
  ci: number;
  type: string;
  cellPath?: CellPathLike;
  before: Record<string, unknown>;
  after: Record<string, unknown>;
};

/**
 * 그림/도형 리사이즈처럼 드래그 중 WASM에 이미 반영된 속성 변경을
 * Undo/Redo 스택에 기록하기 위한 명령.
 */
export class ResizeObjectCommand implements EditCommand {
  readonly type = 'resizeObject';
  readonly timestamp: number;

  constructor(
    private targets: ObjectResizeTarget[],
    timestamp?: number,
  ) {
    this.timestamp = timestamp ?? Date.now();
  }

  private setProps(wasm: WasmBridge, target: ObjectResizeTarget, props: Record<string, unknown>): void {
    if (target.type === 'shape' || target.type === 'line' || target.type === 'group' || target.type === 'ole') {
      if (target.cellPath && target.cellPath.length > 0) {
        wasm.setCellShapePropertiesByPath(target.sec, target.ppi, target.cellPath, target.ci, props);
        return;
      }
      wasm.setShapeProperties(target.sec, target.ppi, target.ci, props);
    } else {
      if (target.type === 'image' && target.cellPath && target.cellPath.length > 0) {
        wasm.setCellPicturePropertiesByPath(target.sec, target.ppi, target.cellPath, target.ci, props);
        return;
      }
      wasm.setPictureProperties(target.sec, target.ppi, target.ci, props);
    }
  }

  execute(wasm: WasmBridge): DocumentPosition {
    for (const target of this.targets) {
      this.setProps(wasm, target, target.after);
    }
    const first = this.targets[0];
    return { sectionIndex: first?.sec ?? 0, paragraphIndex: first?.ppi ?? 0, charOffset: 0 };
  }

  undo(wasm: WasmBridge): DocumentPosition {
    for (const target of this.targets) {
      this.setProps(wasm, target, target.before);
    }
    const first = this.targets[0];
    return { sectionIndex: first?.sec ?? 0, paragraphIndex: first?.ppi ?? 0, charOffset: 0 };
  }

  mergeWith(): null { return null; }
}

/**
 * [Task #2759] 직선/연결선 끝점 드래그를 Undo/Redo 스택에 기록하기 위한 명령.
 *
 * ResizeObjectCommand 와 동일하게 드래그 중 WASM 에 이미 반영된 변경을 kind:'record' 로
 * 사후 기록한다(execute 는 redo 경로에서만 재적용). before/after 는 글로벌 끝점 좌표
 * (HWPUNIT)이며 moveLineEndpoint 는 절대 좌표 setter 라 역연산이 자명하다.
 */
export class MoveLineEndpointCommand implements EditCommand {
  readonly type = 'moveLineEndpoint';
  readonly timestamp: number;

  constructor(
    private sec: number,
    private ppi: number,
    private ci: number,
    private before: LineEndpointsLike,
    private after: LineEndpointsLike,
    timestamp?: number,
  ) {
    this.timestamp = timestamp ?? Date.now();
  }

  execute(wasm: WasmBridge): DocumentPosition {
    wasm.moveLineEndpoint(this.sec, this.ppi, this.ci,
      this.after.sx, this.after.sy, this.after.ex, this.after.ey);
    return { sectionIndex: this.sec, paragraphIndex: this.ppi, charOffset: 0 };
  }

  undo(wasm: WasmBridge): DocumentPosition {
    wasm.moveLineEndpoint(this.sec, this.ppi, this.ci,
      this.before.sx, this.before.sy, this.before.ex, this.before.ey);
    return { sectionIndex: this.sec, paragraphIndex: this.ppi, charOffset: 0 };
  }

  mergeWith(): null { return null; }
}

/**
 * [Task #2374] 양식 값 변경 대상 — 본문 또는 표 셀 내 컨트롤 locator + 전/후 값 JSON.
 * before/after 는 setFormValue(InCell) 에 그대로 전달되는 JSON 문자열이다.
 */
export interface FormValueTarget {
  sec: number;
  para: number;
  ci: number;
  inCell?: { tablePara: number; tableCi: number; cellIdx: number; cellPara: number };
  beforeJson: string;
  afterJson: string;
}

/**
 * [Task #2374] 양식 컨트롤 값 변경의 경량 역연산 명령 (kind:'record' 용, #2337 계열).
 *
 * 뮤테이션은 클릭 핸들러가 직접 적용하고 이 명령은 기록만 담당한다(재실행 안 함).
 * 라디오 버튼처럼 다중 쓰기(그룹 해제 + 선택)인 조작은 targets 배열로 묶어 undo/redo 가
 * 그룹 상태를 원자적으로 왕복하게 한다 — 양식 모드에서는 snapshot 이 게이트에서 드롭되므로
 * record 가 유일한 기록 경로다.
 */
export class SetFormValueCommand implements EditCommand {
  readonly type = 'setFormValue';
  readonly timestamp: number;

  constructor(
    private targets: FormValueTarget[],
    private pos: DocumentPosition,
    timestamp?: number,
  ) {
    this.timestamp = timestamp ?? Date.now();
  }

  private apply(wasm: WasmBridge, t: FormValueTarget, json: string): void {
    if (t.inCell) {
      wasm.setFormValueInCell(t.sec, t.inCell.tablePara, t.inCell.tableCi, t.inCell.cellIdx, t.inCell.cellPara, t.ci, json);
    } else {
      wasm.setFormValue(t.sec, t.para, t.ci, json);
    }
  }

  execute(wasm: WasmBridge): DocumentPosition {
    for (const t of this.targets) this.apply(wasm, t, t.afterJson);
    return this.pos;
  }

  undo(wasm: WasmBridge): DocumentPosition {
    for (let i = this.targets.length - 1; i >= 0; i--) this.apply(wasm, this.targets[i], this.targets[i].beforeJson);
    return this.pos;
  }

  mergeWith(): null { return null; }
}
