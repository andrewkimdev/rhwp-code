import { WasmBridge } from '@/core/wasm-bridge';
import type { DeferredFocusedPagePatch } from '@/core/wasm-bridge';
import { EventBus } from '@/core/event-bus';
import { CursorState } from './cursor';
import { CaretRenderer } from './caret-renderer';
import { FieldMarkerRenderer } from './field-marker-renderer';
import { SelectionRenderer } from './selection-renderer';
import { CommandHistory } from './history';
import { DeleteSelectionCommand, ApplyCharFormatCommand, SnapshotCommand, SubmodeSnapshotCommand, TextMutationEffectAccumulator, IMMEDIATE_TEXT_MUTATION_EFFECTS, cellAxisPath, cellParaIndexOf } from './command';
import type { OperationDescriptor, ParaFormatTarget, RefreshPolicy, TextMutationEffects, EditCommand, EditContext, FormValueTarget } from './command';
import { selectCellIndicesInRange } from './cell-block-format';
import type { SelectedCellBlock } from './cell-block-format';
import { VirtualScroll } from '@/view/virtual-scroll';
import { ViewportManager } from '@/view/viewport-manager';
import type {
  DocumentPosition,
  CharProperties,
  ParaProperties,
  CursorRect,
  CellProperties,
  FormObjectHitResult,
  LayerNode,
  LayerTextRunOp,
  PageInfo,
} from '@/core/types';
import type { CommandDispatcher } from '@/command/dispatcher';
import type { EditorEditMode } from '@/command/types';
import { matchShortcut, defaultShortcuts } from '@/command/shortcut-map';
import type { ContextMenu, ContextMenuItem } from '@/ui/context-menu';
import type { CommandPalette } from '@/ui/command-palette';
import type { CellSelectionRenderer } from './cell-selection-renderer';
import type { TableObjectRenderer } from './table-object-renderer';
import type { TableResizeRenderer, BorderEdge } from './table-resize-renderer';
import type { CellBbox, CellPathLike } from '@/core/types';
import { showConfirm } from '@/ui/confirm-dialog';
import * as _mouse from './input-handler-mouse';
import * as _table from './input-handler-table';
import * as _keyboard from './input-handler-keyboard';
import * as _text from './input-handler-text';
import * as _picture from './input-handler-picture';
import * as _formOverlay from './input-handler-form-overlay';
import * as _dragScroll from './input-handler-drag-scroll';
import * as _contextMenu from './input-handler-context-menu';
import * as _placement from './input-handler-placement';
import * as _fieldNav from './input-handler-field-nav';
import * as _format from './input-handler-format';
import * as _afterEdit from './input-handler-after-edit';
import type { PageLocalTextEditOptions } from './input-edit-invalidation';
import type { NavigationKeyInput } from './navigation-keymap';
import { isPointNearBoxBorder } from './table-border-hit';
import { DeferredPaginationRunner } from './deferred-pagination-runner';
import { tableObjectClipboardTarget } from './table-object-clipboard-target';

const PX_TO_RAW_2X = 150;
const PX_TO_HWPUNIT = 75;

/**
 * 두 위치가 같은 셀 컨테이너에 있는지 전체 경로로 판정한다(#4272).
 * 마지막 cellParaIndex는 컨테이너 안의 현재 문단 축이므로 달라도 같은 셀이다.
 */
function isSameSelectionCellContainer(a: DocumentPosition, b: DocumentPosition): boolean {
  if (a.sectionIndex !== b.sectionIndex || a.parentParaIndex !== b.parentParaIndex) return false;
  const left = cellAxisPath(a);
  const right = cellAxisPath(b);
  if (left.length !== right.length || left.length === 0) return false;
  return left.every((entry, index) => {
    const other = right[index];
    return entry.controlIndex === other.controlIndex
      && entry.cellIndex === other.cellIndex
      && (index + 1 === left.length || entry.cellParaIndex === other.cellParaIndex);
  });
}

type FormatCopyState = {
  charProps: Partial<CharProperties>;
  paraProps: Partial<ParaProperties>;
  cellProps?: Partial<CellProperties>;
};

type PagePoint = {
  pageIdx: number;
  pageX: number;
  pageY: number;
};


const FORMAT_COPY_CHAR_KEYS: Array<keyof CharProperties> = [
  'fontSize',
  'bold',
  'italic',
  'underline',
  'strikethrough',
  'textColor',
  'shadeColor',
  'emboss',
  'engrave',
  'fontId',
  'fontIds',
  'underlineType',
  'underlineColor',
  'outlineType',
  'shadowType',
  'shadowColor',
  'shadowOffsetX',
  'shadowOffsetY',
  'strikeColor',
  'subscript',
  'superscript',
  'ratios',
  'spacings',
  'relativeSizes',
  'charOffsets',
  'emphasisDot',
  'underlineShape',
  'strikeShape',
  'kerning',
];

const FORMAT_COPY_PARA_KEYS: Array<keyof ParaProperties> = [
  'alignment',
  'lineSpacing',
  'lineSpacingType',
  'marginLeft',
  'marginRight',
  'indent',
  'spacingBefore',
  'spacingAfter',
  'headType',
  'paraLevel',
  'numberingId',
  'widowOrphan',
  'keepWithNext',
  'keepLines',
  'pageBreakBefore',
  'fontLineHeight',
  'singleLine',
  'autoSpaceKrEn',
  'autoSpaceKrNum',
  'verticalAlign',
  'englishBreakUnit',
  'koreanBreakUnit',
  'borderConnect',
  'borderIgnoreMargin',
];

const FORMAT_COPY_CELL_KEYS: Array<keyof CellProperties> = [
  'paddingLeft',
  'paddingRight',
  'paddingTop',
  'paddingBottom',
  'applyInnerMargin',
  'verticalAlign',
  'textDirection',
  'isHeader',
  'cellProtect',
  'fieldName',
  'editableInForm',
  'borderFillId',
];

function pickDefined<T extends object, K extends keyof T>(source: T, keys: K[]): Partial<T> {
  const result: Partial<T> = {};
  for (const key of keys) {
    if (source[key] !== undefined) result[key] = source[key];
  }
  return result;
}

function pxToRaw2x(px: number): number {
  return Math.round(px * PX_TO_RAW_2X);
}

function pxToRaw(px: number): number {
  return Math.round(px * PX_TO_HWPUNIT);
}

function normalizeFormatCopyParaProps(props: Partial<ParaProperties>): Partial<ParaProperties> {
  const normalized = { ...props };
  if (props.marginLeft !== undefined) normalized.marginLeft = pxToRaw2x(props.marginLeft);
  if (props.marginRight !== undefined) normalized.marginRight = pxToRaw2x(props.marginRight);
  if (props.indent !== undefined) normalized.indent = pxToRaw2x(props.indent);
  if (props.spacingBefore !== undefined) normalized.spacingBefore = pxToRaw(props.spacingBefore);
  if (props.spacingAfter !== undefined) normalized.spacingAfter = pxToRaw(props.spacingAfter);
  return normalized;
}

/** 클릭 커서 배치 + 키보드 입력을 처리한다 */
export class InputHandler {
  private cursor: CursorState;
  private caret: CaretRenderer;
  private fieldMarker: FieldMarkerRenderer;
  private selectionRenderer: SelectionRenderer;
  private history: CommandHistory;
  private textarea: HTMLTextAreaElement;
  private active = false;
  private insertMode = true;  // true=삽입, false=수정(덮어쓰기)
  private editMode: EditorEditMode = 'normal';
  /** 마지막 셀 키 (눈금자 셀 bbox 중복 조회 방지) */
  private lastCellKey: string | null = null;
  /** [#4162] 선택 없이 지정한 글자 서식 — 다음 삽입 런에 적용 예약(캐럿 대기 글자 모양) */
  private pendingCharShape: Partial<CharProperties> | null = null;
  /** pendingCharShape 를 예약·연장한 캐럿 위치. 여기서 벗어나면(진짜 이동) 예약을 버린다. */
  private pendingCharShapeAnchor: DocumentPosition | null = null;
  private dispatcher: CommandDispatcher | null = null;
  private contextMenu: ContextMenu | null = null;
  private commandPalette: CommandPalette | null = null;
  private cellSelectionRenderer: CellSelectionRenderer | null = null;
  private tableObjectRenderer: TableObjectRenderer | null = null;
  private tableResizeRenderer: TableResizeRenderer | null = null;
  private pictureObjectRenderer: TableObjectRenderer | null = null;
  /** 마지막 rhwp-studio 내부 복사의 시스템 클립보드 marker token */
  private rhwpClipboardToken: string | null = null;
  /** 누름틀 시작 경계에서 왼쪽/Home 이동으로 필드 밖에 머문 상태 */
  private fieldStartExitKey: string | null = null;
  /** 누름틀 끝 경계에서 오른쪽 이동으로 필드 밖에 머문 상태 */
  private fieldEndExitKey: string | null = null;
  /** 누름틀을 포함한 붙여넣기 직후 마지막 필드 끝을 바깥 위치로 고정한다 */
  private pastedFieldEndOutsidePending = false;
  /** 모양 복사로 기억한 글자/문단 모양 */
  private formatCopyState: FormatCopyState | null = null;

  // 마우스 드래그 선택 상태
  private isDragging = false;
  private dragRafId = 0; // requestAnimationFrame throttle용
  private dragAutoScrollRafId = 0;
  private dragLastClientX = 0;
  private dragLastClientY = 0;
  private cellSelectionDragState: {
    startClientX: number;
    startClientY: number;
    lastClientX: number;
    lastClientY: number;
    startRow: number;
    startCol: number;
    lastRow: number;
    lastCol: number;
    isDragging: boolean;
  } | null = null;
  private cellSelectionDragCandidate: {
    startClientX: number;
    startClientY: number;
    startRow: number;
    startCol: number;
  } | null = null;

  // 표 경계선 hover 상태
  private resizeHoverRafId = 0;
  private cachedTableRef: { sec: number; ppi: number; ci: number; pageHint?: number } | null = null;
  private cachedCellBboxes: CellBbox[] | null = null;
  private protectedCellHitCache: { key: string; protected: boolean } | null = null;
  private protectedCellHoverEl: HTMLDivElement | null = null;
  private deferredPaginationFlushTimer: ReturnType<typeof setTimeout> | null = null;
  private deferredPaginationPending = false;
  private readonly deferredPaginationRunner: DeferredPaginationRunner;
  private rawTextMutationEffects = new TextMutationEffectAccumulator();
  private pendingFocusedPagePatch: DeferredFocusedPagePatch | null = null;

  // 표 경계선 리사이즈 드래그 상태
  private isResizeDragging = false;
  private resizeDragState: {
    edge: BorderEdge;
    tableRef: { sec: number; ppi: number; ci: number };
    bboxes: CellBbox[];
    pageBboxes: CellBbox[];
    affectedCellIndices: number[];
    borderOriginalPos: number;
    minResizePos: number;
    maxResizePos: number;
    resizeTarget?: { cellIdx: number; side: 'start' | 'end' } | null;
    singleCellTarget?: { cellIdx: number; side: 'start' | 'end' } | null;
    shiftResize?: boolean;
  } | null = null;
  private tableLocalResizeSegments = new Set<string>();

  // 표 이동 드래그 상태
  private isMoveDragging = false;
  private moveDragState: {
    tableRef: { sec: number; ppi: number; ci: number };
    startPpi: number;  // 드래그 시작 시 ppi (Undo용)
    startPageX: number;
    startPageY: number;
    lastPageX: number;
    lastPageY: number;
    totalDeltaH: number;  // 누적 HWPUNIT 델타 (Undo용)
    totalDeltaV: number;
  } | null = null;

  // 그림 삽입 배치 모드 상태
  private imagePlacementMode = false;
  private imagePlacementData: {
    data: Uint8Array; ext: string; fileName: string;
    naturalWidth: number; naturalHeight: number;
  } | null = null;
  private imagePlacementDrag: {
    startClientX: number; startClientY: number;
    currentClientX: number; currentClientY: number;
    isDragging: boolean;
  } | null = null;
  private imagePlacementOverlay: HTMLDivElement | null = null;

  // 도형/글상자 삽입 배치 모드 상태
  private shapePlacementType: string = 'rectangle'; // 'rectangle' | 'ellipse' | 'line' | 'arc' | 'polygon' | 'textbox' | 'connector-*'
  private textboxPlacementMode = false;
  private textboxPlacementDrag: {
    startClientX: number; startClientY: number;
    currentClientX: number; currentClientY: number;
    isDragging: boolean;
  } | null = null;
  private textboxPlacementOverlay: HTMLDivElement | null = null;

  // 연결선 드로잉 모드 상태
  private connectorDrawingMode = false;
  private connectorType: string = 'connector-straight';
  private connectorStartRef: { sec: number; ppi: number; ci: number; pointIndex: number; x: number; y: number } | null = null;
  private connectorOverlay: HTMLDivElement | null = null;

  // 다각형 그리기 모드 상태
  private polygonDrawingMode = false;
  private polygonPoints: { x: number; y: number }[] = [];
  private polygonOverlay: HTMLDivElement | null = null;
  private polygonMousePos: { x: number; y: number } | null = null;

  // 그림/글상자 핸들 드래그 리사이즈 상태
  private isPictureResizeDragging = false;
  private pictureResizeState: {
    dir: string;
    ref: { sec: number; ppi: number; ci: number; type: 'image' | 'shape' | 'equation' | 'group' | 'line' | 'ole'; cellPath?: CellPathLike; headerFooter?: { kind: 'header' | 'footer'; outerParaIdx: number; outerControlIdx: number } };
    origWidth: number;
    origHeight: number;
    origHorzOffset?: number;
    origVertOffset?: number;
    startClientX: number;
    startClientY: number;
    pageIndex: number;
    bbox: { x: number; y: number; w: number; h: number };
    /** 다중 선택 리사이즈 시 각 개체의 원래 크기/위치 */
    multiRefs?: { sec: number; ppi: number; ci: number; type: string; origWidth: number; origHeight: number; origHorzOffset: number; origVertOffset: number; bboxX: number; bboxY: number }[];
  } | null = null;

  // 그림/글상자 이동 드래그 상태
  private isPictureMoveDragging = false;
  private pictureMoveState: {
    ref: { sec: number; ppi: number; ci: number; type: 'image' | 'shape' | 'equation' | 'group' | 'line' | 'ole'; cellPath?: CellPathLike; headerFooter?: { kind: 'header' | 'footer'; outerParaIdx: number; outerControlIdx: number } };
    origHorzOffset: number;
    origVertOffset: number;
    startPageX: number;
    startPageY: number;
    lastPageX: number;
    lastPageY: number;
    totalDeltaH: number;
    totalDeltaV: number;
    pageIndex: number;
    /** 다중 선택 이동 시 각 개체의 원래 offset 기록 */
    multiRefs?: { sec: number; ppi: number; ci: number; type: string; origHorzOffset: number; origVertOffset: number }[];
  } | null = null;

  // 그림/글상자 회전 드래그 상태
  private isPictureRotateDragging = false;
  private pictureRotateState: {
    ref: { sec: number; ppi: number; ci: number; type: 'image' | 'shape' | 'equation' | 'group' | 'line' | 'ole'; cellPath?: CellPathLike; headerFooter?: { kind: 'header' | 'footer'; outerParaIdx: number; outerControlIdx: number } };
    origAngle: number;      // 드래그 시작 시 원래 회전각 (도)
    centerX: number;        // 도형 중심 (scroll-content 좌표, px)
    centerY: number;
    startAngle: number;     // 드래그 시작 시 마우스→중심 각도 (rad)
    pageIndex: number;
  } | null = null;

  // 직선 끝점 드래그 상태
  private isLineEndpointDragging = false;
  private lineEndpointState: {
    ref: { sec: number; ppi: number; ci: number; type: string };
    endpoint: 'start' | 'end';
    pageIndex: number;
    pageLeft: number;
    pageOffset: number;
    zoom: number;
    // [Task #2759] 드래그 시작 시 캡처한 원래 끝점(글로벌 HWPUNIT) — 종료 시 Undo 기록의 before.
    orig: { sx: number; sy: number; ex: number; ey: number };
  } | null = null;

  // 양식 개체 오버레이
  private formOverlay: HTMLElement | null = null;

  // [Task #394] 셀 진입 자동 ON 로직 비활성화 — checkTransparentBordersTransition 와 동시 주석 처리.
  // 되돌리려면 아래 3 개 변수 + 호출 지점 + 메서드 본체 + 이벤트 핸들러의 주석을 동시에 해제.
  // // 투명선 자동 활성화 상태
  // private wasInCell = false;
  // private manualTransparentBorders = false;
  // private autoTransparentBorders = false;

  // IME 조합 상태
  private isComposing = false;
  private compositionAnchor: DocumentPosition | null = null;
  private compositionLength = 0; // 문서에 삽입된 조합 텍스트 길이
  private _lastCompositionText = '';
  private _lastComposedText = '';
  private _pendingNavAfterIME: NavigationKeyInput | null = null;
  // iOS 폴백: composition 이벤트 없이 input만으로 한글 조합 처리
  private _iosComposing = false;
  private _iosAnchor: DocumentPosition | null = null;
  private _iosBeforePageIndex: number | undefined = undefined;
  private _iosLength = 0;
  private _iosPrevText = '';
  private _iosInputTimer: any = null;
  private _iosRequiresFullRefresh = false;
  private _isIOS = /iPad|iPhone|iPod/.test(navigator.userAgent) ||
    (navigator.platform === 'MacIntel' && navigator.maxTouchPoints > 1);

  private onClickBound: (e: MouseEvent) => void;
  private onDblClickBound: (e: MouseEvent) => void;
  private onKeyDownBound: (e: KeyboardEvent) => void;
  private onInputBound: (e?: Event) => void;
  private onCompositionStartBound: () => void;
  private onCompositionEndBound: () => void;
  private onInputBlurBound: () => void;
  private onCopyBound: (e: ClipboardEvent) => void;
  private onCutBound: (e: ClipboardEvent) => void;
  private onPasteBound: (e: ClipboardEvent) => void;
  private onContextMenuBound: (e: MouseEvent) => void;
  private onMouseMoveBound: (e: MouseEvent) => void;
  private onMouseUpBound: (e: MouseEvent) => void;
  private onF11InterceptBound: (e: KeyboardEvent) => void;

  constructor(
    private container: HTMLElement,
    private wasm: WasmBridge,
    private eventBus: EventBus,
    private virtualScroll: VirtualScroll,
    private viewportManager: ViewportManager,
  ) {
    this.cursor = new CursorState(wasm);
    this.caret = new CaretRenderer(container, virtualScroll);
    this.fieldMarker = new FieldMarkerRenderer(container, virtualScroll);
    this.selectionRenderer = new SelectionRenderer(container, virtualScroll);
    this.history = new CommandHistory();
    this.deferredPaginationRunner = new DeferredPaginationRunner(
      wasm,
      (result) => this.completeResumablePagination(result.pageCount),
      () => this.fallbackFromResumablePagination(),
    );

    // Hidden input 요소 생성
    // iOS WebKit에서는 <textarea>로 composition 이벤트가 발생하지 않으므로
    // contentEditable <div>를 사용하고 .value 프록시를 추가한다.
    const isIOS = /iPad|iPhone|iPod/.test(navigator.userAgent) ||
      (navigator.platform === 'MacIntel' && navigator.maxTouchPoints > 1);
    const inputHost = this.container.closest('main') ?? document.body;

    if (isIOS) {
      const div = document.createElement('div');
      div.contentEditable = 'true';
      div.style.cssText =
        'position:absolute;left:0;top:0;width:2em;height:1.5em;' +
        'color:transparent;background:transparent;caret-color:transparent;' +
        'border:none;outline:none;overflow:hidden;white-space:nowrap;' +
        'z-index:10;font-size:16px;padding:0;margin:0;';
      div.setAttribute('autocomplete', 'off');
      div.setAttribute('autocorrect', 'off');
      div.setAttribute('autocapitalize', 'off');
      div.setAttribute('spellcheck', 'false');
      div.setAttribute('inputmode', 'text');
      div.setAttribute('aria-label', '문서 편집 입력');
      inputHost.appendChild(div);
      // textarea 인터페이스 호환을 위한 프록시
      Object.defineProperty(div, 'value', {
        get() { return div.textContent || ''; },
        set(v: string) { div.textContent = v; },
      });
      this.textarea = div as unknown as HTMLTextAreaElement;
    } else {
      this.textarea = document.createElement('textarea');
      this.textarea.style.cssText =
        'position:fixed;left:-9999px;top:0;width:1px;height:1px;opacity:0;';
      this.textarea.setAttribute('autocomplete', 'off');
      this.textarea.setAttribute('autocorrect', 'off');
      this.textarea.setAttribute('autocapitalize', 'off');
      this.textarea.setAttribute('spellcheck', 'false');
      this.textarea.setAttribute('aria-label', '문서 편집 입력');
      inputHost.appendChild(this.textarea);
    }

    this.onClickBound = this.onClick.bind(this);
    this.onDblClickBound = this.onDblClick.bind(this);
    this.onKeyDownBound = this.onKeyDown.bind(this);
    this.onInputBound = this.onInput.bind(this);
    this.onCompositionStartBound = this.onCompositionStart.bind(this);
    this.onCompositionEndBound = this.onCompositionEnd.bind(this);
    this.onInputBlurBound = () => {
      this.flushDeferredPaginationIfNeeded('input-blur', false);
      // 편집 표면(textarea) 밖으로 포커스가 나가면 활성 누름틀을 해제해
      // 안내문을 다시 표시한다 (#클릭 후 클릭-away 시 안내문이 안 돌아오는 결함).
      this.clearActiveFieldMarker();
    };
    this.onCopyBound = this.onCopy.bind(this);
    this.onCutBound = this.onCut.bind(this);
    this.onPasteBound = this.onPaste.bind(this);
    this.onContextMenuBound = this.onContextMenu.bind(this);
    this.onMouseMoveBound = this.onMouseMove.bind(this);
    this.onMouseUpBound = this.onMouseUp.bind(this);

    // F11 브라우저 fullscreen 방지 (capture 단계에서 차단) + 컨트롤 선택 실행
    this.onF11InterceptBound = (e: KeyboardEvent) => {
      if (e.key === 'F11') {
        e.preventDefault();
        e.stopPropagation();
        if (e.shiftKey) {
          _keyboard.handleShiftF11.call(this);
        } else {
          _keyboard.handleF11.call(this);
        }
      }
    };
    document.addEventListener('keydown', this.onF11InterceptBound, true);

    container.addEventListener('mousedown', this.onClickBound);
    container.addEventListener('dblclick', this.onDblClickBound);
    container.addEventListener('contextmenu', this.onContextMenuBound);
    container.addEventListener('mousemove', this.onMouseMoveBound);
    this.textarea.addEventListener('keydown', this.onKeyDownBound);
    this.textarea.addEventListener('input', this.onInputBound);
    this.textarea.addEventListener('compositionstart', this.onCompositionStartBound);
    this.textarea.addEventListener('compositionend', this.onCompositionEndBound);
    this.textarea.addEventListener('blur', this.onInputBlurBound);
    this.textarea.addEventListener('copy', this.onCopyBound);
    this.textarea.addEventListener('cut', this.onCutBound);
    this.textarea.addEventListener('paste', this.onPasteBound);

    // 줌 변경 시 캐럿/선택 마커 위치 갱신
    eventBus.on('zoom-changed', () => {
      if (this.active) {
        const rect = this.cursor.getRect();
        if (rect) {
          this.caret.updatePosition(this.viewportManager.getZoom());
        }
        // 필드 마커도 줌에 맞게 갱신
        if (this.fieldMarker.isVisible) {
          this.updateFieldMarkers();
        }
      }
      // 텍스트 블럭 선택 줌 동기화
      if (this.cursor.hasSelection()) {
        this.updateSelection();
      }
      // F5 셀 선택 줌 동기화
      if (this.cursor.isInCellSelectionMode()) {
        this.updateCellSelection();
      }
      // 도형/표 선택 핸들 줌 동기화
      if (this.cursor.isInPictureObjectSelection()) {
        this.renderPictureObjectSelection();
      }
      if (this.cursor.isInTableObjectSelection()) {
        this.renderTableObjectSelection();
      }
    });

    eventBus.on('document-view-changed', () => {
      if (!this.active) return;
      requestAnimationFrame(() => this.updateCaret(true));
    });

    // 표 객체 선택 변경 시 렌더링
    eventBus.on('table-object-selection-changed', (selected) => {
      if (selected) {
        this.renderTableObjectSelection();
      } else {
        this.tableObjectRenderer?.clear();
      }
    });

    // 문서 변경 후 그림/표 선택 마커 재렌더링
    eventBus.on('document-changed', () => {
      this.protectedCellHitCache = null;
      this.protectedCellHoverEl?.remove();
      this.protectedCellHoverEl = null;
      requestAnimationFrame(() => {
        if (this.cursor.isInPictureObjectSelection()) {
          this.renderPictureObjectSelection();
        }
        if (this.cursor.isInTableObjectSelection()) {
          this.renderTableObjectSelection();
        }
      });
    });
    eventBus.on('create-new-document', () => {
      this.clearTableResizeRuntimeCache();
    });
    eventBus.on('open-document-bytes', () => {
      this.clearTableResizeRuntimeCache();
    });

    // [Task #394] 셀 진입 자동 ON 로직 비활성화 — manual 추적 불필요.
    // transparent-borders-changed 이벤트 자체는 view.ts 에서 emit 되므로 보존됨 (다른 구독자가 사용 가능).
    // // 투명선 수동 토글 상태 추적
    // eventBus.on('transparent-borders-changed', (show) => {
    //   this.manualTransparentBorders = show as boolean;
    // });

    // Toolbar에서 서식 적용 요청 수신 (글꼴명, 크기, 색상 — 커맨드 시스템 미경유)
    eventBus.on('format-char', (props) => {
      if (!this.active) return;
      if (this.editMode === 'form') return;
      // [#4162] 선택이 없어도(캐럿만) applyCharFormat 이 캐럿 대기 서식으로 예약한다 —
      // 여기서 선택 유무로 걸러내면 글꼴/크기/색 피커가 다시 무언 no-op 이 된다.
      this.applyCharFormat(props as Partial<CharProperties>);
      // 서식바 조작으로 빠진 포커스를 항상 복원
      this.focusTextarea();
    });
  }

  /** 클릭 이벤트 처리 — hitTest로 커서 배치 */
  private onClick(e: MouseEvent): void {
    _mouse.onClick.call(this, e);
  }

  /** 우클릭 컨텍스트 메뉴 처리 */
  private onContextMenu(e: MouseEvent): void {
    _mouse.onContextMenu.call(this, e);
  }

  /** 더블클릭: 글상자 객체 선택 → 텍스트 편집 진입 */
  private onDblClick(e: MouseEvent): void {
    _mouse.onDblClick.call(this, e);
  }

  /** 마우스 이동: 드래그 선택 또는 표 객체 선택 중 핸들 위 커서 변경 */
  private onMouseMove(e: MouseEvent): void {
    _mouse.onMouseMove.call(this, e);
  }

  /** 표 경계선 hover 감지 처리 */
  private handleResizeHover(e: MouseEvent): void {
    _mouse.handleResizeHover.call(this, e);
  }

  /** 리사이즈 드래그를 시작한다 */
  private startResizeDrag(
    edge: BorderEdge,
    pageX: number, pageY: number,
    pageBboxes: CellBbox[],
    shiftResize = false,
  ): void {
    _table.startResizeDrag.call(this, edge, pageX, pageY, pageBboxes, shiftResize);
  }

  /** 리사이즈 드래그 중 마커 위치를 갱신한다 */
  private updateResizeDrag(e: MouseEvent): void {
    _table.updateResizeDrag.call(this, e);
  }

  /** 리사이즈 드래그를 완료하고 셀 크기를 적용한다 */
  private finishResizeDrag(e: MouseEvent): void {
    _table.finishResizeDrag.call(this, e);
  }

  /** 리사이즈 드래그 상태를 초기화한다 */
  private cleanupResizeDrag(): void {
    _table.cleanupResizeDrag.call(this);
  }

  // ─── 격자 이동 크기 (mm) ───────────────────────────────
  private gridStepMm = 3; // 기본 3mm

  /** 격자 간격 설정 (mm 단위) */
  setGridStep(mm: number): void { this.gridStepMm = mm; }

  /** 현재 격자 간격 반환 (mm 단위) */
  getGridStepMm(): number { return this.gridStepMm; }

  /** 문서 스냅샷 전환 뒤 표 resize 런타임 캐시를 비운다. */
  private clearTableResizeRuntimeCache(): void {
    this.tableLocalResizeSegments.clear();
    this.cachedTableRef = null;
    this.cachedCellBboxes = null;
    this.tableResizeRenderer?.clear();
  }

  // ─── 그림 삽입 배치 모드 ───────────────────────────────

  /** 그림 배치 모드 진입: 파일 선택 후 호출. 마우스로 영역 지정 대기 */
  enterImagePlacementMode(data: Uint8Array, ext: string, naturalWidth: number, naturalHeight: number, fileName: string = ''): void {
    _placement.enterImagePlacementMode.call(this, data, ext, naturalWidth, naturalHeight, fileName);
  }

  /** 외부 파일 드롭 그림 삽입: 한컴처럼 원본 크기, 글자처럼 취급으로 바로 넣는다. */
  insertDroppedImageAtClientPoint(
    data: Uint8Array,
    ext: string,
    naturalWidth: number,
    naturalHeight: number,
    fileName: string,
    clientX: number,
    clientY: number,
  ): { ok: boolean; error?: string } {
    return _placement.insertDroppedImageAtClientPoint.call(this, data, ext, naturalWidth, naturalHeight, fileName, clientX, clientY);
  }

  /** 그림 배치 모드 취소 */
  private cancelImagePlacement(): void {
    _table.cancelImagePlacement.call(this);
  }

  /** 그림 배치 사각형 오버레이 표시/갱신 */
  private showImagePlacementOverlay(x1: number, y1: number, x2: number, y2: number): void {
    _table.showImagePlacementOverlay.call(this, x1, y1, x2, y2);
  }

  /** 그림 배치 오버레이 제거 */
  private hideImagePlacementOverlay(): void {
    _table.hideImagePlacementOverlay.call(this);
  }

  /** 그림 배치 완료: 마우스업 시 호출 */
  private finishImagePlacement(e: MouseEvent): void {
    _table.finishImagePlacement.call(this, e);
  }

  // ─── 글상자 삽입 배치 모드 ───────────────────────────────

  /** 글상자 배치 모드 진입: 메뉴에서 호출. 마우스로 영역 지정 대기 */
  enterTextboxPlacementMode(): void {
    _placement.enterTextboxPlacementMode.call(this);
  }

  /** 도형 배치 모드 진입 (도형 타입 지정) */
  enterShapePlacementMode(shapeType: string): void {
    _placement.enterShapePlacementMode.call(this, shapeType);
  }

  /** 다각형 그리기: 꼭짓점 추가 (클릭) */
  private polygonAddPoint(clientX: number, clientY: number): void {
    _placement.polygonAddPoint.call(this, clientX, clientY);
  }

  /** 다각형 그리기: 마우스 이동 시 프리뷰 갱신 */
  private updatePolygonOverlay(mx: number, my: number): void {
    _placement.updatePolygonOverlay.call(this, mx, my);
  }

  /** 다각형 그리기: 완료 (더블클릭 또는 시작점 근접) */
  private finishPolygonDrawing(): void {
    _placement.finishPolygonDrawing.call(this);
  }

  /** 다각형 그리기: 취소 */
  private cancelPolygonDrawing(): void {
    _placement.cancelPolygonDrawing.call(this);
  }

  /** 글상자 배치 모드 취소 */
  private cancelTextboxPlacement(): void {
    _placement.cancelTextboxPlacement.call(this);
  }

  /** 도형 배치 오버레이 표시/갱신 (도형 타입별 SVG) */
  private showTextboxPlacementOverlay(x1: number, y1: number, x2: number, y2: number, shiftKey = false): void {
    _placement.showTextboxPlacementOverlay.call(this, x1, y1, x2, y2, shiftKey);
  }

  /** 도형 배치 오버레이 제거 */
  private hideTextboxPlacementOverlay(): void {
    _placement.hideTextboxPlacementOverlay.call(this);
  }

  /** 글상자 배치 완료: 마우스업 시 호출 */
  private finishTextboxPlacement(e: MouseEvent): void {
    _placement.finishTextboxPlacement.call(this, e);
  }

  /** 표 객체 선택 모드에서 방향키로 표 위치 이동 */
  private moveSelectedTable(key: 'ArrowUp' | 'ArrowDown' | 'ArrowLeft' | 'ArrowRight'): void {
    _table.moveSelectedTable.call(this, key);
  }

  /** 그림 객체 선택 모드에서 방향키로 그림 위치 이동 */
  private moveSelectedPicture(key: 'ArrowUp' | 'ArrowDown' | 'ArrowLeft' | 'ArrowRight'): void {
    _table.moveSelectedPicture.call(this, key);
  }

  /** 그림 객체 선택 모드에서 Shift+방향키로 개체 크기 조절 (#1231) */
  private resizeSelectedPicture(key: 'ArrowUp' | 'ArrowDown' | 'ArrowLeft' | 'ArrowRight'): void {
    _picture.resizeSelectedPicture.call(this, key);
  }

  /** 마우스 드래그로 표 이동 — 드래그 중 갱신 */
  private updateMoveDrag(e: MouseEvent): void {
    _table.updateMoveDrag.call(this, e);
  }

  /** 마우스 드래그로 표 이동 — 드래그 종료 */
  private finishMoveDrag(): void {
    _table.finishMoveDrag.call(this);
  }

  /** 셀 선택 모드에서 Ctrl+방향키로 셀 크기 조절 */
  private resizeCellByKeyboard(key: 'ArrowUp' | 'ArrowDown' | 'ArrowLeft' | 'ArrowRight'): void {
    _table.resizeCellByKeyboard.call(this, key);
  }

  private resizeCellLocalByKeyboard(key: 'ArrowUp' | 'ArrowDown' | 'ArrowLeft' | 'ArrowRight'): void {
    _table.resizeCellLocalByKeyboard.call(this, key);
  }

  private resizeCellBoundaryByKeyboard(key: 'ArrowUp' | 'ArrowDown' | 'ArrowLeft' | 'ArrowRight'): void {
    _table.resizeCellBoundaryByKeyboard.call(this, key);
  }

  private resizeTableProportional(key: 'ArrowUp' | 'ArrowDown' | 'ArrowLeft' | 'ArrowRight'): void {
    _table.resizeTableProportional.call(this, key);
  }

  /** 마우스 버튼 놓기: 드래그 선택 종료 */
  private onMouseUp(_e: MouseEvent): void {
    _mouse.onMouseUp.call(this, _e);
  }

  /** [Task #2759] 직선 끝점 드래그 종료 — 끝점 이동을 Undo 히스토리에 기록 */
  private finishLineEndpointDrag(): void {
    _mouse.finishLineEndpointDrag.call(this);
  }

  /** 마우스 이벤트에서 hitTest 결과를 반환한다 */
  private hitTestFromEvent(e: MouseEvent): DocumentPosition | null {
    return this.hitTestFromClientPoint(e.clientX, e.clientY);
  }

  /** 화면 좌표에서 hitTest 결과를 반환한다 */
  private hitTestFromClientPoint(clientX: number, clientY: number): DocumentPosition | null {
    const pagePoint = this.pagePointFromClientPoint(clientX, clientY);
    if (!pagePoint) return null;
    try {
      return this.wasm.hitTest(pagePoint.pageIdx, pagePoint.pageX, pagePoint.pageY);
    } catch {
      return null;
    }
  }

  private pagePointFromClientPoint(clientX: number, clientY: number): PagePoint | null {
    const zoom = this.viewportManager.getZoom();
    const scrollContent = this.container.querySelector('#scroll-content');
    if (!scrollContent) return null;
    const contentRect = scrollContent.getBoundingClientRect();
    // [Task #661 + #685+#689 통합] PR #718 영역 의 clientX/Y parameter 영역 +
    // PR #693 영역 의 getPageAtPoint (그리드 모드 click 좌표 정합) 보존.
    const contentX = clientX - contentRect.left;
    const contentY = clientY - contentRect.top;
    const pageIdx = this.virtualScroll.getPageAtPoint(contentX, contentY);
    const pageOffset = this.virtualScroll.getPageOffset(pageIdx);
    const pageLeft = this.virtualScroll.getPageLeftResolved(pageIdx, scrollContent.clientWidth);
    const pageX = (contentX - pageLeft) / zoom;
    const pageY = (contentY - pageOffset) / zoom;
    return { pageIdx, pageX, pageY };
  }

  private getPageInfoForDrop(pageIdx: number): PageInfo | null {
    try {
      return this.wasm.getPageInfo(pageIdx);
    } catch {
      return null;
    }
  }

  /** 화면 좌표에서 각주/미주 내부 hitTest 결과를 반환한다. */
  private footnoteHitTestFromClientPoint(clientX: number, clientY: number): {
    pageIdx: number;
    hit: {
      hit: boolean;
      fnParaIndex?: number;
      charOffset?: number;
      footnoteIndex?: number;
      cursorRect?: { pageIndex: number; x: number; y: number; height: number };
    };
  } | null {
    const zoom = this.viewportManager.getZoom();
    const scrollContent = this.container.querySelector('#scroll-content');
    if (!scrollContent) return null;
    const contentRect = scrollContent.getBoundingClientRect();
    const contentX = clientX - contentRect.left;
    const contentY = clientY - contentRect.top;
    const pageIdx = this.virtualScroll.getPageAtPoint(contentX, contentY);
    const pageOffset = this.virtualScroll.getPageOffset(pageIdx);
    const pageLeft = this.virtualScroll.getPageLeftResolved(pageIdx, scrollContent.clientWidth);
    const pageX = (contentX - pageLeft) / zoom;
    const pageY = (contentY - pageOffset) / zoom;
    try {
      return { pageIdx, hit: this.wasm.hitTestInFootnote(pageIdx, pageX, pageY) };
    } catch {
      return null;
    }
  }

  /** 텍스트 선택 드래그를 시작한다 */
  private startTextSelectionDrag(e: MouseEvent): void {
    _dragScroll.startTextSelectionDrag.call(this, e);
  }

  /** 텍스트 선택 드래그 포인터 좌표를 갱신한다 */
  private updateTextSelectionDragPointer(e: MouseEvent): void {
    _dragScroll.updateTextSelectionDragPointer.call(this, e);
  }

  /** 마지막 포인터 좌표 기준으로 드래그 선택 focus를 갱신한다 */
  private updateTextSelectionDragFromPointer(): void {
    _dragScroll.updateTextSelectionDragFromPointer.call(this);
  }

  /** 텍스트 선택 드래그를 종료한다 */
  private stopTextSelectionDrag(): void {
    _dragScroll.stopTextSelectionDrag.call(this);
  }

  private getTextSelectionDragScrollDeltaY(): number {
    return _dragScroll.getTextSelectionDragScrollDeltaY.call(this);
  }

  private scaleTextSelectionDragScrollStep(distance: number): number {
    return _dragScroll.scaleTextSelectionDragScrollStep.call(this, distance);
  }

  private updateTextSelectionDragAutoScroll(): void {
    _dragScroll.updateTextSelectionDragAutoScroll.call(this);
  }

  private runTextSelectionDragAutoScroll(): void {
    _dragScroll.runTextSelectionDragAutoScroll.call(this);
  }

  private stopTextSelectionDragAutoScroll(): void {
    _dragScroll.stopTextSelectionDragAutoScroll.call(this);
  }

  /** 클릭 좌표가 표 외곽 경계선 위인지 판별한다 (페이지 좌표 기준) */
  private isTableBorderClick(
    pageIdx: number,
    pageX: number, pageY: number,
    sec: number, ppi: number, ci: number,
  ): boolean {
    try {
      const bbox = this.wasm.getTableBBoxAtPage(sec, ppi, ci, pageIdx);
      return isPointNearBoxBorder(pageX, pageY, bbox);
    } catch {
      return false;
    }
  }

  /** [Task #919] 클릭 좌표가 (sec, ppi, ci) 글상자의 외곽 경계선 위인지 판정.
   *  isShapeBorderClick(picture 모듈) 의 sec/ppi/ci 변형 — getShapeBBox API 사용
   *  tolerance 5px 한컴 정합 (Native bbox + 5px 안). */
  isShapeBorderClickByRef(
    pageX: number, pageY: number,
    sec: number, ppi: number, ci: number,
  ): boolean {
    try {
      const bbox = this.wasm.getShapeBBox(sec, ppi, ci);
      const tolerance = 5;
      const nearLeft = Math.abs(pageX - bbox.x) <= tolerance;
      const nearRight = Math.abs(pageX - (bbox.x + bbox.width)) <= tolerance;
      const nearTop = Math.abs(pageY - bbox.y) <= tolerance;
      const nearBottom = Math.abs(pageY - (bbox.y + bbox.height)) <= tolerance;
      const inVertRange = pageY >= bbox.y - tolerance && pageY <= bbox.y + bbox.height + tolerance;
      const inHorzRange = pageX >= bbox.x - tolerance && pageX <= bbox.x + bbox.width + tolerance;
      return (nearLeft && inVertRange) || (nearRight && inVertRange) ||
             (nearTop && inHorzRange) || (nearBottom && inHorzRange);
    } catch {
      return false;
    }
  }

  /** [Task #919] 클릭 좌표 근처에 글상자가 있는지 확인 (글상자 바깥에서 외곽 근처 클릭) */
  findShapeByOuterClick(
    pageX: number, pageY: number,
    sec: number, paragraphIndex: number,
  ): { sec: number; ppi: number; ci: number } | null {
    // 현재 문단 및 인접 문단 (±2) 검사 — findTableByOuterClick 동일 패턴
    for (let offset = 0; offset <= 2; offset++) {
      const candidates = offset === 0
        ? [paragraphIndex]
        : [paragraphIndex - offset, paragraphIndex + offset];
      for (const ppi of candidates) {
        if (ppi < 0) continue;
        // Shape 컨트롤은 paragraph 의 어느 위치든 있을 수 있으므로 0..N 시도
        for (let ci = 0; ci < 10; ci++) {
          if (this.isShapeBorderClickByRef(pageX, pageY, sec, ppi, ci)) {
            return { sec, ppi, ci };
          }
        }
      }
    }
    return null;
  }

  /**
   * 클릭 좌표 근처에 표가 있는지 확인한다 (표 바깥에서 클릭한 경우).
   * 페이지 레이아웃의 실제 표 컨트롤 인덱스를 우선 사용하고, 보조로 주변 문단을 검사한다.
   */
  private findTableByOuterClick(
    pageIdx: number,
    pageX: number, pageY: number,
    sec: number, paragraphIndex: number,
  ): { sec: number; ppi: number; ci: number } | null {
    try {
      const layout = this.wasm.getPageControlLayout(pageIdx);
      const isNearBorder = (x: number, y: number, w: number, h: number): boolean => {
        return isPointNearBoxBorder(pageX, pageY, { x, y, width: w, height: h });
      };

      for (const item of layout.controls) {
        if (item.type !== 'table') continue;
        if (item.paraIdx === undefined || item.controlIdx === undefined) continue;
        if ((item.secIdx ?? sec) !== sec) continue;
        if (Math.abs(item.paraIdx - paragraphIndex) > 2) continue;
        if (isNearBorder(item.x, item.y, item.w, item.h)) {
          return { sec: item.secIdx ?? sec, ppi: item.paraIdx, ci: item.controlIdx };
        }
      }
    } catch { /* 레이아웃 조회 실패 시 주변 문단 스캔으로 보조 */ }

    // 현재 문단 및 인접 문단 (±2) 검사. 컨트롤 인덱스는 0 고정이 아니므로 일부 범위를 시도한다.
    for (let offset = 0; offset <= 2; offset++) {
      const candidates = offset === 0
        ? [paragraphIndex]
        : [paragraphIndex - offset, paragraphIndex + offset];
      for (const ppi of candidates) {
        if (ppi < 0) continue;
        for (let ci = 0; ci < 10; ci++) {
          if (this.isTableBorderClick(pageIdx, pageX, pageY, sec, ppi, ci)) {
            return { sec, ppi, ci };
          }
        }
      }
    }
    return null;
  }

  /** 표 객체 선택 상태 컨텍스트 메뉴 항목 */
  private getTableObjectContextMenuItems(): ContextMenuItem[] {
    return _contextMenu.getTableObjectContextMenuItems.call(this);
  }

  /** 그림 객체 선택 컨텍스트 메뉴 항목 */
  private getPictureObjectContextMenuItems(): ContextMenuItem[] {
    return _contextMenu.getPictureObjectContextMenuItems.call(this);
  }

  /** 표 셀 내부 컨텍스트 메뉴 항목 */
  private getTableContextMenuItems(): ContextMenuItem[] {
    return _contextMenu.getTableContextMenuItems.call(this);
  }

  /** 일반 컨텍스트 메뉴 항목 */
  private getDefaultContextMenuItems(): ContextMenuItem[] {
    return _contextMenu.getDefaultContextMenuItems.call(this);
  }

  /** 특수 키 처리 (Backspace, Enter, 화살표, Ctrl+Z/Y) */
  private onKeyDown(e: KeyboardEvent): void {
    _keyboard.onKeyDown.call(this, e);
  }

  /** Ctrl/Meta 단축키 처리 */
  private handleCtrlKey(e: KeyboardEvent): void {
    _keyboard.handleCtrlKey.call(this, e);
  }

  /** Ctrl+A: 전체 선택 */
  private handleSelectAll(): void {
    _keyboard.handleSelectAll.call(this);
  }

  // ─── 클립보드 이벤트 처리 ─────────────────────────────

  /** 복사 이벤트 처리 */
  private onCopy(e: ClipboardEvent): void {
    _keyboard.onCopy.call(this, e);
  }

  /** 잘라내기 이벤트 처리 */
  private onCut(e: ClipboardEvent): void {
    _keyboard.onCut.call(this, e);
  }

  /** 붙여넣기 이벤트 처리 */
  private onPaste(e: ClipboardEvent): void {
    _keyboard.onPaste.call(this, e);
  }

  // ─── 서식 적용 ─────────────────────────────────────────

  /** 선택 범위에 글자 서식을 적용한다. 선택이 없으면 캐럿 대기 서식으로 예약한다. */
  private applyCharFormat(props: Partial<CharProperties>): void {
    _format.applyCharFormat.call(this, props);
  }

  /** [#4162][#4271 리뷰] 선택 없이 지정한 글자 서식을 다음 삽입 런에 적용하도록 예약한다.
   *
   * 새 props 를 병합하기 전에 getPendingCharShape() 로 낡은 예약을 먼저 걷어낸다 — 안 그러면
   * A 에서 예약한 서식이 B 로 캐럿이 실제로 이동한 뒤에도 raw 필드에 남아 있다가, B 에서
   * 새로 예약할 때 그대로 병합돼(굵게@A + 색@B) 요청한 적 없는 서식이 B 로 샌다. */
  private stagePendingCharShape(props: Partial<CharProperties>): void {
    _format.stagePendingCharShape.call(this, props);
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
  getPendingCharShape(): Partial<CharProperties> | undefined {
    return _format.getPendingCharShape.call(this);
  }

  /** [#4162] Command 를 거치지 않는 삽입(IME 조합)에 예약 서식을 직접 적용한다. */
  applyPendingCharShapeToRange(anchor: DocumentPosition, count: number): void {
    _format.applyPendingCharShapeToRange.call(this, anchor, count);
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
  private advancePendingCharShapeAnchor(oldPos: DocumentPosition, newPos: DocumentPosition): void {
    _format.advancePendingCharShapeAnchor.call(this, oldPos, newPos);
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
  private applyCharFormatToCellBlock(block: SelectedCellBlock, props: Partial<CharProperties>): void {
    _format.applyCharFormatToCellBlock.call(this, block, props);
  }

  /** [#4151] 셀 블록 서식의 토글 방향·툴바 상태 기준: 블록 첫 셀의 첫 글자 서식. */
  private getCharPropertiesAtCellBlockAnchor(block: SelectedCellBlock): CharProperties {
    return _format.getCharPropertiesAtCellBlockAnchor.call(this, block);
  }

  /** 토글 서식 적용 (상호 배타 처리 포함) */
  private applyToggleFormat(prop: 'bold' | 'italic' | 'underline' | 'strikethrough' | 'emboss' | 'engrave' | 'outline' | 'superscript' | 'subscript'): void {
    _format.applyToggleFormat.call(this, prop);
  }

  /**
   * [#4162] 실제로 문자가 선택된 범위만 돌려준다. anchor 만 있고 focus 와 같은 위치
   * (빈 선택, 드래그 없이 클릭만 한 상태)는 선택 없음으로 접는다 — getSelectionOrdered()
   * 는 anchor 유무만 보고 non-null 을 돌려줘, 그대로 쓰면 서식 커맨드가 빈 range 로
   * 조용히 no-op 된다.
   */
  private getNonEmptySelection(): { start: DocumentPosition; end: DocumentPosition } | null {
    return _format.getNonEmptySelection.call(this);
  }

  /** 커서 위치의 글자 서식을 조회한다. 선택이 있으면 선택 첫 글자, 없으면 캐럿 앞 글자 기준. */
  private getCharPropertiesAtCursor(): CharProperties {
    return _format.getCharPropertiesAtCursor.call(this);
  }

  /** 커서 위치 문단에 문단 서식을 적용한다 */
  private applyParaFormat(props: Record<string, unknown>): void {
    _format.applyParaFormat.call(this, props);
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
  private applyParaFormatInNoteOrHeader(props: Record<string, unknown>): boolean {
    return _format.applyParaFormatInNoteOrHeader.call(this, props);
  }
  private executeParaFormatCommand(targets: ParaFormatTarget[], props: Record<string, unknown>): boolean {
    return _format.executeParaFormatCommand.call(this, targets, props);
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
  private getSelectedCellBlock(): SelectedCellBlock | null {
    return _format.getSelectedCellBlock.call(this);
  }
  private getParaFormatTargetsAtCursor(): ParaFormatTarget[] {
    return _format.getParaFormatTargetsAtCursor.call(this);
  }

  /** 셀 블록 안 모든 셀의 모든 문단을 문단 서식 대상으로 만든다 */
  private getParaFormatTargetsForCellBlock(block: SelectedCellBlock): ParaFormatTarget[] {
    return _format.getParaFormatTargetsForCellBlock.call(this, block);
  }
  private getParaFormatTargetsForRange(start: DocumentPosition, end: DocumentPosition): ParaFormatTarget[] {
    return _format.getParaFormatTargetsForRange.call(this, start, end);
  }

  /** 한컴식 Shift+Tab: 첫 줄 시작 위치를 기준으로 문단 내어쓰기를 설정한다. */
  applyHangingIndentAtCursor(): boolean {
    return _format.applyHangingIndentAtCursor.call(this);
  }

  /** 커서 위치 서식 상태를 Toolbar에 알린다 */
  private emitCursorFormatState(): void {
    _format.emitCursorFormatState.call(this);
  }

  /** 선택 영역을 삭제한다 */
  private deleteSelection(): void {
    const sel = this.cursor.getSelectionOrdered();
    if (!sel) return;
    if (!this.canDeleteSelectionInFormMode()) return;

    const cmd = new DeleteSelectionCommand(sel.start, sel.end);
    this.cursor.clearSelection();
    this.executeOperation({ kind: 'command', command: cmd });
  }

  /** Undo 처리 */
  private handleUndo(): void {
    this.flushDeferredPaginationIfNeeded('before-undo', false);
    const newPos = this.history.undo(this.wasm);
    if (newPos) {
      this.prepareTextMutationBeforeCursor(IMMEDIATE_TEXT_MUTATION_EFFECTS);
      this.clearTableResizeRuntimeCache();
      this.resetDerivedStateAfterHistoryJump();
      // [Task #2337] 방금 되돌린 커맨드가 HF/FN 편집이면 그 커서 모드로 복원(본문 moveTo 대신).
      this.restoreEditContextAfterHistory(this.history.peekRedoTop(), newPos);
      this.afterEdit();
    }
  }

  /** Redo 처리 */
  private handleRedo(): void {
    this.flushDeferredPaginationIfNeeded('before-redo', false);
    const newPos = this.history.redo(this.wasm);
    if (newPos) {
      const boundaryHandled = this.prepareTextMutationBeforeCursor(
        this.history.consumeLastExecutionEffects(),
      );
      this.clearTableResizeRuntimeCache();
      this.resetDerivedStateAfterHistoryJump();
      // [Task #2337] 방금 다시 실행한 커맨드가 HF/FN 편집이면 그 커서 모드로 복원.
      this.restoreEditContextAfterHistory(this.history.peekUndoTop(), newPos);
      this.afterEdit(!boundaryHandled);
    }
  }

  /**
   * [Task #2337] undo/redo 후 편집 컨텍스트(본문 vs HF/FN) 복원.
   *
   * 본문 커맨드(editContext 없음)는 기존대로 HF/FN 모드를 빠져나오고 본문 커서를
   * 이동한다. HF/FN 편집 커맨드는 해당 모드로 (재)진입해 커서 오프셋을 복원하며,
   * 이때 본문 moveTo 는 건너뛴다(HF/FN 커서는 별도 상태라 본문 위치 이동이 부적합).
   * 모드 전환 시 mode-change 이벤트를 emit 해 툴바/오버레이가 따라오게 한다.
   * enterHeaderFooterMode/enterFootnoteMode 는 _savedBodyPosition 을 덮어쓰므로 이미
   * 같은 모드일 때는 재진입하지 않고 switch/set 만 한다.
   */
  private restoreEditContextAfterHistory(cmd: EditCommand | null, bodyPos: DocumentPosition): void {
    const ctx: EditContext | null = cmd?.editContext?.() ?? null;

    if (ctx?.mode === 'headerFooter') {
      if (this.cursor.isInFootnote()) {
        this.cursor.exitFootnoteMode();
        this.eventBus.emit('footnoteModeChanged', false);
      }
      const sameTarget = this.cursor.isInHeaderFooter()
        && this.cursor.hfSectionIdx === ctx.sectionIdx
        && (this.cursor.headerFooterMode === 'header') === ctx.isHeader
        && this.cursor.hfApplyTo === ctx.applyTo;
      if (!sameTarget) {
        if (this.cursor.isInHeaderFooter()) {
          this.cursor.switchHeaderFooterTarget(ctx.isHeader, ctx.sectionIdx, ctx.applyTo);
        } else {
          this.cursor.enterHeaderFooterMode(ctx.isHeader, ctx.sectionIdx, ctx.applyTo);
        }
        // 진입/전환 양쪽 모두 mode-change 를 알려 툴바/오버레이가 stale 하지 않게 한다.
        this.eventBus.emit('headerFooterModeChanged', ctx.isHeader ? 'header' : 'footer');
      }
      this.cursor.setHfCursorPosition(ctx.paraIdx, ctx.charOffset);
      return;
    }

    if (ctx?.mode === 'footnote') {
      if (this.cursor.isInHeaderFooter()) {
        this.cursor.exitHeaderFooterMode();
        this.eventBus.emit('headerFooterModeChanged', 'none');
      }
      const sameTarget = this.cursor.isInFootnote()
        && this.cursor.fnSectionIdx === ctx.sectionIdx
        && this.cursor.fnParaIdx === ctx.paraIdx
        && this.cursor.fnControlIdx === ctx.controlIdx;
      if (!sameTarget) {
        if (this.cursor.isInFootnote()) this.cursor.exitFootnoteMode();
        this.cursor.enterFootnoteMode(ctx.sectionIdx, ctx.paraIdx, ctx.controlIdx, ctx.footnoteIndex, ctx.pageNum);
        this.eventBus.emit('footnoteModeChanged', true);
      }
      this.cursor.setFnCursorPosition(ctx.innerParaIdx, ctx.charOffset);
      return;
    }

    // 본문 커맨드 — HF/FN 모드였으면 빠져나오고 본문 커서 이동.
    if (this.cursor.isInHeaderFooter()) {
      this.cursor.exitHeaderFooterMode();
      this.eventBus.emit('headerFooterModeChanged', 'none');
    }
    if (this.cursor.isInFootnote()) {
      this.cursor.exitFootnoteMode();
      this.eventBus.emit('footnoteModeChanged', false);
    }
    this.cursor.moveTo(bodyPos);
  }

  /**
   * [Task #2303 → #2339] 히스토리 점프(undo/redo)는 문단/컨트롤 구성을 되돌리므로,
   * 위치 기반 파생 상태가 이전 문서를 가리킨 채 stale 로 남아 다음 조작에서 WASM 예외나
   * 무언 오편집을 일으킨다. 커서-소유 파생 상태(개체/표 선택·텍스트 선택·셀 블록 선택)를
   * 여기서 일괄 해제하고, 외부 모듈(find-dialog 등)이 정리할 수 있도록 'history-jumped'
   * 를 emit 한다. 이후 stale 파생 상태는 handleUndo/Redo 수정 없이 이 이벤트를 구독만
   * 하면 된다(계급 2 근절·확장점). 비선택/비활성 항목은 no-op.
   */
  private resetDerivedStateAfterHistoryJump(): void {
    // [#2303] 위치 기반 개체/표 선택 ref({sec, ppi, ci})는 undo 로 어긋나 이후 개체 속성
    // 커맨드가 WASM 예외("지정된 컨트롤이 그림이 아닙니다")로 실패 → 선택 모드 해제.
    if (this.cursor.isInPictureObjectSelection()) {
      this.cursor.exitPictureObjectSelection();
      this.pictureObjectRenderer?.clear();
      this.eventBus.emit('picture-object-selection-changed', false);
    }
    if (this.cursor.isInTableObjectSelection()) {
      this.cursor.exitTableObjectSelection();
      this.eventBus.emit('table-object-selection-changed', false);
    }
    // [#2339] 텍스트 선택 anchor/focus 는 undo 로 축소된 문서에서 유령 범위가 되어 이후
    // Bold/Backspace 시 WASM 예외·본 적 없는 범위 무언 삭제를 유발한다. 본문 블록 선택
    // (F3 확장 단계·F5)도 _blockSelectionMode/_expandPhase 가 stale 로 남으면 이후 F5 첫
    // 입력이 모드 종료에만 소비되고 F3 이 미처리 단계로 넘어가므로, 선택만이 아니라 단계까지
    // 초기화하는 exitBlockSelectionMode 로 해제(내부에서 clearSelection 수행 — 안전 최소).
    this.cursor.exitBlockSelectionMode();
    // [#2339] F5 셀 블록 선택은 커서 ctx 해제만으로 stale 병합을 막지만, 하이라이트 DIV 는
    // 렌더러 clear 까지 해야 사라진다(afterEdit·document-changed 경로가 셀 렌더러 미처리) →
    // 고스트 오버레이 제거를 위해 렌더러도 함께 clear.
    this.cursor.exitCellSelectionMode();
    this.cellSelectionRenderer?.clear();
    // [#2339] 외부 위치-기반 파생 상태(find currentHit 등)를 구독으로 정리하는 확장점.
    this.eventBus.emit('history-jumped');
  }

  /**
   * 편집 작업 통합 라우터.
   * 호출부는 OperationDescriptor로 "무엇을 하려는가"만 서술하고,
   * 라우터가 적절한 Undo 전략을 자동 선택한다.
   */
  executeOperation(desc: OperationDescriptor): void {
    if (!this.isOperationAllowedInEditMode(desc)) return;
    switch (desc.kind) {
      case 'command': {
        const beforePos = this.cursor.getPosition();
        const beforePageIndex = this.cursor.getRect()?.pageIndex;
        const keepFieldStartOutside = (desc.command.type === 'insertText' || desc.command.type === 'deleteText')
          && this.isExitedFieldStartPosition(beforePos);
        if (keepFieldStartOutside) {
          this.wasm.clearActiveField();
        }
        const newPos = this.history.execute(desc.command, this.wasm);
        const boundaryHandled = this.prepareTextMutationBeforeCursor(
          this.history.consumeLastExecutionEffects(),
        );
        // 글자/문단 서식 변경은 문서 구조 불변 → 선택 영역 유지
        if (desc.command.type !== 'applyCharFormat' && desc.command.type !== 'applyParaFormat') {
          this.cursor.moveTo(newPos);
          this.cursor.resetPreferredX();
        }
        // [#4162] 삽입으로 캐럿이 전진한 것은 "이동"이 아니므로 예약을 이어간다.
        if (desc.command.type === 'insertText') {
          this.advancePendingCharShapeAnchor(beforePos, newPos);
        }
        if (keepFieldStartOutside) {
          this.markCurrentFieldStartOutside();
        }
        this.refreshAfterOperation(desc.meta?.refresh, 'auto', desc.command.type, beforePos, newPos, {
          ...desc.command.getPageLocalTextEditOptions?.(),
          beforePageIndex,
          afterPageIndex: this.cursor.getRect()?.pageIndex,
        }, boundaryHandled);
        break;
      }
      case 'snapshot': {
        const cursorBefore = this.cursor.getPosition();
        // 일반 snapshot은 구조 편집의 본문 복귀 의미를 유지한다. HF/FN 안에서만
        // 문맥을 보존하는 전용 명령을 써서 undo/redo의 대상 범위를 호출부가 드러낸다.
        const cmd = desc.editContext
          ? new SubmodeSnapshotCommand(
              desc.operationType,
              cursorBefore,
              cursorBefore,
              desc.operation,
              desc.editContext,
            )
          : new SnapshotCommand(desc.operationType, cursorBefore, cursorBefore, desc.operation);
        const newPos = this.history.execute(cmd, this.wasm);
        const markPastedFieldEndOutside = this.pastedFieldEndOutsidePending;
        // 무변경 경로에서도 pending 플래그는 소비한다 — 남겨 두면 다음 연산으로 샌다.
        this.pastedFieldEndOutsidePending = false;
        // [Task #2370] operation 이 무변경(null)을 알리면 기록도 리프레시도 없다.
        // 문서가 그대로이므로 다시 그릴 것이 없고, 커서도 움직이지 않았다.
        if (cmd.isNoOp()) break;
        this.cursor.moveTo(newPos);
        this.cursor.resetPreferredX();
        if (markPastedFieldEndOutside) {
          this.markCurrentFieldEndOutside();
        }
        this.refreshAfterOperation(desc.meta?.refresh, 'full', desc.operationType, cursorBefore, newPos);
        break;
      }
      case 'record': {
        const pos = this.cursor.getPosition();
        this.history.recordWithoutExecute(desc.command, this.wasm);
        this.refreshAfterOperation(desc.meta?.refresh, 'none', desc.command.type, pos, pos);
        break;
      }
    }
  }

  /** Backspace 처리 */
  private handleBackspace(pos: DocumentPosition, inCell: boolean): void {
    _text.handleBackspace.call(this, pos, inCell);
  }

  /** Delete 처리 */
  private handleDelete(pos: DocumentPosition, inCell: boolean): void {
    _text.handleDelete.call(this, pos, inCell);
  }

  /** IME 조합 시작 */
  private onCompositionStart(): void {
    _text.onCompositionStart.call(this);
  }

  /** IME 조합 완료 — 조합 텍스트를 Command로 기록 */
  private onCompositionEnd(): void {
    _text.onCompositionEnd.call(this);
  }

  /** 위치에서 텍스트를 읽는다 (본문/셀 자동 분기) */
  private getTextAt(pos: DocumentPosition, count: number): string {
    return _text.getTextAt.call(this, pos, count);
  }

  /** 텍스트 입력 처리 (textarea input 이벤트) */
  private onInput(e?: Event): void {
    _text.onInput.call(this, e as InputEvent);
  }

  /** 위치에 텍스트를 삽입한다 (WASM 직접 호출, IME 조합용) */
  private insertTextAtRaw(pos: DocumentPosition, text: string): void {
    this.rawTextMutationEffects.add(_text.insertTextAtRaw.call(this, pos, text));
  }

  private replaceTextAtRaw(pos: DocumentPosition, deleteCount: number, text: string): void {
    this.rawTextMutationEffects.add(
      _text.replaceTextAtRaw.call(this, pos, deleteCount, text),
    );
  }

  /** 위치에서 텍스트를 삭제한다 (WASM 직접 호출, IME 조합용) */
  private deleteTextAt(pos: DocumentPosition, count: number): void {
    this.rawTextMutationEffects.add(_text.deleteTextAt.call(this, pos, count));
  }

  /** textarea에 포커스를 설정한다 (iOS 호환) */
  private focusTextarea(): void {
    _afterEdit.focusTextarea.call(this);
  }

  /** 편집 후 처리: 재렌더링 + 캐럿 갱신 */
  private afterEdit(flushDeferredPagination = true): void {
    _afterEdit.afterEdit.call(this, flushDeferredPagination);
  }

  /** 셀 내부 단일 텍스트 편집 후 처리: 현재 페이지 canvas만 갱신한다. */
  private afterPageLocalEdit(): void {
    _afterEdit.afterPageLocalEdit.call(this);
  }

  /** 셀 안 새 줄이 기존 가시 높이를 넘으면 즉시 전체 표 레이아웃을 다시 계산한다. */
  private flushDeferredPaginationForCellOverflow(): boolean {
    return _afterEdit.flushDeferredPaginationForCellOverflow.call(this);
  }

  private scheduleDeferredPaginationFlush(): void {
    _afterEdit.scheduleDeferredPaginationFlush.call(this);
  }

  /**
   * [#3412] idle 자동 flush 대상 여부.
   *
   * 전진 중인 재개형 잡이 있으면 idle flush 는 그 잡을 취소하고 같은 일을 동기로 다시
   * 하는 셈이라 예약하지 않는다. 문서 크기 상한은 위 상수 주석 참조.
   */
  private shouldAutoFlushDeferredPagination(): boolean {
    return _afterEdit.shouldAutoFlushDeferredPagination.call(this);
  }

  private cancelDeferredPaginationFlush(): void {
    _afterEdit.cancelDeferredPaginationFlush.call(this);
  }

  /** deferred mutation을 cursor lookup 전에 등록하고 flow 경계에서는 resumable job을 시작한다. */
  private prepareTextMutationBeforeCursor(effects: TextMutationEffects): boolean {
    return _afterEdit.prepareTextMutationBeforeCursor.call(this, effects);
  }

  private completeResumablePagination(_pageCount: number): void {
    _afterEdit.completeResumablePagination.call(this, _pageCount);
  }

  private fallbackFromResumablePagination(): void {
    _afterEdit.fallbackFromResumablePagination.call(this);
  }

  private resetRawTextMutationEffects(): void {
    _afterEdit.resetRawTextMutationEffects.call(this);
  }

  private consumeRawTextMutationBeforeCursor(): boolean {
    return _afterEdit.consumeRawTextMutationBeforeCursor.call(this);
  }

  hasDeferredPaginationPending(): boolean {
    return _afterEdit.hasDeferredPaginationPending.call(this);
  }

  flushDeferredPaginationIfNeeded(reason = 'manual', emitChange = true): boolean {
    return _afterEdit.flushDeferredPaginationIfNeeded.call(this, reason, emitChange);
  }

  /**
   * [#4031] 동기 full pagination을 소유하는 structural command(셀 Enter 분할)가 확정된
   * 경로에서, 곧 폐기될 stale deferred job을 계산 완료 없이 취소한다.
   * `wasm.flushDeferredPagination()`을 호출하지 않는 것이 flush 경로와의 유일한 차이다.
   * runner.cancel()이 전진 중인 WASM resumable job까지 취소한다.
   * `deferredPaginationPending`은 유지한다 — mutation이 실패하면 다음 boundary flush가
   * 기존 barrier 의미론으로 복구하도록 fail-closed로 남긴다.
   */
  cancelDeferredPaginationForOwnedMutation(): void {
    _afterEdit.cancelDeferredPaginationForOwnedMutation.call(this);
  }

  /** raw IME/iOS 텍스트 입력처럼 command를 거치지 않는 경로의 갱신 라우터. */
  private afterTextInputEdit(
    beforePos: DocumentPosition,
    afterPos: DocumentPosition,
    pageLocalOptions: PageLocalTextEditOptions = {},
    boundaryHandled = false,
  ): void {
    _afterEdit.afterTextInputEdit.call(this, beforePos, afterPos, pageLocalOptions, boundaryHandled);
  }

  private refreshAfterOperation(
    requested: RefreshPolicy | undefined,
    fallback: RefreshPolicy,
    commandType: string,
    beforePos: DocumentPosition,
    afterPos: DocumentPosition,
    pageLocalOptions: PageLocalTextEditOptions = {},
    boundaryHandled = false,
  ): void {
    _afterEdit.refreshAfterOperation.call(
      this,
      requested,
      fallback,
      commandType,
      beforePos,
      afterPos,
      pageLocalOptions,
      boundaryHandled,
    );
  }

  private shouldUsePageLocalRefresh(
    commandType: string,
    beforePos: DocumentPosition,
    afterPos: DocumentPosition,
    pageLocalOptions: PageLocalTextEditOptions = {},
  ): boolean {
    return _afterEdit.shouldUsePageLocalRefresh.call(this, commandType, beforePos, afterPos, pageLocalOptions);
  }

  /**
   * 캐럿 위치를 갱신한다.
   *
   * @param skipScroll true 시 `scrollCaretIntoView` 호출 skip — cursor 변경 trigger 가 동반되지 않은
   *                   onMouseUp (예: drag-during-scroll 영역, scrollbar release 영역) 의 자동 scroll back
   *                   결함 차단 영역. (Task #779)
   */
  private updateCaret(skipScroll: boolean = false): void {
    const rect = this.cursor.getRect();
    if (rect) {
      const zoom = this.viewportManager.getZoom();
      const caretRect = this.adjustExitedFieldEndCaretRect(rect);

      // IME 조합 중: 블랙박스 캐럿 표시
      if (this.isComposing && this.compositionAnchor && this.compositionLength > 0) {
        try {
          const anchor = this.compositionAnchor;
          let startRect: CursorRect;
          if (this.cursor.isInHeaderFooter()) {
            const isHeader = this.cursor.headerFooterMode === 'header';
            startRect = this.wasm.getCursorRectInHeaderFooter(
              this.cursor.hfSectionIdx, isHeader, this.cursor.hfApplyTo,
              this.cursor.hfParaIdx, anchor.charOffset, this.cursor.getRect()?.pageIndex ?? 0,
            )!;
          } else if (this.cursor.isInFootnote()) {
            startRect = this.wasm.getCursorRectInFootnote(
              this.cursor.fnPageNum, this.cursor.fnFootnoteIndex,
              this.cursor.fnInnerParaIdx, anchor.charOffset,
            )!;
          } else if ((anchor.cellPath?.length ?? 0) > 1 && anchor.parentParaIndex !== undefined) {
            startRect = this.wasm.getCursorRectByPath(
              anchor.sectionIndex, anchor.parentParaIndex,
              JSON.stringify(anchor.cellPath), anchor.charOffset,
            );
          } else if (anchor.parentParaIndex !== undefined) {
            startRect = this.wasm.getCursorRectInCell(
              anchor.sectionIndex, anchor.parentParaIndex,
              anchor.controlIndex!, anchor.cellIndex!,
              anchor.cellParaIndex!, anchor.charOffset,
            );
          } else {
            startRect = this.wasm.getCursorRect(
              anchor.sectionIndex, anchor.paragraphIndex, anchor.charOffset,
            );
          }
          const charWidth = rect.x - startRect.x;
          const text = this.textarea.value || '';
          // 현재 커서 위치의 글꼴 정보
          let fontFamily = 'sans-serif';
          try {
            const props = this.getCharPropertiesAtCursor();
            if (props.fontFamily) fontFamily = props.fontFamily;
          } catch { /* fallback */ }
          this.caret.showComposition(startRect, charWidth, zoom, text, fontFamily);
        } catch {
          // getCursorRect 실패 시 일반 캐럿
          this.caret.hideComposition();
          this.caret.update(rect, zoom);
        }
      } else {
        this.caret.hideComposition();
        this.caret.update(caretRect, zoom);
      }
      if (!skipScroll) {
        this.scrollCaretIntoView(caretRect);
      }
    }
    this.updateSelection();
    this.emitCursorFormatState();
    // [Task #394] 셀 진입 자동 ON 로직 비활성화 — 한컴 출력 정합성을 위해 OFF 기본값 유지.
    // 되돌리려면 아래 호출 + line ~1520 의 동일 호출 + 메서드 본체 / 상태 변수 / 이벤트 핸들러
    // 의 주석을 동시에 풀면 이전 동작 복원.
    // this.checkTransparentBordersTransition();
    this.updateFieldMarkers();
    // 눈금자 다단 영역 표시용 커서 좌표 전달
    const cursorRect = this.cursor.getRect();
    if (cursorRect) {
      const adjustedCursorRect = this.adjustExitedFieldEndCaretRect(cursorRect);
      this.eventBus.emit('cursor-rect-updated', { x: adjustedCursorRect.x, y: adjustedCursorRect.y });
    }
  }

  /** 빈 누름틀 끝 바깥 상태에서는 caret을 안내문 오른쪽에 둔다. */
  private adjustExitedFieldEndCaretRect(rect: CursorRect): CursorRect {
    const pos = this.cursor.getPosition();
    try {
      const fi = this.wasm.getFieldInfoAt(pos);
      if (!fi.inField || fi.fieldType !== 'clickhere' || !fi.isGuide || !fi.guideName) {
        return rect;
      }
      if (!this.isAtExitedFieldEnd(pos, fi)) return rect;

      const guideRect = this.findGuideTextRect(rect, fi.guideName);
      if (guideRect) {
        return { ...rect, x: guideRect.x + guideRect.width };
      }

      const measured = this.measureGuideTextWidth(fi.guideName, rect);
      return measured > 0 ? { ...rect, x: rect.x + measured } : rect;
    } catch {
      return rect;
    }
  }

  private findGuideTextRect(
    caretRect: CursorRect,
    guideName: string,
  ): { x: number; y: number; width: number; height: number } | null {
    let best: { x: number; y: number; width: number; height: number; score: number } | null = null;
    try {
      const tree = this.wasm.getPageLayerTreeObject(caretRect.pageIndex);
      const visit = (node: LayerNode | undefined): void => {
        if (!node) return;
        if (node.kind === 'group') {
          for (const child of node.children) visit(child);
          return;
        }
        if (node.kind === 'clipRect') {
          visit(node.child);
          return;
        }
        for (const op of node.ops) {
          if (op.type !== 'textRun') continue;
          const textOp = op as LayerTextRunOp;
          if (textOp.text !== guideName) continue;
          const b = textOp.bbox;
          const score = Math.abs(b.y - caretRect.y) + Math.abs(b.x - caretRect.x) * 0.25;
          if (!best || score < best.score) {
            best = { x: b.x, y: b.y, width: b.width, height: b.height, score };
          }
        }
      };
      visit(tree.root);
    } catch {
      return null;
    }
    const found = best as { x: number; y: number; width: number; height: number; score: number } | null;
    return found ? { x: found.x, y: found.y, width: found.width, height: found.height } : null;
  }

  private measureGuideTextWidth(guideName: string, rect: CursorRect): number {
    const measure = (globalThis as { measureTextWidth?: (font: string, text: string) => number }).measureTextWidth;
    if (typeof measure !== 'function') return 0;
    try {
      const props = this.getCharPropertiesAtCursor();
      const fontFamily = props.fontFamily || 'sans-serif';
      const font = `italic ${Math.max(1, rect.height)}px ${fontFamily}`;
      return measure(font, guideName);
    } catch {
      return 0;
    }
  }

  /** 캐럿 위치를 갱신하되 스크롤하지 않는다 (머리말/꼬리말 닫기 등) */
  private updateCaretNoScroll(): void {
    const rect = this.cursor.getRect();
    if (rect) {
      this.caret.update(rect, this.viewportManager.getZoom());
    }
    this.updateSelection();
    this.emitCursorFormatState();
    // [Task #394] 셀 진입 자동 ON 로직 비활성화 — 위 updateCaretAndScroll 의 코멘트 참고.
    // this.checkTransparentBordersTransition();
  }

  /** 드래그 중 캐럿/선택만 가볍게 갱신한다 */
  private updateCaretDuringDrag(): void {
    if (this.isComposing) {
      this.updateCaret();
      return;
    }

    const rect = this.cursor.getRect();
    if (rect) {
      const zoom = this.viewportManager.getZoom();
      this.caret.hideComposition();
      this.caret.updateLive(rect, zoom);
      // [Task #661] 드래그 중 스크롤은 caret rect 가 아니라 포인터 edge 기준 경로에서만 처리한다.
      // 메인테이너 통합 정정: devel 의 updateLive (PR #664 깜박임 타이머 유지 본질) 보존 +
      // PR #718 의 scrollCaretIntoView 부재 본질 적용.
    }
    this.updateSelection();

    const cursorRect = this.cursor.getRect();
    if (cursorRect) {
      this.eventBus.emit('cursor-rect-updated', { x: cursorRect.x, y: cursorRect.y });
    }
  }

  /** 클릭 좌표에서 같은 표 내 셀의 row/col을 반환한다. 다른 표이거나 셀이 아니면 null. */
  private hitTestCellRowCol(e: MouseEvent): { row: number; col: number } | null {
    const ctx = this.cursor.getCellTableContext();
    if (!ctx) return null;
    const zoom = this.viewportManager.getZoom();
    const scrollContent = this.container.querySelector('#scroll-content')!;
    const contentRect = scrollContent.getBoundingClientRect();
    const contentX = e.clientX - contentRect.left;
    const contentY = e.clientY - contentRect.top;
    const pageIdx = this.virtualScroll.getPageAtPoint(contentX, contentY);
    const pageOffset = this.virtualScroll.getPageOffset(pageIdx);
    const pageDisplayWidth = this.virtualScroll.getPageWidth(pageIdx);
    const pageLeft = this.virtualScroll.getPageLeftResolved(pageIdx, scrollContent.clientWidth);
    const pageX = (contentX - pageLeft) / zoom;
    const pageY = (contentY - pageOffset) / zoom;
    try {
      const hit = this.wasm.hitTest(pageIdx, pageX, pageY);
      // 같은 표인지 확인
      if (hit.parentParaIndex !== ctx.ppi || hit.controlIndex !== ctx.ci) return null;
      if (hit.cellIndex === undefined) return null;
      if (ctx.cellPath && ctx.cellPath.length > 1 && hit.cellPath) {
        // 중첩 표: 경로 기반으로 셀 정보 조회
        const pathJson = JSON.stringify(hit.cellPath);
        const info = this.wasm.getCellInfoByPath(ctx.sec, ctx.ppi, pathJson);
        return { row: info.row, col: info.col };
      }
      const info = this.wasm.getCellInfo(ctx.sec, ctx.ppi, ctx.ci, hit.cellIndex);
      return { row: info.row, col: info.col };
    } catch {
      return null;
    }
  }

  /** F5 셀 선택 하이라이트를 갱신한다 */
  private updateCellSelection(): void {
    if (!this.cellSelectionRenderer) return;
    const range = this.cursor.getSelectedCellRange();
    const ctx = this.cursor.getCellTableContext();
    if (!range || !ctx) {
      this.cellSelectionRenderer.clear();
      return;
    }
    try {
      let bboxes;
      if (ctx.cellPath && ctx.cellPath.length > 1) {
        // 중첩 표: 경로 기반 API 사용
        const pathJson = JSON.stringify(ctx.cellPath);
        bboxes = this.wasm.getTableCellBboxesByPath(ctx.sec, ctx.ppi, pathJson);
      } else {
        bboxes = this.wasm.getTableCellBboxes(ctx.sec, ctx.ppi, ctx.ci);
      }
      const zoom = this.viewportManager.getZoom();
      const excluded = this.cursor.getExcludedCells();
      this.cellSelectionRenderer.render(bboxes, range, zoom, excluded.size > 0 ? excluded : undefined);
    } catch (e) {
      console.warn('[InputHandler] updateCellSelection 실패:', e);
      this.cellSelectionRenderer.clear();
    }
  }

  /** 선택 영역 하이라이트를 갱신한다 */
  private updateSelection(): void {
    const fnSel = this.cursor.getFootnoteSelectionOrdered();
    if (fnSel) {
      const { start, end, pageNum, footnoteIndex } = fnSel;
      const zoom = this.viewportManager.getZoom();
      try {
        const rects = this.wasm.getSelectionRectsInFootnote(
          pageNum,
          footnoteIndex,
          start.fnParaIdx,
          start.charOffset,
          end.fnParaIdx,
          end.charOffset,
        );
        this.selectionRenderer.render(rects, zoom);
      } catch (e) {
        console.warn('[InputHandler] getSelectionRectsInFootnote 실패:', e);
        this.selectionRenderer.clear();
      }
      return;
    }

    const sel = this.cursor.getSelectionOrdered();
    if (!sel) {
      this.selectionRenderer.clear();
      return;
    }

    const { start, end } = sel;
    const zoom = this.viewportManager.getZoom();

    try {
      let rects;
      const startInCell = start.parentParaIndex !== undefined;
      const endInCell = end.parentParaIndex !== undefined;

      if (startInCell && endInCell && isSameSelectionCellContainer(start, end)) {
        // 같은 셀 내부 선택
        const pageHints = start.cursorRect && end.cursorRect
          ? {
            startPageHint: start.cursorRect.pageIndex,
            endPageHint: end.cursorRect.pageIndex,
          }
          : undefined;
        const cellPath = cellAxisPath(start);
        if (cellPath.length > 1) {
          rects = this.wasm.getSelectionRectsInCellByPath(
            start.sectionIndex,
            start.parentParaIndex!,
            JSON.stringify(cellPath),
            cellParaIndexOf(start),
            start.charOffset,
            cellParaIndexOf(end),
            end.charOffset,
            pageHints,
          );
        } else {
          rects = this.wasm.getSelectionRectsInCell(
            start.sectionIndex, start.parentParaIndex!, start.controlIndex!, start.cellIndex!,
            start.cellParaIndex!, start.charOffset,
            end.cellParaIndex!, end.charOffset,
            pageHints,
          );
        }
      } else if (!startInCell && !endInCell) {
        // 본문 선택
        rects = this.wasm.getSelectionRects(
          start.sectionIndex,
          start.paragraphIndex, start.charOffset,
          end.paragraphIndex, end.charOffset,
        );
      } else {
        // 셀↔본문 또는 셀↔다른 셀 혼합 선택: 렌더링 생략
        this.selectionRenderer.clear();
        return;
      }
      this.selectionRenderer.render(rects, zoom);
    } catch (e) {
      console.warn('[InputHandler] getSelectionRects 실패:', e);
      this.selectionRenderer.clear();
    }
  }

  /** 표 객체 선택 시 외곽선 + 핸들을 렌더링한다 */
  private renderTableObjectSelection(): void {
    if (!this.tableObjectRenderer) return;
    const ref = this.cursor.getSelectedTableRef();
    if (!ref) {
      this.tableObjectRenderer.clear();
      return;
    }
    try {
      const zoom = this.viewportManager.getZoom();
      const pageHint = this.cursor.getRect()?.pageIndex;
      // 셀 bbox를 페이지별로 그룹화하여 합집합 계산 (다중 페이지 표 지원)
      let cellBboxes: { cellIdx: number; row: number; col: number; rowSpan: number; colSpan: number; pageIndex: number; x: number; y: number; w: number; h: number }[];
      if (ref.cellPath && ref.cellPath.length > 1) {
        // 중첩 표: 경로 기반 API
        const pathJson = JSON.stringify(ref.cellPath);
        cellBboxes = this.wasm.getTableCellBboxesByPath(ref.sec, ref.ppi, pathJson);
      } else {
        // 외부 표: flat API
        cellBboxes = this.wasm.getTableCellBboxes(ref.sec, ref.ppi, ref.ci, pageHint);
      }
      if (cellBboxes.length === 0) {
        this.tableObjectRenderer.clear();
        return;
      }
      // 페이지별 그룹화
      const byPage = new Map<number, typeof cellBboxes>();
      for (const b of cellBboxes) {
        let arr = byPage.get(b.pageIndex);
        if (!arr) { arr = []; byPage.set(b.pageIndex, arr); }
        arr.push(b);
      }
      const pageBboxes: { pageIndex: number; x: number; y: number; width: number; height: number }[] = [];
      for (const [pageIndex, cells] of byPage) {
        let minX = Infinity, minY = Infinity, maxX = -Infinity, maxY = -Infinity;
        for (const c of cells) {
          minX = Math.min(minX, c.x);
          minY = Math.min(minY, c.y);
          maxX = Math.max(maxX, c.x + c.w);
          maxY = Math.max(maxY, c.y + c.h);
        }
        pageBboxes.push({ pageIndex, x: minX, y: minY, width: maxX - minX, height: maxY - minY });
      }
      this.tableObjectRenderer.renderMultiPage(pageBboxes, zoom);
    } catch (e) {
      console.warn('[InputHandler] renderTableObjectSelection 실패:', e);
      this.tableObjectRenderer.clear();
    }
  }

  /** 그림/글상자 클릭 감지 — getPageControlLayout으로 개체 bbox 겹침 확인 */
  private findPictureAtClick(
    pageIdx: number, pageX: number, pageY: number,
  ): { sec: number; ppi: number; ci: number; type: 'image' | 'shape' | 'equation' | 'group' | 'line' | 'ole'; cellIdx?: number; cellParaIdx?: number; noteRef?: any; x1?: number; y1?: number; x2?: number; y2?: number } | null {
    return _picture.findPictureAtClick.call(this, pageIdx, pageX, pageY);
  }

  /** 선택된 그림/글상자의 bbox를 페이지 레이아웃에서 찾는다 */
  private findPictureBbox(
    ref: { sec: number; ppi: number; ci: number; type?: 'image' | 'shape' | 'equation' | 'group' | 'line' | 'ole' },
  ): { pageIndex: number; x: number; y: number; w: number; h: number } | null {
    return _picture.findPictureBbox.call(this, ref);
  }

  /** 개체 속성을 타입에 따라 조회한다 (그림/글상자 분기) */
  private getObjectProperties(ref: { sec: number; ppi: number; ci: number; type: 'image' | 'shape' | 'equation' | 'group' | 'line' | 'ole' }): any {
    return _picture.getObjectProperties.call(this, ref);
  }

  /** 개체 속성을 타입에 따라 변경한다 (그림/글상자 분기) */
  private setObjectProperties(ref: { sec: number; ppi: number; ci: number; type: 'image' | 'shape' | 'equation' | 'group' | 'line' | 'ole' }, props: Record<string, unknown>): void {
    _picture.setObjectProperties.call(this, ref, props);
  }

  /** 개체를 타입에 따라 삭제한다 (그림/글상자 분기) */
  private deleteObjectControl(ref: { sec: number; ppi: number; ci: number; type: 'image' | 'shape' | 'equation' | 'group' | 'line' | 'ole' }): void {
    _picture.deleteObjectControl.call(this, ref);
  }

  /** [Task #2230] 그림 미지정 placeholder 에 그림 지정 (파일 선택 → assignPictureImage) */
  private promptAssignPictureImage(ref: { sec: number; ppi: number; ci: number; type: 'image' | 'shape' | 'equation' | 'group' | 'line' | 'ole'; cellPath?: any }): void {
    _picture.promptAssignPictureImage.call(this, ref);
  }

  /** 그림 객체 선택 시 외곽선 + 핸들을 렌더링한다 */
  private renderPictureObjectSelection(): void {
    _picture.renderPictureObjectSelection.call(this);
  }

  /** 그림 객체 선택을 해제한다 (있으면) */
  private exitPictureObjectSelectionIfNeeded(): void {
    _picture.exitPictureObjectSelectionIfNeeded.call(this);
  }

  /** 클릭 좌표가 글상자의 경계선 위인지 판정한다 */
  private isShapeBorderClick(
    pageX: number, pageY: number,
    shape: { sec: number; ppi: number; ci: number },
  ): boolean {
    return _picture.isShapeBorderClick.call(this, pageX, pageY, shape);
  }

  // ─── 그림 핸들 드래그 리사이즈 ─────────────────────────


  /** 드래그 중 실시간 피드백: 핸들 위치를 새 bbox에 맞춰 재렌더 */
  private updatePictureResizeDrag(e: MouseEvent): void {
    _picture.updatePictureResizeDrag.call(this, e);
  }

  /** 드래그 완료: 새 크기를 WASM에 반영 */
  private finishPictureResizeDrag(e: MouseEvent): void {
    _picture.finishPictureResizeDrag.call(this, e);
  }

  /** 드래그 delta로 새 bbox 계산 (page coords) */
  private calcResizedBbox(e: MouseEvent, zoom: number): { x: number; y: number; width: number; height: number } {
    return _picture.calcResizedBbox.call(this, e, zoom);
  }

  private cleanupPictureResizeDrag(): void {
    _picture.cleanupPictureResizeDrag.call(this);
  }

  // ─── 그림 이동 드래그 ──────────────────────────────

  /** 마우스 드래그로 그림 이동 — 드래그 중 갱신 */
  private updatePictureMoveDrag(e: MouseEvent): void {
    _picture.updatePictureMoveDrag.call(this, e);
  }

  /** 마우스 드래그로 그림 이동 — 드래그 종료 */
  private finishPictureMoveDrag(): void {
    _picture.finishPictureMoveDrag.call(this);
  }

  /** 마우스 드래그로 그림 회전 — 드래그 업데이트 */
  private updatePictureRotateDrag(e: MouseEvent): void {
    _picture.updatePictureRotateDrag.call(this, e);
  }

  /** 마우스 드래그로 그림 회전 — 드래그 종료 */
  private finishPictureRotateDrag(e: MouseEvent): void {
    _picture.finishPictureRotateDrag.call(this, e);
  }

  /* [Task #394] 셀 진입 자동 ON 로직 비활성화 — 호출 지점 (updateCaretAndScroll, updateCaretNoScroll)
     의 호출도 같이 주석 처리됨. 되돌리려면 본 블록 주석 + 호출 지점 주석 + 상태 변수 / 이벤트 핸들러
     주석을 동시에 풀면 이전 동작 복원.

  // 셀 진입/탈출 시 투명선 자동 ON/OFF
  private checkTransparentBordersTransition(): void {
    const nowInCell = this.cursor.isInCell() && !this.cursor.isInTextBox();
    if (nowInCell && !this.wasInCell) {
      // 셀 밖 → 셀 진입: 자동 ON
      if (!this.manualTransparentBorders) {
        this.autoTransparentBorders = true;
        this.wasm.setShowTransparentBorders(true);
        document.querySelectorAll('[data-cmd="view:border-transparent"]').forEach(el => {
          el.classList.add('active');
        });
        this.eventBus.emit('document-changed');
      }
    } else if (!nowInCell && this.wasInCell) {
      // 셀 안 → 셀 탈출: 자동으로 켜진 경우에만 OFF
      if (this.autoTransparentBorders && !this.manualTransparentBorders) {
        this.autoTransparentBorders = false;
        this.wasm.setShowTransparentBorders(false);
        document.querySelectorAll('[data-cmd="view:border-transparent"]').forEach(el => {
          el.classList.remove('active');
        });
        this.eventBus.emit('document-changed');
      }
    }
    this.wasInCell = nowInCell;
  }
  */

  /** 캐럿이 화면 밖이면 스크롤을 조정한다 */
  private scrollCaretIntoView(rect: import('@/core/types').CursorRect): void {
    const zoom = this.viewportManager.getZoom();
    const pageOffset = this.virtualScroll.getPageOffset(rect.pageIndex);
    const caretDocY = pageOffset + rect.y * zoom;
    const caretHeight = rect.height * zoom;

    const scrollTop = this.container.scrollTop;
    const viewHeight = this.container.clientHeight;
    const margin = 20; // 여백 px

    if (caretDocY < scrollTop + margin) {
      // 캐럿이 화면 위쪽 밖
      this.container.scrollTop = Math.max(0, caretDocY - margin);
    } else if (caretDocY + caretHeight > scrollTop + viewHeight - margin) {
      // 캐럿이 화면 아래쪽 밖
      this.container.scrollTop = caretDocY + caretHeight - viewHeight + margin;
    }
  }

  /** 문서 로딩 후 저장된 캐럿 위치에 캐럿을 배치한다 */
  activateWithCaretPosition(): void {
    try {
      const savedPos = this.wasm.getCaretPosition();
      if (savedPos) {
        this.cursor.moveTo(savedPos);
      } else {
        this.cursor.moveTo({ sectionIndex: 0, paragraphIndex: 0, charOffset: 0 });
      }
      this.cursor.resetPreferredX();
      this.active = true;

      const rect = this.cursor.getRect();
      if (rect) {
        this.caret.show(rect, this.viewportManager.getZoom());
      }
      this.emitCursorFormatState();
      this.focusTextarea();
    } catch (e) {
      console.warn('[InputHandler] 캐럿 자동 배치 실패:', e);
      // 실패 시 문서 시작에 배치
      this.cursor.moveTo({ sectionIndex: 0, paragraphIndex: 0, charOffset: 0 });
      this.active = true;
      const rect = this.cursor.getRect();
      if (rect) {
        this.caret.show(rect, this.viewportManager.getZoom());
      }
      this.focusTextarea();
    }
  }

  /** 캐럿을 숨기고 히스토리를 초기화한다 */
  /** textarea에 포커스를 복원한다 (대화상자 닫힌 후 등) */
  focus(): void {
    this.focusTextarea();
  }

  deactivate(): void {
    this.flushDeferredPaginationIfNeeded('before-deactivate', false);
    this.active = false;
    this.cancelDeferredPaginationFlush();
    this.deferredPaginationRunner.cancel();
    this.deferredPaginationPending = false;
    this.resetRawTextMutationEffects();
    this.isComposing = false;
    this.compositionAnchor = null;
    this.compositionLength = 0;
    // [#4162] 문서 전환·닫기에서 안 지우면, 이전 문서에서 예약한 서식이 새 문서의
    // 흔한 시작 캐럿 위치(예: {sec:0,para:0,offset:0})와 우연히 일치할 때 새 문서
    // 첫 글자로 새어 들어간다 — 실행 확인: deactivate() 호출 전후 필드가 안 바뀜.
    this.pendingCharShape = null;
    this.pendingCharShapeAnchor = null;
    this._lastCompositionText = '';
    this._lastComposedText = '';
    this._pendingNavAfterIME = null;
    if (this._iosInputTimer) {
      clearTimeout(this._iosInputTimer);
      this._iosInputTimer = null;
    }
    this._iosAnchor = null;
    this._iosBeforePageIndex = undefined;
    this._iosComposing = false;
    this._iosLength = 0;
    this._iosPrevText = '';
    this._iosRequiresFullRefresh = false;
    this.textarea.value = '';
    this.caret.hide();
    this.fieldMarker.hide();
    this.cursor.clearSelection();
    this.selectionRenderer.clear();
    this.history.clear(this.wasm);
  }

  dispose(): void {
    this.flushDeferredPaginationIfNeeded('before-dispose', false);
    if (this.isResizeDragging) {
      this.cleanupResizeDrag();
    }
    if (this.dragRafId) {
      cancelAnimationFrame(this.dragRafId);
      this.dragRafId = 0;
    }
    this.cellSelectionDragState = null;
    this.cellSelectionDragCandidate = null;
    this.stopTextSelectionDragAutoScroll();
    if (this.resizeHoverRafId) {
      cancelAnimationFrame(this.resizeHoverRafId);
      this.resizeHoverRafId = 0;
    }
    this.cancelDeferredPaginationFlush();
    this.deferredPaginationRunner.cancel();
    this.deferredPaginationPending = false;
    this.resetRawTextMutationEffects();
    this.isComposing = false;
    this.compositionAnchor = null;
    this.compositionLength = 0;
    // [#4162] 문서 전환·닫기에서 안 지우면, 이전 문서에서 예약한 서식이 새 문서의
    // 흔한 시작 캐럿 위치(예: {sec:0,para:0,offset:0})와 우연히 일치할 때 새 문서
    // 첫 글자로 새어 들어간다 — 실행 확인: deactivate() 호출 전후 필드가 안 바뀜.
    this.pendingCharShape = null;
    this.pendingCharShapeAnchor = null;
    this._lastCompositionText = '';
    this._lastComposedText = '';
    this._pendingNavAfterIME = null;
    if (this._iosInputTimer) {
      clearTimeout(this._iosInputTimer);
      this._iosInputTimer = null;
    }
    this._iosAnchor = null;
    this._iosBeforePageIndex = undefined;
    this._iosComposing = false;
    this._iosLength = 0;
    this._iosPrevText = '';
    this._iosRequiresFullRefresh = false;
    document.removeEventListener('keydown', this.onF11InterceptBound, true);
    this.container.removeEventListener('mousedown', this.onClickBound);
    this.container.removeEventListener('dblclick', this.onDblClickBound);
    this.container.removeEventListener('contextmenu', this.onContextMenuBound);
    this.container.removeEventListener('mousemove', this.onMouseMoveBound);
    document.removeEventListener('mousemove', this.onMouseMoveBound);
    document.removeEventListener('mouseup', this.onMouseUpBound);
    this.textarea.removeEventListener('keydown', this.onKeyDownBound);
    this.textarea.removeEventListener('input', this.onInputBound);
    this.textarea.removeEventListener('compositionstart', this.onCompositionStartBound);
    this.textarea.removeEventListener('compositionend', this.onCompositionEndBound);
    this.textarea.removeEventListener('blur', this.onInputBlurBound);
    this.textarea.removeEventListener('copy', this.onCopyBound);
    this.textarea.removeEventListener('cut', this.onCutBound);
    this.textarea.removeEventListener('paste', this.onPasteBound);
    this.textarea.remove();
    this.caret.dispose();
    this.fieldMarker.dispose();
    this.selectionRenderer.dispose();
    this.cellSelectionRenderer?.dispose();
    this.tableObjectRenderer?.dispose();
    this.tableResizeRenderer?.dispose();
    this.protectedCellHoverEl?.remove();
    this.contextMenu?.dispose();
  }

  // ─── 커맨드 시스템용 public 접근자 ─────────────────────────

  /** 커맨드 디스패처를 주입한다 (main.ts에서 호출) */
  setDispatcher(d: CommandDispatcher): void { this.dispatcher = d; }

  /** 현재 편집 모드를 설정한다 */
  setEditMode(mode: EditorEditMode): void {
    this.editMode = mode;
    if (mode === 'form') {
      if (this.cursor.isInPictureObjectSelection()) {
        this.cursor.moveOutOfSelectedPicture();
        this.pictureObjectRenderer?.clear();
        this.eventBus.emit('picture-object-selection-changed', false);
      }
      if (this.cursor.isInTableObjectSelection()) {
        this.cursor.moveOutOfSelectedTable();
        this.tableObjectRenderer?.clear();
        this.eventBus.emit('table-object-selection-changed', false);
      }
    }
    this.eventBus.emit('command-state-changed');
  }

  /** 양식 모드인가? */
  isFormMode(): boolean { return this.editMode === 'form'; }

  /** 현재 커서가 양식 모드에서 편집 가능한 누름틀 안인가? */
  canEditCurrentFormField(): boolean {
    return this.isEditableFormFieldPosition(this.cursor.getPosition());
  }

  private isSameTextContainer(a: DocumentPosition, b: DocumentPosition): boolean {
    if (a.sectionIndex !== b.sectionIndex) return false;
    if (a.paragraphIndex !== b.paragraphIndex) return false;
    if (a.parentParaIndex !== b.parentParaIndex) return false;
    if (a.controlIndex !== b.controlIndex) return false;
    if (a.cellIndex !== b.cellIndex) return false;
    if (a.cellParaIndex !== b.cellParaIndex) return false;
    if ((a.isTextBox ?? false) !== (b.isTextBox ?? false)) return false;
    return JSON.stringify(a.cellPath ?? []) === JSON.stringify(b.cellPath ?? []);
  }

  private getFormFieldInfoAt(pos: DocumentPosition): any | null {
    if (this.cursor.isInHeaderFooter() || this.cursor.isInFootnote()) return null;
    try {
      const fi = this.wasm.getFieldInfoAt(pos);
      if (!fi?.inField) return null;
      if (fi.fieldType !== 'clickhere') return null;
      return fi;
    } catch {
      return null;
    }
  }

  private isEditableFormFieldPosition(pos: DocumentPosition): boolean {
    const fi = this.getFormFieldInfoAt(pos);
    if (!fi?.editableInForm) return false;
    const start = fi.startCharIdx ?? -1;
    const end = fi.endCharIdx ?? -1;
    return pos.charOffset >= start && pos.charOffset <= end;
  }

  canInsertTextInFormMode(pos: DocumentPosition): boolean {
    if (this.editMode !== 'form') return true;
    return this.isEditableFormFieldPosition(pos);
  }

  canDeleteTextInFormMode(pos: DocumentPosition, count: number): boolean {
    if (this.editMode !== 'form') return true;
    const fi = this.getFormFieldInfoAt(pos);
    if (!fi?.editableInForm) return false;
    const start = fi.startCharIdx ?? -1;
    const end = fi.endCharIdx ?? -1;
    return pos.charOffset >= start && pos.charOffset + count <= end;
  }

  canDeleteSelectionInFormMode(): boolean {
    if (this.editMode !== 'form') return true;
    const sel = this.cursor.getSelectionOrdered();
    if (!sel) return this.canEditCurrentFormField();
    if (!this.isSameTextContainer(sel.start, sel.end)) return false;
    const fi = this.getFormFieldInfoAt(sel.start);
    if (!fi?.editableInForm) return false;
    if (fi.fieldId === undefined) return false;
    const endInfo = this.getFormFieldInfoAt(sel.end);
    if (!endInfo?.editableInForm || endInfo.fieldId !== fi.fieldId) return false;
    const start = fi.startCharIdx ?? -1;
    const end = fi.endCharIdx ?? -1;
    return sel.start.charOffset >= start && sel.end.charOffset <= end;
  }

  moveToAdjacentFormField(delta: number): boolean {
    if (this.editMode !== 'form') return false;
    const currentInfo = this.getFormFieldInfoAt(this.cursor.getPosition());
    const currentFieldId = currentInfo?.fieldId;
    const currentKey = this.formFieldSortKey(this.cursor.getPosition());
    const fields = this.wasm.getFieldList()
      .filter((field: any) =>
        field.fieldType === 'clickhere'
        && field.editableInForm === true
        && typeof field.startCharIdx === 'number')
      .map((field: any) => {
        const pos = this.formFieldPosition(field);
        return pos ? { field, pos, key: this.formFieldSortKey(pos) } : null;
      })
      .filter(Boolean)
      .sort((a: any, b: any) => this.compareFormFieldKeys(a.key, b.key));

    if (fields.length === 0) return false;

    const forward = delta >= 0;
    const withoutCurrent = fields.filter((entry: any) => entry.field.fieldId !== currentFieldId);
    const candidates = withoutCurrent.length > 0 ? withoutCurrent : fields;
    const target = forward
      ? candidates.find((entry: any) => this.compareFormFieldKeys(entry.key, currentKey) > 0) ?? candidates[0]
      : [...candidates].reverse().find((entry: any) => this.compareFormFieldKeys(entry.key, currentKey) < 0) ?? candidates[candidates.length - 1];

    if (!target) return false;
    this.cursor.clearSelection();
    this.cursor.moveTo(target.pos);
    this.cursor.resetPreferredX();
    this.active = true;
    this.updateCaret();
    this.updateFieldMarkers();
    this.focusTextarea();
    this.eventBus.emit('command-state-changed');
    return true;
  }

  private formFieldPosition(field: any): DocumentPosition | null {
    const loc = field.location;
    if (!loc || typeof loc.sectionIndex !== 'number' || typeof loc.paraIndex !== 'number') {
      return null;
    }
    const charOffset = typeof field.startCharIdx === 'number' ? field.startCharIdx : 0;
    const path = Array.isArray(loc.path) ? loc.path : [];
    if (path.length === 0) {
      return { sectionIndex: loc.sectionIndex, paragraphIndex: loc.paraIndex, charOffset };
    }

    const cellPath = path.map((entry: any) => ({
      controlIndex: entry.controlIndex ?? 0,
      cellIndex: entry.type === 'textbox' ? 0 : (entry.cellIndex ?? 0),
      cellParaIndex: entry.paraIndex ?? 0,
    }));
    const last = cellPath[cellPath.length - 1];
    const lastRaw = path[path.length - 1] ?? {};
    return {
      sectionIndex: loc.sectionIndex,
      paragraphIndex: last.cellParaIndex,
      charOffset,
      parentParaIndex: loc.paraIndex,
      controlIndex: cellPath[0].controlIndex,
      cellIndex: last.cellIndex,
      cellParaIndex: last.cellParaIndex,
      cellPath,
      isTextBox: lastRaw.type === 'textbox',
    };
  }

  private formFieldSortKey(pos: DocumentPosition): number[] {
    const pathKey = (pos.cellPath ?? [])
      .flatMap((entry: any) => [
        entry.controlIndex ?? entry.controlIdx ?? 0,
        entry.cellIndex ?? entry.cellIdx ?? 0,
        entry.cellParaIndex ?? entry.cellParaIdx ?? 0,
      ]);
    return [
      pos.sectionIndex,
      pos.parentParaIndex ?? pos.paragraphIndex,
      ...pathKey,
      pos.paragraphIndex,
      pos.charOffset,
    ];
  }

  private compareFormFieldKeys(a: number[], b: number[]): number {
    const len = Math.max(a.length, b.length);
    for (let i = 0; i < len; i++) {
      const av = a[i] ?? -1;
      const bv = b[i] ?? -1;
      if (av !== bv) return av - bv;
    }
    return 0;
  }

  private isOperationAllowedInEditMode(desc: OperationDescriptor): boolean {
    if (this.editMode !== 'form') return true;
    // [Task #2337-review] kind:'record' 는 이미 적용된 뮤테이션을 히스토리에 기록만 한다.
    // form mode 에서 이를 드롭하면 그 뮤테이션이 undo 불가한 미기록 편집으로 남아(더블클릭
    // 진입한 HF/FN 입력·Enter 분할 등) 이 커밋이 막으려는 무언 손실 경로가 그대로 유지된다.
    // 뮤테이션 적용 여부는 호출부의 form-mode 게이트(IME 조합·본문 입력 경로)가 이미 결정하므로,
    // 이미 적용된 편집은 항상 기록한다.
    if (desc.kind === 'record') return true;
    if (desc.kind === 'snapshot') return false;

    const command = desc.command as any;
    switch (command.type) {
      case 'insertText':
        return this.canInsertTextInFormMode(command.position ?? this.cursor.getPosition());
      case 'deleteText':
        return this.canDeleteTextInFormMode(command.position ?? this.cursor.getPosition(), command.count ?? 1);
      case 'deleteSelection':
        return this.canDeleteSelectionInFormMode();
      default:
        return false;
    }
  }

  /** 편집 영역이 활성 상태인지 (문서 로드 + 편집 영역 포커스) */
  isActive(): boolean { return this.active; }

  /** 컨텍스트 메뉴를 주입한다 (main.ts에서 호출) */
  setContextMenu(cm: ContextMenu): void { this.contextMenu = cm; }

  /** 커맨드 팔레트를 주입한다 (main.ts에서 호출) */
  setCommandPalette(cp: CommandPalette): void { this.commandPalette = cp; }

  /** 셀 선택 렌더러를 주입한다 (main.ts에서 호출) */
  setCellSelectionRenderer(r: CellSelectionRenderer): void { this.cellSelectionRenderer = r; }

  /** 표 객체 선택 렌더러를 주입한다 (main.ts에서 호출) */
  setTableObjectRenderer(r: TableObjectRenderer): void { this.tableObjectRenderer = r; }

  /** 그림 객체 선택 렌더러를 주입한다 (main.ts에서 호출) */
  setPictureObjectRenderer(r: TableObjectRenderer): void { this.pictureObjectRenderer = r; }

  /** 그림 객체 선택 모드인가? */
  isInPictureObjectSelection(): boolean { return this.cursor.isInPictureObjectSelection(); }

  /** 선택된 그림/글상자 참조 반환 ([Task #825] headerFooter 동반 시 머리말/꼬리말 picture marker) */
  getSelectedPictureRef(): { sec: number; ppi: number; ci: number; type: 'image' | 'shape' | 'equation' | 'group' | 'line' | 'ole'; cellIdx?: number; cellParaIdx?: number; outerTableControlIdx?: number; cellPath?: Array<{ controlIndex: number; cellIndex: number; cellParaIndex: number }>; noteRef?: any; headerFooter?: { kind: 'header' | 'footer'; outerParaIdx: number; outerControlIdx: number } } | null { return this.cursor.getSelectedPictureRef(); }

  /** 다중 선택된 개체 목록 */
  getSelectedPictureRefs(): { sec: number; ppi: number; ci: number; type: string }[] { return this.cursor.getSelectedPictureRefs(); }

  /** 다중 선택 상태인가? */
  isMultiPictureSelection(): boolean { return this.cursor.isMultiPictureSelection(); }

  /** 지정 개체를 선택 상태로 진입 */
  selectPictureObject(sec: number, ppi: number, ci: number, type: 'image' | 'shape' | 'equation' | 'group' | 'line' | 'ole'): void {
    this.cursor.enterPictureObjectSelectionDirect(sec, ppi, ci, type);
    this.renderPictureObjectSelection();
    this.eventBus.emit('picture-object-selection-changed', true);
  }

  /** 그림 삭제 후: 선택 해제 + afterEdit */
  /** 커서 위치 반환 */
  getPosition(): { sectionIndex: number; paragraphIndex: number; charOffset: number } {
    return this.cursor.getPosition();
  }

  /** 편집 완료 후 렌더링 갱신 */
  triggerAfterEdit(): void {
    this.afterEdit();
  }

  exitPictureObjectSelectionAndAfterEdit(): void {
    this.exitPictureObjectSelectionIfNeeded();
    this.afterEdit();
  }

  /** 글상자 내부 텍스트 편집 모드 진입 */
  private enterTextboxEditing(sec: number, ppi: number, ci: number): void {
    this.enterInlineEditing(sec, ppi, ci, 0);
  }

  /** 캡션/글상자 내부 텍스트 편집 모드 진입 (charOffset 지정 가능) */
  enterInlineEditing(sec: number, ppi: number, ci: number, charOffset = 0): void {
    this.cursor.clearSelection();
    this.cursor.moveTo({
      sectionIndex: sec,
      paragraphIndex: 0,
      charOffset,
      parentParaIndex: ppi,
      controlIndex: ci,
      cellIndex: 0,
      cellParaIndex: 0,
      isTextBox: true,
    });
    this.cursor.resetPreferredX();
    this.updateCaret();
    this.focusTextarea();
  }

  /** 표 캡션 텍스트 편집 모드 진입 (cellIndex=65534로 캡션 구분) */
  enterTableCaptionEditing(sec: number, ppi: number, ci: number, charOffset = 0): void {
    this.cursor.clearSelection();
    this.cursor.moveTo({
      sectionIndex: sec,
      paragraphIndex: 0,
      charOffset,
      parentParaIndex: ppi,
      controlIndex: ci,
      cellIndex: 65534,
      cellParaIndex: 0,
    });
    this.cursor.resetPreferredX();
    this.updateCaret();
    this.focusTextarea();
  }

  /** 표 경계선 리사이즈 렌더러를 주입한다 (main.ts에서 호출) */
  setTableResizeRenderer(r: TableResizeRenderer): void { this.tableResizeRenderer = r; }

  /** 선택 영역이 있는가? */
  hasSelection(): boolean { return this.getNonEmptySelection() !== null; }

  /** 선택 anchor만 지운다(캐럿 위치는 그대로) — snapshot 삽입 직후 stale 선택이
   * 새로 삽입된 내용까지 걸치는 유령 범위를 만드는 것을 막는다. `executeOperation`의
   * `cursor.moveTo(newPos)`는 anchor를 지우지 않으므로(cursor.ts의 `moveTo`) 자동으로
   * 처리되지 않는다 — 호출부가 명시적으로 불러야 한다. */
  clearSelectionAnchor(): void { this.cursor.clearSelection(); }

  /** 모양 복사 상태가 있는가? */
  hasCopiedFormat(): boolean { return this.formatCopyState !== null; }

  /** 현재 커서 위치를 반환한다 */
  getCursorPosition(): DocumentPosition { return this.cursor.getPosition(); }

  /** 본문 탐색 전에 각주 전용 편집 컨텍스트를 종료한다. */
  exitFootnoteModeForBodyNavigation(): void {
    if (!this.cursor.isInFootnote()) return;
    this.cursor.exitFootnoteMode();
    this.eventBus.emit('footnoteModeChanged', false);
  }

  /** 커서를 지정 위치로 이동하고 캐럿을 표시한다. 성공하면 true 반환. */
  moveCursorTo(pos: DocumentPosition): boolean {
    return _fieldNav.moveCursorTo.call(this, pos);
  }

  /** 현재 커서 위치의 누름틀 필드와 내용을 제거한다. */
  removeCurrentField(posOverride?: DocumentPosition): void {
    _fieldNav.removeCurrentField.call(this, posOverride);
  }

  /** 현재 커서 위치의 누름틀 제거를 한컴처럼 확인 후 수행한다. */
  confirmRemoveCurrentField(): boolean {
    return _fieldNav.confirmRemoveCurrentField.call(this);
  }

  /** 누름틀 끝에서 오른쪽 이동 시 같은 charOffset을 필드 밖 위치로 취급한다. */
  tryExitCurrentFieldEnd(): boolean {
    return _fieldNav.tryExitCurrentFieldEnd.call(this);
  }

  /** 누름틀 시작에서 왼쪽 이동 시 같은 charOffset을 필드 밖 위치로 취급한다. */
  tryExitCurrentFieldStart(): boolean {
    return _fieldNav.tryExitCurrentFieldStart.call(this);
  }

  /** 누름틀 시작 밖 위치에서 오른쪽 이동하면 같은 charOffset의 필드 내부 시작으로 들어간다. */
  tryEnterExitedFieldStart(): boolean {
    return _fieldNav.tryEnterExitedFieldStart.call(this);
  }

  /** 누름틀 끝 밖 위치에서 왼쪽 이동하면 같은 charOffset의 필드 내부 끝으로 들어간다. */
  tryEnterExitedFieldEnd(): boolean {
    return _fieldNav.tryEnterExitedFieldEnd.call(this);
  }

  /** Home 이동 결과가 누름틀 시작이면 한컴처럼 누름틀 이전 위치로 취급한다. */
  markCurrentFieldStartOutside(): boolean {
    return _fieldNav.markCurrentFieldStartOutside.call(this);
  }

  /** End 이동 결과가 누름틀 끝이면 한컴처럼 누름틀 이후 위치로 취급한다. */
  markCurrentFieldEndOutside(): boolean {
    return _fieldNav.markCurrentFieldEndOutside.call(this);
  }

  isAtExitedFieldStart(pos: DocumentPosition, fi?: { fieldId?: number; startCharIdx?: number }): boolean {
    return _fieldNav.isAtExitedFieldStart.call(this, pos, fi);
  }

  private isExitedFieldStartPosition(pos: DocumentPosition): boolean {
    return _fieldNav.isExitedFieldStartPosition.call(this, pos);
  }

  isAtExitedFieldEnd(pos: DocumentPosition, fi?: { fieldId?: number; endCharIdx?: number }): boolean {
    return _fieldNav.isAtExitedFieldEnd.call(this, pos, fi);
  }

  /** 빈 누름틀 안내문 클릭 후 첫 입력 위치를 실제 field start로 정규화한다. */
  prepareClickHereInputPosition(): DocumentPosition {
    return _fieldNav.prepareClickHereInputPosition.call(this);
  }

  /** 마우스로 누름틀 위치를 직접 클릭하면 키보드 경계 이탈 상태를 해제한다. */
  prepareClickHerePointerEntry(pageX?: number): void {
    _fieldNav.prepareClickHerePointerEntry.call(this, pageX);
  }

  private prepareClickHerePointerBoundaryExit(pos: DocumentPosition, fi: any, pageX: number): boolean {
    return _fieldNav.prepareClickHerePointerBoundaryExit.call(this, pos, fi, pageX);
  }

  private findEmptyClickHereGuideHitPosition(pos: DocumentPosition): DocumentPosition | null {
    return _fieldNav.findEmptyClickHereGuideHitPosition.call(this, pos);
  }

  /** 현재 위치가 빈 누름틀 안내문 영역인지 확인한다. */
  isClickHereGuidePosition(pos: DocumentPosition): boolean {
    return _fieldNav.isClickHereGuidePosition.call(this, pos);
  }

  /** 빈 누름틀 첫 입력 직후 안내문/마커 캐시를 새 field value 기준으로 다시 잡는다. */
  refreshClickHereAfterFirstInput(): void {
    _fieldNav.refreshClickHereAfterFirstInput.call(this);
  }

  private fieldBoundaryKey(pos: DocumentPosition, fieldId: number | undefined, charOffset: number): string {
    return _fieldNav.fieldBoundaryKey.call(this, pos, fieldId, charOffset);
  }

  private getClickHereBoundaryRects(pos: DocumentPosition, start: number, end: number): { startRect: CursorRect; endRect: CursorRect } | null {
    return _fieldNav.getClickHereBoundaryRects.call(this, pos, start, end);
  }

  /** 커서 위치의 필드 상태에 따라 낫표 마커를 표시/숨김한다 */
  private updateFieldMarkers(): void {
    _fieldNav.updateFieldMarkers.call(this);
  }

  /** 활성 필드를 무조건 해제하고 마커를 숨긴다 (안내문 다시 표시). */
  private clearActiveFieldMarker(): void {
    _fieldNav.clearActiveFieldMarker.call(this);
  }

  /** 커서가 누름틀 필드 내부인가? */
  isInField(): boolean {
    try {
      const fi = this.wasm.getFieldInfoAt(this.cursor.getPosition());
      return fi.inField;
    } catch { return false; }
  }

  /** 현재 커서 위치의 필드 정보를 반환한다. */
  getFieldInfo(): { fieldId: number; fieldType: string; guideName: string } | null {
    try {
      const fi = this.wasm.getFieldInfoAt(this.cursor.getPosition());
      if (fi.inField && fi.fieldId !== undefined) {
        return { fieldId: fi.fieldId, fieldType: fi.fieldType ?? '', guideName: fi.guideName ?? '' };
      }
    } catch { /* 무시 */ }
    return null;
  }

  /** 커서가 표 셀 내부인가? */
  isInTable(): boolean { return this.cursor.isInCell(); }

  /** 셀 선택 모드인가? */
  isInCellSelectionMode(): boolean { return this.cursor.isInCellSelectionMode(); }

  /** 여러 셀이 선택된 상태인가? */
  hasMultiCellSelection(): boolean {
    const range = this.cursor.getSelectedCellRange();
    return Boolean(range && (range.startRow !== range.endRow || range.startCol !== range.endCol));
  }

  /** 표 객체 선택 모드인가? */
  isInTableObjectSelection(): boolean { return this.cursor.isInTableObjectSelection(); }

  /** 선택된 표의 참조 정보 반환 */
  getSelectedTableRef() { return this.cursor.getSelectedTableRef(); }

  /** 표 객체 선택 해제 + 재렌더링 */
  exitTableObjectSelection(): void {
    this.cursor.exitTableObjectSelection();
    this.afterEdit();
  }

  /** 셀 선택 범위 반환 (셀 선택 모드가 아니면 null) */
  getSelectedCellRange() { return this.cursor.getSelectedCellRange(); }

  /** 셀 선택 중인 표의 컨텍스트 반환 */
  getCellTableContext() { return this.cursor.getCellTableContext(); }

  /** 제외 셀이 있는 비직사각형 셀 선택인가? */
  hasExcludedCellSelection(): boolean { return this.cursor.getExcludedCells().size > 0; }

  /** 셀 선택 모드 종료 */
  exitCellSelectionMode(): void {
    this.cursor.exitCellSelectionMode();
    this.cellSelectionRenderer?.clear();
    this.updateCaret();
  }

  /** Undo 가능한가? */
  canUndo(): boolean { return this.history.canUndo(); }

  /** Redo 가능한가? */
  canRedo(): boolean { return this.history.canRedo(); }

  /** Undo 실행 (커맨드 시스템용) */
  performUndo(): void { this.handleUndo(); }

  /** Redo 실행 (커맨드 시스템용) */
  performRedo(): void { this.handleRedo(); }

  /** 복사 (커맨드 시스템용 — 컨텍스트 메뉴/도구 상자에서 호출) */
  performCopy(): void {
    // 개체 선택 모드 → 직접 클립보드 기록 (textarea 포커스 불필요)
    if (this.cursor.isInPictureObjectSelection()) {
      const ref = this.cursor.getSelectedPictureRef();
      if (ref) {
        try {
          const cellPathJson = _keyboard.pictureCellPathJson(ref);
          this.wasm.copyControl(ref.sec, ref.ppi, ref.ci, cellPathJson);
          const text = this.wasm.getClipboardText() || '[그림]';
          let html = '';
          try { html = this.wasm.exportControlHtml(ref.sec, ref.ppi, ref.ci, cellPathJson) || ''; } catch { /* 무시 */ }
          const markedHtml = _keyboard.prepareRhwpInternalClipboardHtml(this, html, text);
          if (ref.type === 'image') {
            _keyboard.writeImageToClipboard(this.wasm, ref.sec, ref.ppi, ref.ci, text, markedHtml, cellPathJson)
              .catch(() => navigator.clipboard.writeText(text).catch(() => {}));
          } else {
            _keyboard.writeTextHtmlToClipboard(text, markedHtml)
              .catch(() => navigator.clipboard.writeText(text).catch(() => {}));
          }
        } catch (err) {
          console.warn('[InputHandler] 개체 복사 실패:', err);
        }
      }
      return;
    }
    if (this.cursor.isInTableObjectSelection()) {
      const ref = this.cursor.getSelectedTableRef();
      if (ref) {
        try {
          const target = tableObjectClipboardTarget(ref);
          this.wasm.copyControl(
            ref.sec, ref.ppi, target.controlIndex, target.ownerCellPathJson,
          );
          const text = this.wasm.getClipboardText() || '[표]';
          let html = '';
          try {
            html = this.wasm.exportControlHtml(
              ref.sec, ref.ppi, target.controlIndex, target.ownerCellPathJson,
            ) || '';
          } catch { /* 무시 */ }
          const markedHtml = _keyboard.prepareRhwpInternalClipboardHtml(this, html, text);
          _keyboard.writeTextHtmlToClipboard(text, markedHtml)
            .catch(() => navigator.clipboard.writeText(text).catch(() => {}));
        } catch (err) {
          console.warn('[InputHandler] 표 복사 실패:', err);
        }
      }
      return;
    }
    // 텍스트 선택 → textarea 포커스 후 execCommand
    this.focusTextarea();
    document.execCommand('copy');
  }

  /** 붙이기 (커맨드 시스템용 — 컨텍스트 메뉴/도구 상자에서 호출) */
  performPaste(): boolean {
    if (this.editMode === 'form') return false;
    this.focusTextarea();
    return document.execCommand('paste');
  }

  /** 잘라내기 (커맨드 시스템용 — 컨텍스트 메뉴/도구 상자에서 호출) */
  performCut(): void {
    if (this.editMode === 'form') return;
    // 개체 선택 모드 → 복사 + 삭제
    if (this.cursor.isInPictureObjectSelection()) {
      const ref = this.cursor.getSelectedPictureRef();
      if (ref) {
        // 클립보드에 복사
        this.performCopy();
        // 삭제
        this.cursor.moveOutOfSelectedPicture();
        this.pictureObjectRenderer?.clear();
        this.eventBus.emit('picture-object-selection-changed', false);
        this.executeOperation({ kind: 'snapshot', operationType: 'cutObject', operation: (wasm: WasmBridge) => {
          if (ref.type === 'image' && ref.cellPath && ref.cellPath.length > 0) {
            wasm.deleteCellPictureControlByPath(ref.sec, ref.ppi, ref.cellPath, ref.ci);
          } else if (ref.type === 'image') {
            wasm.deletePictureControl(ref.sec, ref.ppi, ref.ci);
          } else if (ref.type === 'equation') {
            wasm.deleteEquationControl(ref.sec, ref.ppi, ref.ci);
          } else {
            wasm.deleteShapeControl(ref.sec, ref.ppi, ref.ci);
          }
          return this.cursor.getPosition();
        }});
      }
      return;
    }
    if (this.cursor.isInTableObjectSelection()) {
      const ref = this.cursor.getSelectedTableRef();
      if (ref) {
        this.performCopy();
        this.cursor.moveOutOfSelectedTable();
        this.eventBus.emit('table-object-selection-changed', false);
        this.executeOperation({ kind: 'snapshot', operationType: 'cutTable', operation: (wasm: WasmBridge) => {
          wasm.deleteTableControl(ref.sec, ref.ppi, ref.ci);
          return this.cursor.getPosition();
        }});
      }
      return;
    }
    // 텍스트 선택 → textarea 포커스 후 execCommand
    this.focusTextarea();
    document.execCommand('cut');
  }

  /** 선택 영역 삭제 (커맨드 시스템용 — 편집 > 지우기) */
  performDelete(): void {
    if (this.editMode === 'form') return;
    if (this.cursor.isInPictureObjectSelection()) {
      const ref = this.cursor.getSelectedPictureRef();
      if (ref) {
        this.cursor.moveOutOfSelectedPicture();
        this.pictureObjectRenderer?.clear();
        this.eventBus.emit('picture-object-selection-changed', false);
        this.executeOperation({ kind: 'snapshot', operationType: 'deleteObject', operation: (wasm: WasmBridge) => {
          this.deleteObjectControl(ref);
          return this.cursor.getPosition();
        }});
      }
      return;
    }
    if (this.cursor.isInTableObjectSelection()) {
      const ref = this.cursor.getSelectedTableRef();
      if (!ref) return;
      if (ref.cellPath && ref.cellPath.length > 1) {
        this.cursor.moveOutOfSelectedTable();
        this.eventBus.emit('table-object-selection-changed', false);
        return;
      }
      this.cursor.moveOutOfSelectedTable();
      this.eventBus.emit('table-object-selection-changed', false);
      this.executeOperation({ kind: 'snapshot', operationType: 'deleteTable', operation: (wasm: WasmBridge) => {
        wasm.deleteTableControl(ref.sec, ref.ppi, ref.ci);
        return this.cursor.getPosition();
      }});
      return;
    }
    if (this.cursor.hasSelection()) {
      this.deleteSelection();
    }
  }

  /** 전체 선택 (커맨드 시스템용) */
  performSelectAll(): void { this.handleSelectAll(); }

  /** 모양 복사/붙여넣기 (커맨드 시스템용) */
  performFormatCopy(): void {
    if (this.applyCopiedFormatToCurrentTarget()) return;
    this.copyFormatAtCursor();
  }

  /** 모양 붙여넣기만 수행한다 (커맨드 시스템용) */
  performFormatPaste(): void {
    this.applyCopiedFormatToCurrentTarget();
  }

  private applyCopiedFormatToCurrentTarget(): boolean {
    if (!this.formatCopyState) return false;

    if (this.cursor.isInCellSelectionMode()) {
      if (this.formatCopyState.cellProps && Object.keys(this.formatCopyState.cellProps).length > 0) {
        const applied = this.applyCopiedCellPropsToSelection(this.formatCopyState.cellProps);
        if (applied) this.formatCopyState = null;
        return applied;
      }
      return false;
    }

    const sel = this.getSelection();
    if (!sel) return false;

    const { charProps, paraProps } = this.formatCopyState;
    if (Object.keys(charProps).length > 0) {
      this.applyCharPropsToRange(sel.start, sel.end, charProps);
    }
    if (Object.keys(paraProps).length > 0) {
      this.applyParaPropsToRange(sel.start, sel.end, paraProps);
    }
    // 한컴 호환: 복사한 모양은 한 번 붙여넣으면 자동 해제한다.
    this.formatCopyState = null;
    this.focusTextarea();
    return true;
  }

  private copyFormatAtCursor(): void {
    const currentCharProps = this.getCharProperties();
    const charProps = pickDefined(currentCharProps, FORMAT_COPY_CHAR_KEYS) as Partial<CharProperties>;
    if (charProps.fontIds === undefined && charProps.fontId === undefined) {
      const fontFamily = currentCharProps.fontFamily;
      if (fontFamily) {
        const fontId = this.wasm.findOrCreateFontId(fontFamily);
        if (fontId >= 0) charProps.fontId = fontId;
      }
    }
    const paraProps = normalizeFormatCopyParaProps(
      pickDefined(this.getParaProperties(), FORMAT_COPY_PARA_KEYS) as Partial<ParaProperties>,
    );
    const pos = this.cursor.getPosition();
    const cellProps = pos.parentParaIndex !== undefined
      ? pickDefined(
          this.wasm.getCellOwnProperties(pos.sectionIndex, pos.parentParaIndex, pos.controlIndex!, pos.cellIndex!),
          FORMAT_COPY_CELL_KEYS,
        ) as Partial<CellProperties>
      : undefined;
    this.formatCopyState = {
      charProps: JSON.parse(JSON.stringify(charProps)),
      paraProps: JSON.parse(JSON.stringify(paraProps)),
      cellProps: cellProps ? JSON.parse(JSON.stringify(cellProps)) : undefined,
    };
    this.focusTextarea();
  }

  private applyCopiedCellPropsToSelection(cellProps: Partial<CellProperties>): boolean {
    const ctx = this.cursor.getCellTableContext();
    const range = this.cursor.getSelectedCellRange();
    if (!ctx || !range) {
      this.focusTextarea();
      return false;
    }
    if (ctx.cellPath && ctx.cellPath.length > 1) {
      console.info('[InputHandler] 중첩 표 셀 모양복사는 아직 지원하지 않습니다');
      this.focusTextarea();
      return false;
    }

    const props = JSON.parse(JSON.stringify(cellProps)) as Partial<CellProperties>;
    this.executeOperation({
      kind: 'snapshot',
      operationType: 'formatCopyCellProps',
      operation: (wasm) => {
        const dims = wasm.getTableDimensions(ctx.sec, ctx.ppi, ctx.ci);
        const cellIndices = selectCellIndicesInRange(
          dims.cellCount,
          (cellIdx) => wasm.getCellInfo(ctx.sec, ctx.ppi, ctx.ci, cellIdx),
          range,
          this.cursor.getExcludedCells(),
        );
        for (const cellIdx of cellIndices) {
          wasm.setCellProperties(ctx.sec, ctx.ppi, ctx.ci, cellIdx, props);
        }
        return this.cursor.getPosition();
      },
    });
    this.focusTextarea();
    return true;
  }

  /** 서식 토글 (커맨드 시스템용) */
  toggleFormat(prop: 'bold' | 'italic' | 'underline' | 'strikethrough' | 'emboss' | 'engrave' | 'outline' | 'superscript' | 'subscript'): void {
    this.applyToggleFormat(prop);
  }

  /** 문단 정렬 적용 (커맨드 시스템용) */
  applyParaAlign(align: string): void {
    this.applyParaFormat({ alignment: align });
  }

  /** 줄 간격 적용 (커맨드 시스템용, Percent 타입) */
  setLineSpacing(value: number): void {
    this.applyParaFormat({ lineSpacing: value, lineSpacingType: 'Percent' });
  }

  /** 글꼴 크기 증감 (커맨드 시스템용, delta: HWPUNIT, 1pt=100) */
  adjustFontSize(delta: number): void {
    // [#4162] 선택이 없어도(캐럿만) applyCharFormat 이 캐럿 대기 서식으로 예약한다.
    const current = this.getCharPropertiesAtCursor();
    const newSize = Math.max(100, (current.fontSize ?? 1000) + delta); // 최소 1pt
    this.applyCharFormat({ fontSize: newSize });
  }

  /** 장평 증감 (커맨드 시스템용, delta: percent point) */
  adjustCharRatio(delta: number): void {
    const current = this.getCharPropertiesAtCursor();
    const currentRatio = current.ratios?.[0] ?? 100;
    const nextRatio = Math.max(50, Math.min(200, Math.round(currentRatio + delta)));
    this.applyCharFormat({ ratios: Array(7).fill(nextRatio) });
  }

  /** 자간 증감 (커맨드 시스템용, delta: percent point) */
  adjustCharSpacing(delta: number): void {
    const current = this.getCharPropertiesAtCursor();
    const currentSpacing = current.spacings?.[0] ?? 0;
    const nextSpacing = Math.max(-50, Math.min(50, Math.round(currentSpacing + delta)));
    this.applyCharFormat({ spacings: Array(7).fill(nextSpacing) });
  }

  /** 스타일 적용 (커맨드 시스템용) */
  applyStyle(styleId: number): void {
    try {
      const targets = this.getParaFormatTargetsAtCursor();
      if (targets.length === 0) return;
      const cursorBefore = this.cursor.getPosition();
      const operation = (wasm: WasmBridge): DocumentPosition => {
        for (const target of targets) {
          if (target.kind === 'body') {
            wasm.applyStyle(target.sec, target.para, styleId);
            continue;
          }
          wasm.applyCellStyle(
            target.sec,
            target.parentPara,
            target.controlIdx,
            target.cellIdx,
            target.cellParaIdx,
            styleId,
          );
        }
        return { ...cursorBefore };
      };
      this.executeOperation({ kind: 'snapshot', operationType: 'applyStyle', operation });
    } catch (err) {
      console.warn('[InputHandler] applyStyle 실패:', err);
    }
  }

  /** 개요 수준 변경 (delta: +1=한 수준 증가, -1=한 수준 감소) */
  changeOutlineLevel(delta: number): void {
    const pos = this.cursor.getPosition();
    try {
      const inCell = pos.parentParaIndex !== undefined;
      const currentStyle = inCell
        ? this.wasm.getCellStyleAt(
            pos.sectionIndex, pos.parentParaIndex!, pos.controlIndex!,
            pos.cellIndex!, pos.cellParaIndex!,
          )
        : this.wasm.getStyleAt(pos.sectionIndex, pos.paragraphIndex);

      // 현재 개요 수준 파싱 (개요 1~7)
      const match = currentStyle.name.match(/^개요\s*(\d)$/);
      if (!match) return; // 개요 스타일이 아니면 무시

      const currentLevel = parseInt(match[1], 10);
      const targetLevel = currentLevel + delta;
      if (targetLevel < 1 || targetLevel > 7) return;

      // 스타일 목록에서 대상 개요 스타일 찾기
      const styles = this.wasm.getStyleList();
      const targetStyle = styles.find(s => {
        const m = s.name.match(/^개요\s*(\d)$/);
        return m && parseInt(m[1], 10) === targetLevel;
      });
      if (!targetStyle) return;

      this.applyStyle(targetStyle.id);
    } catch (err) {
      console.warn('[InputHandler] changeOutlineLevel 실패:', err);
    }
  }

  /** 문단 번호 토글: None→Number, Number/Outline→None */
  toggleNumbering(): void {
    try {
      const props = this.getParaProperties();
      if (props.headType && props.headType !== 'None') {
        // 번호 해제
        this.applyParaFormat({ headType: 'None' } as Partial<import('@/core/types').ParaProperties>);
      } else {
        // 번호 적용
        const nid = this.wasm.ensureDefaultNumbering();
        this.applyParaFormat({
          headType: 'Number',
          numberingId: nid,
          paraLevel: 0,
        } as Partial<import('@/core/types').ParaProperties>);
      }
      this.focusTextarea();
    } catch (err) {
      console.warn('[InputHandler] toggleNumbering 실패:', err);
    }
  }

  /** 글머리표 토글: None→Bullet, Bullet→None */
  toggleBullet(bulletChar = '●'): void {
    try {
      const props = this.getParaProperties();
      if (props.headType === 'Bullet') {
        // 글머리표 해제
        this.applyParaFormat({ headType: 'None' } as Partial<import('@/core/types').ParaProperties>);
      } else {
        // 글머리표 적용
        const bid = this.wasm.ensureDefaultBullet(bulletChar);
        this.applyParaFormat({
          headType: 'Bullet',
          numberingId: bid,
          paraLevel: 0,
        } as Partial<import('@/core/types').ParaProperties>);
      }
      this.focusTextarea();
    } catch (err) {
      console.warn('[InputHandler] toggleBullet 실패:', err);
    }
  }

  /** 글머리표 적용 (팝업에서 선택한 문자, 토글 없이 항상 적용) */
  applyBullet(bulletChar: string): void {
    try {
      const bid = this.wasm.ensureDefaultBullet(bulletChar);
      this.applyParaFormat({
        headType: 'Bullet',
        numberingId: bid,
        paraLevel: 0,
      } as Partial<import('@/core/types').ParaProperties>);
      this.focusTextarea();
    } catch (err) {
      console.warn('[InputHandler] applyBullet 실패:', err);
    }
  }

  /** 문단 번호 모양 적용 (대화상자에서 선택한 numberingId) */
  applyNumbering(numberingId: number): void {
    try {
      this.applyParaFormat({
        headType: 'Number',
        numberingId,
        paraLevel: 0,
      } as Partial<import('@/core/types').ParaProperties>);
      this.focusTextarea();
    } catch (err) {
      console.warn('[InputHandler] applyNumbering 실패:', err);
    }
  }

  /** 글자 모양 대화상자용: 커서 위치의 글자 서식 조회 (커맨드 시스템용) */
  getCharProperties(): CharProperties {
    return this.getCharPropertiesAtCursor();
  }

  /** 문단 모양 대화상자용: 커서 위치의 문단 서식 조회 (커맨드 시스템용) */
  getParaProperties(): ParaProperties {
    // 머리말/꼬리말 모드
    if (this.cursor.isInHeaderFooter()) {
      const isHeader = this.cursor.headerFooterMode === 'header';
      return this.wasm.getParaPropertiesInHf(
        this.cursor.hfSectionIdx, isHeader, this.cursor.hfApplyTo, this.cursor.hfParaIdx,
      );
    }
    if (this.cursor.isInFootnote()) {
      return this.wasm.getParaPropertiesInFootnote(
        this.cursor.fnSectionIdx,
        this.cursor.fnParaIdx,
        this.cursor.fnControlIdx,
        this.cursor.fnInnerParaIdx,
      );
    }
    const pos = this.cursor.getPosition();
    if (pos.parentParaIndex !== undefined) {
      return this.wasm.getCellParaPropertiesAt(
        pos.sectionIndex, pos.parentParaIndex, pos.controlIndex!,
        pos.cellIndex!, pos.cellParaIndex!,
      );
    }
    return this.wasm.getParaPropertiesAt(pos.sectionIndex, pos.paragraphIndex);
  }

  /** 커서 위치의 문단 스타일 ID를 반환한다 (스타일 대화상자용) */
  getCurrentStyleId(): number {
    try {
      const pos = this.cursor.getPosition();
      const info = pos.parentParaIndex !== undefined
        ? this.wasm.getCellStyleAt(
            pos.sectionIndex, pos.parentParaIndex, pos.controlIndex!,
            pos.cellIndex!, pos.cellParaIndex!,
          )
        : this.wasm.getStyleAt(pos.sectionIndex, pos.paragraphIndex);
      return info.id;
    } catch {
      return 0;
    }
  }

  /** 현재 선택 범위를 반환한다 (커맨드 시스템용) */
  getSelection(): { start: DocumentPosition; end: DocumentPosition } | null {
    return this.cursor.getSelectionOrdered();
  }

  /** 지정된 선택 범위에 글자 서식을 적용한다 (커맨드 시스템용) */
  applyCharPropsToRange(
    start: DocumentPosition,
    end: DocumentPosition,
    props: Partial<CharProperties>,
  ): void {
    const cmd = new ApplyCharFormatCommand(start, end, props);
    this.executeOperation({ kind: 'command', command: cmd });
  }

  /** 지정된 선택 범위에 문단 서식을 적용한다 (커맨드 시스템용) */
  applyParaPropsToRange(
    start: DocumentPosition,
    end: DocumentPosition,
    props: Partial<ParaProperties>,
  ): void {
    try {
      const targets = this.getParaFormatTargetsForRange(start, end);
      this.executeParaFormatCommand(targets, props as Record<string, unknown>);
    } catch (err) {
      console.warn('[InputHandler] applyParaPropsToRange 실패:', err);
    }
  }

  /** 커서 위치 문단에 문단 서식을 적용한다 (커맨드 시스템용) */
  applyParaPropsAtCursor(props: Partial<ParaProperties>): void {
    this.applyParaFormat(props as Record<string, unknown>);
  }

  /**
   * [Task #2374] 이미 적용된 양식 값 변경을 역연산 커맨드로 기록한다(no-op 제외).
   * 미기록 시 이후 스냅샷 undo 가 값 변경 이전 문서를 복원해 양식 값을 무언 파괴한다
   * (#2337 계급). 양식 모드에서는 snapshot 이 게이트에서 드롭되므로 record 가 유일한
   * 기록 경로다. before==after(이미 선택된 라디오 재클릭 등)는 유령 엔트리 방지를 위해
   * 기록하지 않는다.
   */
  private recordFormValueChanges(targets: FormValueTarget[]): void {
    _formOverlay.recordFormValueChanges.call(this, targets);
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
  private formInCellLoc(formHit: FormObjectHitResult):
    { tablePara: number; tableCi: number; cellIdx: number; cellPara: number } | undefined {
    return _formOverlay.formInCellLoc.call(this, formHit);
  }

  /** 양식 개체 클릭 처리 */
  handleFormObjectClick(formHit: FormObjectHitResult, pageIdx: number, _zoom: number): void {
    _formOverlay.handleFormObjectClick.call(this, formHit, pageIdx, _zoom);
  }

  /** 라디오 버튼 클릭: 같은 그룹 내 다른 라디오 버튼 해제 */
  private handleRadioButtonClick(sec: number, para: number, ci: number): void {
    _formOverlay.handleRadioButtonClick.call(this, sec, para, ci);
  }

  /** 양식 개체 bbox를 scroll-content 내 절대 좌표로 변환 */
  private formBboxToOverlayRect(bbox: { x: number; y: number; w: number; h: number }, pageIdx: number): { left: number; top: number; width: number; height: number } {
    return _formOverlay.formBboxToOverlayRect.call(this, bbox, pageIdx);
  }

  /** 기존 양식 오버레이 제거 */
  private removeFormOverlay(): void {
    _formOverlay.removeFormOverlay.call(this);
  }

  /** ComboBox 드롭다운 오버레이 */
  private showComboBoxOverlay(sec: number, para: number, ci: number, formHit: FormObjectHitResult, pageIdx: number): void {
    _formOverlay.showComboBoxOverlay.call(this, sec, para, ci, formHit, pageIdx);
  }

  /** Edit 입력 오버레이 */
  private showEditOverlay(sec: number, para: number, ci: number, formHit: FormObjectHitResult, pageIdx: number): void {
    _formOverlay.showEditOverlay.call(this, sec, para, ci, formHit, pageIdx);
  }
}
