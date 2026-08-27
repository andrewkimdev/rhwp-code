/**
 * 개체 속성 대화상자 — 한컴 개체 속성 대화상자 재현
 *
 * 탭 구성 (글상자):
 *  기본 | 여백/캡션 | 선 | 채우기 | 글상자 | 그림자
 *
 * 탭 구성 (그림):
 *  기본 | 여백/캡션 | 선 | 그림 | 그림자
 *
 * 레이아웃: 좌측(탭+컨텐츠) + 우측(버튼) 패턴
 * CSS 접두어: pp-
 */
import type { PictureProperties, ShapeProperties, CellPathLike } from '@/core/types';
import type { WasmBridge } from '@/core/wasm-bridge';
import type { EventBus } from '@/core/event-bus';
import type { CommandServices } from '@/command/types';
import { enableDialogDrag } from './dialog-drag';
import {
  buildPicturePropsPatch,
  resolvePicturePropsApplyTarget,
  type PicturePropsApplyForm,
  type PicturePropsApplyTarget,
  type PicturePropsPatch,
} from './picture-props-apply-model';
import { setAreaDisabled } from './dialog-dom-helpers';
import { buildBasicTab } from './picture-props/tabs/basic';
import { buildMarginCaptionTab } from './picture-props/tabs/margin-caption';
import { buildLineTab } from './picture-props/tabs/line';
import { buildFillTab } from './picture-props/tabs/fill';
import { buildTextboxTab } from './picture-props/tabs/textbox';
import { buildShadowTab } from './picture-props/tabs/shadow';
import { buildPictureTab } from './picture-props/tabs/picture';
import { buildReflectionTab, buildGlowTab, buildSoftEdgeTab, buildStubTab } from './picture-props/tabs/effects';

/** HWPUNIT ↔ mm 변환 상수 (1 inch = 25.4 mm = 7200 HWPUNIT) */
const HWP_PER_MM = 7200 / 25.4; // ≈ 283.46

function hwpToMm(hwp: number): number {
  return hwp / HWP_PER_MM;
}

/** HWP ColorRef (BGR u32) → HTML hex (#rrggbb) */
function colorRefToHex(c: number): string {
  const b = (c >> 16) & 0xFF;
  const g = (c >> 8) & 0xFF;
  const r = c & 0xFF;
  return '#' + [r, g, b].map(v => v.toString(16).padStart(2, '0')).join('');
}

/** 탭 이름 — 그림용 */
const PICTURE_TAB_NAMES = ['기본', '여백/캡션', '선', '그림', '그림자', '반사', '네온', '열은 테두리'];
/** 탭 이름 — 글상자용 */
const SHAPE_TAB_NAMES = ['기본', '여백/캡션', '선', '채우기', '글상자', '그림자'];
/** 탭 이름 — OLE용 */
const OLE_TAB_NAMES = ['기본', '여백/캡션', '선'];
/** 탭 이름 — 직선용 (채우기/글상자 불필요) */
const LINE_TAB_NAMES = ['기본', '여백/캡션', '선', '그림자'];

/**
 * 개체 설명문(description)의 안전한 상한 길이(문자 수).
 *
 * Rust 직렬화기(`src/serializer/control.rs`)는 그림/글상자 등 개체의 CommonObjAttr
 * description 필드를 `write_hwp_string()`(`src/serializer/byte_writer.rs`)로 기록하는데,
 * 이 함수는 UTF-16 코드 유닛 수를 `as u16`으로 캐스팅해 길이 프리픽스를 만든다. 문자열
 * 길이가 65536 이상이면 캐스팅이 랩어라운드되어 길이 프리픽스와 실제 기록된 바이트 수가
 * 어긋난 손상된 레코드가 만들어진다(#2851/#2862/#2866/#2878과 동일 원인). `.rs`를 수정하지
 * 않는 범위에서, 손상 가능한 값이 wasm 호출까지 도달하지 않도록 프런트엔드에서 훨씬 낮은
 * 상한으로 미리 막는다.
 */
export const MAX_OBJECT_DESCRIPTION_LEN = 4000;

export class PicturePropsDialog {
  private overlay!: HTMLDivElement;
  private dialog!: HTMLDivElement;
  private built = false;

  private wasm: WasmBridge;
  private eventBus: EventBus;
  /** undo 기록 라우팅용 (없으면 wasm 직접 호출 fallback). */
  private services: CommandServices | undefined;

  // 탭
  private tabs: HTMLButtonElement[] = [];
  private panels: HTMLDivElement[] = [];
  private tabGroup!: HTMLDivElement;
  private body!: HTMLDivElement;

  // 현재 편집 대상
  private sec = 0;
  private para = 0;
  private ci = 0;
  objectType: 'image' | 'shape' | 'line' | 'group' | 'ole' = 'image';
  /** [Task #825] 머리말/꼬리말 그림 marker (Some 일 때 신규 API 사용). */
  private headerFooter: { kind: 'header' | 'footer'; outerParaIdx: number; outerControlIdx: number } | undefined;
  /** [Task #1138] 표 셀 내 객체 marker (Some 일 때 by_path API 사용). */
  private cellPath: CellPathLike | undefined;
  /** [Task #1138] 셀 paragraph 내 picture/shape control 인덱스 (cellPath 동반). */
  private innerControlIdx = 0;
  private props: PictureProperties | null = null;
  private shapeProps: ShapeProperties | null = null;

  // ── 기본 탭 컨트롤 ──
  // 크기
  widthInput!: HTMLInputElement;
  heightInput!: HTMLInputElement;
  sizeFixedCheck!: HTMLInputElement;
  keepRatioCheck!: HTMLInputElement;
  sizeLockControls: Array<HTMLInputElement | HTMLSelectElement | HTMLButtonElement> = [];
  syncingBasicSize = false;
  originalWidth = 0;
  originalHeight = 0;

  // 위치
  treatAsCharCheck!: HTMLInputElement;
  wrapBtns: HTMLButtonElement[] = [];
  private wrapValues = ['TopAndBottom', 'Square', 'Tight', 'BehindText', 'InFrontOfText'];
  bodyPosSelect!: HTMLSelectElement;
  horzRelSelect!: HTMLSelectElement;
  horzAlignSelect!: HTMLSelectElement;
  horzOffsetInput!: HTMLInputElement;
  vertRelSelect!: HTMLSelectElement;
  vertAlignSelect!: HTMLSelectElement;
  vertOffsetInput!: HTMLInputElement;
  posDetailEls: HTMLElement[] = [];
  pageAreaLimitCheck!: HTMLInputElement;
  overlapAllowCheck!: HTMLInputElement;
  samePageCheck!: HTMLInputElement;

  // 개체 회전
  rotationInput!: HTMLInputElement;
  horzFlipCheck!: HTMLInputElement;
  vertFlipCheck!: HTMLInputElement;

  // 기울이기
  skewHInput!: HTMLInputElement;
  skewVInput!: HTMLInputElement;

  // 기타
  protectCheck!: HTMLInputElement;
  descInput!: HTMLInputElement;

  // ── 여백/캡션 탭 컨트롤 ──
  outerMarginLeftInput!: HTMLInputElement;
  outerMarginRightInput!: HTMLInputElement;
  outerMarginTopInput!: HTMLInputElement;
  outerMarginBottomInput!: HTMLInputElement;
  captionBtns: HTMLButtonElement[] = [];
  captionSizeInput!: HTMLInputElement;
  captionGapInput!: HTMLInputElement;
  captionExpandCheck!: HTMLInputElement;
  captionSingleLineCheck!: HTMLInputElement;

  // ── 선 탭 컨트롤 ──
  lineColorInput!: HTMLInputElement;
  lineTypeSelect!: HTMLSelectElement;
  lineEndSelect!: HTMLSelectElement;
  lineWidthInput!: HTMLInputElement;
  arrowStartSelect!: HTMLSelectElement;
  arrowEndSelect!: HTMLSelectElement;
  arrowStartSizeSelect!: HTMLSelectElement;
  arrowEndSizeSelect!: HTMLSelectElement;
  cornerBtns: HTMLButtonElement[] = [];
  cornerCustomRadio!: HTMLInputElement;
  cornerCustomInput!: HTMLInputElement;
  arcBtns: HTMLButtonElement[] = [];
  lineTransInput!: HTMLInputElement;
  lineInnerCheck!: HTMLInputElement;

  // ── 채우기 탭 컨트롤 ──
  fillNoneRadio!: HTMLInputElement;
  fillSolidRadio!: HTMLInputElement;
  fillGradientRadio!: HTMLInputElement;
  fillImageCheck!: HTMLInputElement;
  solidFaceColor!: HTMLInputElement;
  solidPatColor!: HTMLInputElement;
  solidPatternSelect!: HTMLSelectElement;
  gradStartColor!: HTMLInputElement;
  gradEndColor!: HTMLInputElement;
  gradTypeSelect!: HTMLSelectElement;
  gradDirBtns: HTMLButtonElement[] = [];
  gradCenterXInput!: HTMLInputElement;
  gradCenterYInput!: HTMLInputElement;
  gradTiltInput!: HTMLInputElement;
  gradBlurInput!: HTMLInputElement;
  gradReverseCenterInput!: HTMLInputElement;
  imageFileInput!: HTMLInputElement;
  imageEmbedCheck!: HTMLInputElement;
  imageFillTypeSelect!: HTMLSelectElement;
  imageBrightnessInput!: HTMLInputElement;
  imageEffectSelect!: HTMLSelectElement;
  imageContrastInput!: HTMLInputElement;
  imageWatermarkCheck!: HTMLInputElement;
  fillTransInput!: HTMLInputElement;
  // 채우기 영역 참조 (라디오 전환용)
  solidArea!: HTMLDivElement;
  gradientArea!: HTMLDivElement;
  imageArea!: HTMLDivElement;

  // ── 글상자 탭 컨트롤 ──
  tbMarginLeftInput!: HTMLInputElement;
  tbMarginRightInput!: HTMLInputElement;
  tbMarginTopInput!: HTMLInputElement;
  tbMarginBottomInput!: HTMLInputElement;
  tbVertAlignBtns: HTMLButtonElement[] = [];
  tbVertWriteCheck!: HTMLInputElement;
  tbEngLay!: HTMLButtonElement;
  tbEngStand!: HTMLButtonElement;
  tbSingleLineCheck!: HTMLInputElement;
  tbFieldNameInput!: HTMLInputElement;
  tbFormModeCheck!: HTMLInputElement;

  // ── 그림 탭 컨트롤 ──
  // [Task #741 후속] 외부 file path 그림 영역 영역 dialog 표시 영역
  picFileNameInput!: HTMLInputElement;
  picEmbedCheck!: HTMLInputElement;
  picScaleXInput!: HTMLInputElement;
  picScaleYInput!: HTMLInputElement;
  picKeepRatioCheck!: HTMLInputElement;
  picCropLeftInput!: HTMLInputElement;
  picCropTopInput!: HTMLInputElement;
  picCropRightInput!: HTMLInputElement;
  picCropBottomInput!: HTMLInputElement;
  picPadLeftInput!: HTMLInputElement;
  picPadTopInput!: HTMLInputElement;
  picPadRightInput!: HTMLInputElement;
  picPadBottomInput!: HTMLInputElement;
  picEffectRadios: HTMLInputElement[] = [];
  picBrightnessInput!: HTMLInputElement;
  picContrastInput!: HTMLInputElement;
  picWatermarkCheck!: HTMLInputElement;
  picTransparencyInput!: HTMLInputElement;

  // ── 그림자 탭 컨트롤 ──
  shadowTypeBtns: HTMLButtonElement[] = [];
  shadowColorInput!: HTMLInputElement;
  shadowHInput!: HTMLInputElement;
  shadowVInput!: HTMLInputElement;
  shadowDirBtns: HTMLButtonElement[] = [];
  shadowTransInput!: HTMLInputElement;

  constructor(wasm: WasmBridge, eventBus: EventBus, services?: CommandServices) {
    this.wasm = wasm;
    this.eventBus = eventBus;
    this.services = services;
  }

  // ════════════════════════════════════════════════════════
  //  공개 API
  // ════════════════════════════════════════════════════════

  open(
    sec: number,
    para: number,
    ci: number,
    type: 'image' | 'shape' | 'line' | 'group' | 'ole' = 'image',
    headerFooter?: { kind: 'header' | 'footer'; outerParaIdx: number; outerControlIdx: number },
    cellPath?: CellPathLike,
    innerControlIdx?: number,
  ): void {
    this.build();
    this.sec = sec;
    this.para = para;
    this.ci = ci;
    this.objectType = type;
    this.headerFooter = headerFooter;
    // [Task #1138] cellPath 가 있으면 표 셀 내부 객체 — by_path API 사용.
    this.cellPath = cellPath;
    this.innerControlIdx = innerControlIdx ?? 0;

    // getter 분기:
    // - shape/line/group/ole: cellPath > 외부 (셀 안 도형은 by_path API)
    // - picture: headerFooter > cellPath > 외부
    //   [Task #1151 v4] 셀 안 inline picture 는 getCellPicturePropertiesByPath
    //   wasm API 호출.
    if (type === 'shape' || type === 'line' || type === 'group' || type === 'ole') {
      if (cellPath) {
        this.shapeProps = this.wasm.getCellShapePropertiesByPath(sec, para, cellPath, this.innerControlIdx);
      } else {
        this.shapeProps = this.wasm.getShapeProperties(sec, para, ci);
      }
      this.props = this.shapeProps as unknown as PictureProperties;
    } else {
      this.shapeProps = null;
      if (headerFooter) {
        // [Task #825] 머리말/꼬리말 그림 — 별도 5-tuple API
        this.props = this.wasm.getHeaderFooterPictureProperties(
          sec, headerFooter.outerParaIdx, headerFooter.outerControlIdx, para, ci,
        );
      } else if (cellPath) {
        // [Task #1151 v4] 셀 안 inline picture — by_path API 호출.
        this.props = this.wasm.getCellPicturePropertiesByPath(sec, para, cellPath, this.innerControlIdx);
      } else {
        this.props = this.wasm.getPictureProperties(sec, para, ci);
      }
    }
    this.originalWidth = this.props.width;
    this.originalHeight = this.props.height;
    this.rebuildTabs();
    this.populateFromProps();
    this.switchTab(0);
    document.body.appendChild(this.overlay);
    setTimeout(() => this.widthInput?.select(), 50);
  }

  hide(): void {
    this.overlay?.remove();
  }

  // ════════════════════════════════════════════════════════
  //  빌드
  // ════════════════════════════════════════════════════════

  private build(): void {
    if (this.built) return;
    this.built = true;

    // 오버레이
    this.overlay = document.createElement('div');
    this.overlay.className = 'modal-overlay';

    // 다이얼로그 컨테이너
    this.dialog = document.createElement('div');
    this.dialog.className = 'dialog-wrap pp-dialog';

    // 타이틀 바
    const titleBar = document.createElement('div');
    titleBar.className = 'dialog-title';
    titleBar.textContent = '개체 속성';
    const closeBtn = document.createElement('button');
    closeBtn.className = 'dialog-close';
    closeBtn.textContent = '\u00D7';
    closeBtn.addEventListener('click', () => this.hide());
    titleBar.appendChild(closeBtn);
    this.dialog.appendChild(titleBar);

    // 메인 레이아웃: 좌측(탭+컨텐츠) + 우측(버튼)
    const mainRow = document.createElement('div');
    mainRow.className = 'cs-main-row';

    // 좌측 영역
    const leftCol = document.createElement('div');
    leftCol.className = 'cs-left-col';

    // 탭 그룹
    this.tabGroup = document.createElement('div');
    this.tabGroup.className = 'dialog-tabs';
    leftCol.appendChild(this.tabGroup);

    // 탭 패널 컨테이너
    this.body = document.createElement('div');
    this.body.className = 'dialog-body';
    leftCol.appendChild(this.body);

    // 우측 버튼 영역
    const rightCol = document.createElement('div');
    rightCol.className = 'cs-right-col';
    const okBtn = document.createElement('button');
    okBtn.className = 'dialog-btn dialog-btn-primary';
    okBtn.textContent = '설정(D)';
    okBtn.addEventListener('click', () => this.handleOk());
    const cancelBtn = document.createElement('button');
    cancelBtn.className = 'dialog-btn';
    cancelBtn.textContent = '취소';
    cancelBtn.addEventListener('click', () => this.hide());
    rightCol.appendChild(okBtn);
    rightCol.appendChild(cancelBtn);

    mainRow.appendChild(leftCol);
    mainRow.appendChild(rightCol);
    this.dialog.appendChild(mainRow);
    this.overlay.appendChild(this.dialog);

    // Escape
    this.overlay.addEventListener('keydown', (e) => {
      if (e.key === 'Escape') { e.stopPropagation(); this.hide(); }
    });

    enableDialogDrag(this.dialog, titleBar);
  }

  private switchTab(idx: number): void {
    this.tabs.forEach((t, i) => t.classList.toggle('active', i === idx));
    this.panels.forEach((p, i) => p.classList.toggle('active', i === idx));
  }

  /** 개체 타입에 따라 탭을 재구성한다 */
  private rebuildTabs(): void {
    this.tabGroup.replaceChildren();
    this.body.replaceChildren();
    this.tabs = [];
    this.panels = [];
    this.sizeLockControls = [];

    const tabNames = this.objectType === 'ole' ? OLE_TAB_NAMES
      : this.objectType === 'line' ? LINE_TAB_NAMES
      : (this.objectType === 'shape' || this.objectType === 'group') ? SHAPE_TAB_NAMES
      : PICTURE_TAB_NAMES;
    tabNames.forEach((name, i) => {
      const btn = document.createElement('button');
      btn.className = 'dialog-tab';
      btn.textContent = name;
      btn.addEventListener('click', () => this.switchTab(i));
      this.tabGroup.appendChild(btn);
      this.tabs.push(btn);
    });

    // 패널 생성
    const builders: Record<string, () => HTMLDivElement> = {
      '기본': () => this.buildBasicPanel(),
      '여백/캡션': () => this.buildMarginCaptionPanel(),
      '선': () => this.buildLinePanel(),
      '채우기': () => this.buildFillPanel(),
      '글상자': () => this.buildTextboxPanel(),
      '그림': () => this.buildPicturePanel(),
      '그림자': () => this.buildShadowPanel(),
      '반사': () => this.buildReflectionPanel(),
      '네온': () => this.buildGlowPanel(),
      '열은 테두리': () => this.buildSoftEdgePanel(),
    };
    tabNames.forEach((name) => {
      const builder = builders[name];
      const panel = builder ? builder() : this.buildStubPanel(name);
      this.panels.push(panel);
      this.body.appendChild(panel);
    });
  }

  // ════════════════════════════════════════════════════════
  //  기본 탭
  // ════════════════════════════════════════════════════════

  private buildBasicPanel(): HTMLDivElement {
    return buildBasicTab(this);
  }

  // ════════════════════════════════════════════════════════
  //  여백/캡션 탭
  // ════════════════════════════════════════════════════════

  private buildMarginCaptionPanel(): HTMLDivElement {
    return buildMarginCaptionTab(this);
  }

  // ════════════════════════════════════════════════════════
  //  선 탭
  // ════════════════════════════════════════════════════════

  private buildLinePanel(): HTMLDivElement {
    return buildLineTab(this);
  }

  // ════════════════════════════════════════════════════════
  //  채우기 탭
  // ════════════════════════════════════════════════════════

  private buildFillPanel(): HTMLDivElement {
    return buildFillTab(this);
  }

  // ════════════════════════════════════════════════════════
  //  글상자 탭
  // ════════════════════════════════════════════════════════

  private buildTextboxPanel(): HTMLDivElement {
    return buildTextboxTab(this);
  }

  // ════════════════════════════════════════════════════════
  //  그림자 탭
  // ════════════════════════════════════════════════════════

  private buildShadowPanel(): HTMLDivElement {
    return buildShadowTab(this);
  }

  // ════════════════════════════════════════════════════════
  //  그림 탭
  // ════════════════════════════════════════════════════════

  private buildPicturePanel(): HTMLDivElement {
    return buildPictureTab(this);
  }

  // ════════════════════════════════════════════════════════
  //  반사 탭
  // ════════════════════════════════════════════════════════

  private buildReflectionPanel(): HTMLDivElement {
    return buildReflectionTab(this);
  }

  // ════════════════════════════════════════════════════════
  //  네온 탭
  // ════════════════════════════════════════════════════════

  private buildGlowPanel(): HTMLDivElement {
    return buildGlowTab(this);
  }

  // ════════════════════════════════════════════════════════
  //  열은 테두리 탭
  // ════════════════════════════════════════════════════════

  private buildSoftEdgePanel(): HTMLDivElement {
    return buildSoftEdgeTab(this);
  }

  /** 미구현 탭 스텁 패널 */
  private buildStubPanel(name: string): HTMLDivElement {
    return buildStubTab(this, name);
  }

  // ════════════════════════════════════════════════════════
  //  설정/취소
  // ════════════════════════════════════════════════════════

  private captureApplyForm(): PicturePropsApplyForm {
    return {
      common: {
        sizeProtect: this.sizeFixedCheck.checked,
        width: this.widthInput.value,
        height: this.heightInput.value,
        treatAsChar: this.treatAsCharCheck.checked,
        textWrap: this.getSelectedWrap(),
        horzRelTo: this.horzRelSelect.value,
        horzAlign: this.horzAlignSelect.value,
        horzOffset: this.horzOffsetInput.value,
        vertRelTo: this.vertRelSelect.value,
        vertAlign: this.vertAlignSelect.value,
        vertOffset: this.vertOffsetInput.value,
        restrictInPage: this.pageAreaLimitCheck.checked,
        allowOverlap: this.overlapAllowCheck.checked,
        description: this.descInput.value,
      },
      transform: {
        rotation: this.rotationInput
          ? { value: this.rotationInput.value, disabled: this.rotationInput.disabled }
          : undefined,
        horzFlip: this.horzFlipCheck
          ? { value: this.horzFlipCheck.checked, disabled: this.horzFlipCheck.disabled }
          : undefined,
        vertFlip: this.vertFlipCheck
          ? { value: this.vertFlipCheck.checked, disabled: this.vertFlipCheck.disabled }
          : undefined,
      },
      outerMargin: {
        left: this.outerMarginLeftInput?.value,
        top: this.outerMarginTopInput?.value,
        right: this.outerMarginRightInput?.value,
        bottom: this.outerMarginBottomInput?.value,
      },
      caption: {
        present: this.captionBtns.length > 0,
        activeIndex: this.captionBtns.findIndex(b => b.classList.contains('active')),
        size: this.captionSizeInput?.value ?? '',
        gap: this.captionGapInput?.value ?? '',
        includeMargin: this.captionExpandCheck?.checked ?? false,
      },
      line: {
        color: this.lineColorInput?.value,
        width: this.lineWidthInput?.value,
        type: this.lineTypeSelect?.value,
        end: this.lineEndSelect?.value,
        arrowStart: this.arrowStartSelect?.value,
        arrowEnd: this.arrowEndSelect?.value,
        arrowStartSize: this.arrowStartSizeSelect?.value,
        arrowEndSize: this.arrowEndSizeSelect?.value,
      },
      shapeTextBox: {
        marginLeft: this.tbMarginLeftInput?.value,
        marginTop: this.tbMarginTopInput?.value,
        marginRight: this.tbMarginRightInput?.value,
        marginBottom: this.tbMarginBottomInput?.value,
        verticalAlign: this.tbVertAlignBtns.find(b => b.classList.contains('active'))?.dataset.value,
      },
      shapeCorner: {
        customChecked: this.cornerCustomRadio?.checked ?? false,
        customValue: this.cornerCustomInput?.value,
        activeIndex: this.cornerBtns.findIndex(b => b.classList.contains('active')),
      },
      shapeFill: {
        solidChecked: this.fillSolidRadio?.checked,
        gradientChecked: this.fillGradientRadio?.checked,
        solidColors: this.solidFaceColor
          ? { face: this.solidFaceColor.value, pattern: this.solidPatColor?.value ?? '' }
          : undefined,
        patternType: this.solidPatternSelect?.value,
        gradientType: this.gradTypeSelect?.value,
        gradientAngle: this.gradTiltInput?.value,
        gradientCenterX: this.gradCenterXInput?.value,
        gradientCenterY: this.gradCenterYInput?.value,
        gradientBlur: this.gradBlurInput?.value,
        transparency: this.fillTransInput?.value,
      },
      shapeShadow: {
        present: this.shadowTypeBtns.length > 0,
        activeIndex: this.shadowTypeBtns.findIndex(b => b.classList.contains('active')),
        color: this.shadowColorInput?.value ?? '',
        offsetX: this.shadowHInput?.value ?? '',
        offsetY: this.shadowVInput?.value ?? '',
      },
      image: {
        scale: this.picScaleXInput
          ? { x: this.picScaleXInput.value, y: this.picScaleYInput?.value ?? '' }
          : undefined,
        crop: this.picCropLeftInput
          ? {
              left: this.picCropLeftInput.value,
              top: this.picCropTopInput?.value ?? '',
              right: this.picCropRightInput?.value ?? '',
              bottom: this.picCropBottomInput?.value ?? '',
            }
          : undefined,
        padding: this.picPadLeftInput
          ? {
              left: this.picPadLeftInput.value,
              top: this.picPadTopInput?.value ?? '',
              right: this.picPadRightInput?.value ?? '',
              bottom: this.picPadBottomInput?.value ?? '',
            }
          : undefined,
        effectControlsPresent: this.picEffectRadios.length > 0,
        selectedEffect: this.picEffectRadios.find(r => r.checked)?.value,
        brightness: this.picBrightnessInput?.value,
        contrast: this.picContrastInput?.value,
        transparency: this.picTransparencyInput?.value,
      },
    };
  }

  private applyPropertyPatchToWasm(
    target: PicturePropsApplyTarget,
    patch: PicturePropsPatch,
  ): void {
    switch (target.kind) {
      case 'cell-shape':
        this.wasm.setCellShapePropertiesByPath(
          target.sec, target.para, target.cellPath, target.innerControlIdx, patch,
        );
        return;
      case 'body-shape':
        this.wasm.setShapeProperties(target.sec, target.para, target.ci, patch);
        return;
      case 'header-footer-picture':
        // 캡션 신규 생성은 native API에서 미지원이며 기존 속성 변경만 허용한다.
        this.wasm.setHeaderFooterPictureProperties(
          target.sec, target.outerParaIdx, target.outerControlIdx,
          target.para, target.ci, patch,
        );
        return;
      case 'cell-picture':
        this.wasm.setCellPicturePropertiesByPath(
          target.sec, target.para, target.cellPath, target.innerControlIdx, patch,
        );
        return;
      case 'body-picture':
        this.wasm.setPictureProperties(target.sec, target.para, target.ci, patch);
    }
  }

  private applyPropertyPatch(patch: PicturePropsPatch): void {
    const target = resolvePicturePropsApplyTarget(this.objectType, {
      sec: this.sec,
      para: this.para,
      ci: this.ci,
      headerFooter: this.headerFooter,
      cellPath: this.cellPath,
      innerControlIdx: this.innerControlIdx,
    });
    const applyProps = () => this.applyPropertyPatchToWasm(target, patch);

    // 개체 속성 변경도 undo 대상이다. services 미주입 환경에서만 직접 적용한다.
    const ih = this.services?.getInputHandler();
    if (ih) {
      ih.executeOperation({
        kind: 'snapshot',
        operationType: 'objectProps',
        operation: () => {
          applyProps();
          return ih.getCursorPosition();
        },
      });
    } else {
      applyProps();
      this.eventBus.emit('document-changed');
    }
  }

  private handleOk(): void {
    if (!this.props) { this.hide(); return; }
    if (this.descInput.value.length > MAX_OBJECT_DESCRIPTION_LEN) {
      this.showDescriptionPrompt();
      return;
    }
    const patch = buildPicturePropsPatch(
      this.objectType,
      this.props,
      this.shapeProps,
      this.captureApplyForm(),
    );
    if (Object.keys(patch).length > 0) this.applyPropertyPatch(patch);
    this.hide();
  }

  // ════════════════════════════════════════════════════════
  //  데이터 ↔ UI
  // ════════════════════════════════════════════════════════

  private populateFromProps(): void {
    if (!this.props) return;
    // [Task #741 후속 — 한컴 viewer 정합] 외부 file path 그림 영역 dialog 표시 영역.
    // props.externalPath 영역 영역 그대로 표시 (resolved path 영역, 한컴 viewer 영역 영역
    // 원본 절대 경로 영역 영역 access 부재 시 HWP file 영역 영역 같은 dir 영역 image 영역
    // 영역 영역 path 영역 영역 갱신 — populate_external_images_from_dir / inject_external_image
    // 영역 영역 변경 영역 ~~basename~~ → resolved local path 영역 영역).
    if (this.picFileNameInput && this.picEmbedCheck) {
      if (this.props.externalPath) {
        this.picFileNameInput.value = this.props.externalPath;
        this.picEmbedCheck.checked = false;
      } else {
        this.picFileNameInput.value = '(문서에 포함된 그림)';
        this.picEmbedCheck.checked = true;
      }
    }
    this.widthInput.value = hwpToMm(this.props.width).toFixed(2);
    this.heightInput.value = hwpToMm(this.props.height).toFixed(2);
    this.sizeFixedCheck.checked = this.props.sizeProtect ?? false;
    this.treatAsCharCheck.checked = this.props.treatAsChar;
    this.selectWrap(this.wrapValues.indexOf(this.props.textWrap));
    this.horzRelSelect.value = this.props.textWrap === 'TopAndBottom'
      ? 'TakePlace'
      : this.props.horzRelTo;
    this.horzAlignSelect.value = this.props.horzAlign;
    this.horzOffsetInput.value = hwpToMm(this.props.horzOffset).toFixed(2);
    this.vertRelSelect.value = this.props.vertRelTo;
    this.vertAlignSelect.value = this.props.vertAlign;
    this.vertOffsetInput.value = hwpToMm(this.props.vertOffset).toFixed(2);
    this.pageAreaLimitCheck.checked = this.props.restrictInPage ?? true;
    this.overlapAllowCheck.checked = this.props.allowOverlap ?? false;
    this.descInput.value = this.props.description;
    this.skewHInput.value = '0';
    this.skewVInput.value = '0';
    this.updatePositionVisibility();
    this.updateOverlapOption();

    // Shape/Line 전용 필드
    if ((this.objectType === 'shape' || this.objectType === 'line' || this.objectType === 'group' || this.objectType === 'ole') && this.shapeProps) {
      const sp = this.shapeProps;
      const isOle = this.objectType === 'ole';

      // 기본 탭 — 회전/대칭
      if (this.rotationInput) {
        this.rotationInput.value = String(sp.rotationAngle ?? 0);
        this.rotationInput.disabled = isOle;
      }
      if (this.horzFlipCheck) {
        this.horzFlipCheck.checked = !!sp.horzFlip;
        this.horzFlipCheck.disabled = isOle;
      }
      if (this.vertFlipCheck) {
        this.vertFlipCheck.checked = !!sp.vertFlip;
        this.vertFlipCheck.disabled = isOle;
      }

      if (isOle) {
        if (this.outerMarginLeftInput) this.outerMarginLeftInput.value = hwpToMm(this.props.outerMarginLeft ?? 0).toFixed(2);
        if (this.outerMarginRightInput) this.outerMarginRightInput.value = hwpToMm(this.props.outerMarginRight ?? 0).toFixed(2);
        if (this.outerMarginTopInput) this.outerMarginTopInput.value = hwpToMm(this.props.outerMarginTop ?? 0).toFixed(2);
        if (this.outerMarginBottomInput) this.outerMarginBottomInput.value = hwpToMm(this.props.outerMarginBottom ?? 0).toFixed(2);
        if (this.captionBtns.length > 0) {
          this.captionBtns.forEach(b => b.disabled = false);
          this.captionSizeInput.disabled = false;
          this.captionGapInput.disabled = false;
          this.captionExpandCheck.disabled = false;

          if (this.props.hasCaption) {
            const gridIdx = this.captionGridIndex(this.props.captionDirection, this.props.captionVertAlign);
            this.captionBtns.forEach((b, j) => b.classList.toggle('active', j === gridIdx));
            this.captionSizeInput.value = hwpToMm(this.props.captionWidth ?? 0).toFixed(2);
            this.captionGapInput.value = hwpToMm(this.props.captionSpacing ?? 0).toFixed(2);
            this.captionExpandCheck.checked = !!this.props.captionIncludeMargin;
          } else {
            this.captionBtns.forEach(b => b.classList.remove('active'));
            this.captionSizeInput.value = '30.00';
            this.captionGapInput.value = '3.00';
            this.captionExpandCheck.checked = false;
          }
        }
      } else {
        // 글상자 탭 — 여백
        if (this.tbMarginLeftInput) this.tbMarginLeftInput.value = hwpToMm(sp.tbMarginLeft ?? 510).toFixed(2);
        if (this.tbMarginRightInput) this.tbMarginRightInput.value = hwpToMm(sp.tbMarginRight ?? 510).toFixed(2);
        if (this.tbMarginTopInput) this.tbMarginTopInput.value = hwpToMm(sp.tbMarginTop ?? 141).toFixed(2);
        if (this.tbMarginBottomInput) this.tbMarginBottomInput.value = hwpToMm(sp.tbMarginBottom ?? 141).toFixed(2);

        // 글상자 탭 — 세로 정렬 아이콘 버튼
        const va = sp.tbVerticalAlign ?? 'Top';
        this.tbVertAlignBtns.forEach(b => b.classList.toggle('active', b.dataset.value === va));
      }

      // 선 탭 — borderColor/borderWidth
      if (this.lineColorInput && sp.borderColor !== undefined) {
        this.lineColorInput.value = colorRefToHex(sp.borderColor);
      }
      if (this.lineWidthInput && sp.borderWidth !== undefined) {
        this.lineWidthInput.value = hwpToMm(sp.borderWidth).toFixed(2);
      }

      // 선 탭 — 선 종류/끝모양/화살표
      if (this.lineTypeSelect && sp.lineType !== undefined) this.lineTypeSelect.value = String(sp.lineType);
      if (this.lineEndSelect && sp.lineEndShape !== undefined) this.lineEndSelect.value = String(sp.lineEndShape);
      if (!isOle && this.arrowStartSelect && sp.arrowStart !== undefined) this.arrowStartSelect.value = String(sp.arrowStart);
      if (!isOle && this.arrowEndSelect && sp.arrowEnd !== undefined) this.arrowEndSelect.value = String(sp.arrowEnd);
      if (!isOle && this.arrowStartSizeSelect && sp.arrowStartSize !== undefined) this.arrowStartSizeSelect.value = String(sp.arrowStartSize);
      if (!isOle && this.arrowEndSizeSelect && sp.arrowEndSize !== undefined) this.arrowEndSizeSelect.value = String(sp.arrowEndSize);

      // 선 탭 — 모서리 곡률
      if (!isOle && this.cornerBtns.length > 0 && sp.roundRate !== undefined) {
        const rr = sp.roundRate;
        if (rr === 0) {
          this.cornerBtns.forEach((b, i) => b.classList.toggle('active', i === 0));
        } else if (rr >= 50) {
          this.cornerBtns.forEach((b, i) => b.classList.toggle('active', i === 2));
        } else if (rr > 0 && rr < 50) {
          // 곡률 지정 모드
          if (this.cornerCustomRadio) this.cornerCustomRadio.checked = true;
          if (this.cornerCustomInput) {
            this.cornerCustomInput.value = String(rr);
            this.cornerCustomInput.disabled = false;
          }
          this.cornerBtns.forEach(b => b.classList.remove('active'));
        }
      }

      if (!isOle) {
        // 채우기 탭 — fillType
        const ft = sp.fillType ?? 'none';
        if (this.fillNoneRadio) this.fillNoneRadio.checked = (ft === 'none');
        if (this.fillSolidRadio) this.fillSolidRadio.checked = (ft === 'solid');
        if (this.fillGradientRadio) this.fillGradientRadio.checked = (ft === 'gradient');

        // 채우기 — 단색
        if (this.solidFaceColor && sp.fillBgColor !== undefined) {
          this.solidFaceColor.value = colorRefToHex(sp.fillBgColor);
        }
        if (this.solidPatColor && sp.fillPatColor !== undefined) {
          this.solidPatColor.value = colorRefToHex(sp.fillPatColor);
        }
        if (this.solidPatternSelect && sp.fillPatType !== undefined) {
          // fillPatType은 정수 — select 값으로 매핑은 추후 세분화
        }

        // 채우기 — 그러데이션
        if (this.gradTypeSelect && sp.gradientType !== undefined) {
          // gradientType 정수 → select 인덱스 매핑은 추후 세분화
        }
        if (this.gradCenterXInput && sp.gradientCenterX !== undefined) this.gradCenterXInput.value = String(sp.gradientCenterX);
        if (this.gradCenterYInput && sp.gradientCenterY !== undefined) this.gradCenterYInput.value = String(sp.gradientCenterY);
        if (this.gradBlurInput && sp.gradientBlur !== undefined) this.gradBlurInput.value = String(sp.gradientBlur);
        if (this.gradTiltInput && sp.gradientAngle !== undefined) this.gradTiltInput.value = String(sp.gradientAngle);

        // 채우기 — 투명도 (한컴 호환: alpha=0 → 불투명, alpha=255 → 완전 투명)
        if (this.fillTransInput && sp.fillAlpha !== undefined) {
          const pct = Math.round(sp.fillAlpha * 100 / 255);
          this.fillTransInput.value = String(pct);
          this.fillTransInput.disabled = false;
        }

        // 그림자 탭 초기화
        if (this.shadowTypeBtns.length > 0) {
          const st = (sp as any).shadowType ?? 0;
          this.shadowTypeBtns.forEach((b, i) => b.classList.toggle('active', i === st));
          const enabled = st > 0;
          this.shadowColorInput.disabled = !enabled;
          this.shadowHInput.disabled = !enabled;
          this.shadowVInput.disabled = !enabled;
          this.shadowDirBtns.forEach(b => b.disabled = !enabled);
          if ((sp as any).shadowColor !== undefined) {
            this.shadowColorInput.value = colorRefToHex((sp as any).shadowColor);
          }
          if ((sp as any).shadowOffsetX !== undefined) {
            this.shadowHInput.value = hwpToMm((sp as any).shadowOffsetX).toFixed(1);
          }
          if ((sp as any).shadowOffsetY !== undefined) {
            this.shadowVInput.value = hwpToMm((sp as any).shadowOffsetY).toFixed(1);
          }
        }

        // 채우기 영역 활성화 상태 업데이트
        if (this.solidArea) {
          const isSolid = ft === 'solid';
          const isGrad = ft === 'gradient';
          this.solidArea.style.opacity = isSolid ? '1' : '0.4';
          this.gradientArea.style.opacity = isGrad ? '1' : '0.4';
          this.setAreaDisabled(this.solidArea, !isSolid);
          this.setAreaDisabled(this.gradientArea, !isGrad);
        }
      }
    } else {
      // 그림 개체일 때
      const pp = this.props!;

      // 기본 탭 — 회전/대칭 활성화
      if (this.rotationInput) {
        this.rotationInput.value = String(pp.rotationAngle ?? 0);
        this.rotationInput.disabled = false;
      }
      if (this.horzFlipCheck) {
        this.horzFlipCheck.checked = !!pp.horzFlip;
        this.horzFlipCheck.disabled = false;
      }
      if (this.vertFlipCheck) {
        this.vertFlipCheck.checked = !!pp.vertFlip;
        this.vertFlipCheck.disabled = false;
      }

      // 여백/캡션 탭 — 바깥 여백
      if (this.outerMarginLeftInput) this.outerMarginLeftInput.value = hwpToMm(pp.outerMarginLeft ?? 0).toFixed(2);
      if (this.outerMarginRightInput) this.outerMarginRightInput.value = hwpToMm(pp.outerMarginRight ?? 0).toFixed(2);
      if (this.outerMarginTopInput) this.outerMarginTopInput.value = hwpToMm(pp.outerMarginTop ?? 0).toFixed(2);
      if (this.outerMarginBottomInput) this.outerMarginBottomInput.value = hwpToMm(pp.outerMarginBottom ?? 0).toFixed(2);

      // 여백/캡션 탭 — 캡션 바인딩
      if (this.captionBtns.length > 0) {
        // 캡션 버튼 활성화
        this.captionBtns.forEach(b => b.disabled = false);
        this.captionSizeInput.disabled = false;
        this.captionGapInput.disabled = false;
        this.captionExpandCheck.disabled = false;

        if (pp.hasCaption) {
          // 방향 + 세로 정렬 → 3×3 그리드 인덱스 매핑
          const gridIdx = this.captionGridIndex(pp.captionDirection, pp.captionVertAlign);
          this.captionBtns.forEach((b, j) => b.classList.toggle('active', j === gridIdx));
          this.captionSizeInput.value = hwpToMm(pp.captionWidth ?? 0).toFixed(2);
          this.captionGapInput.value = hwpToMm(pp.captionSpacing ?? 0).toFixed(2);
          this.captionExpandCheck.checked = !!pp.captionIncludeMargin;
        } else {
          this.captionBtns.forEach(b => b.classList.remove('active'));
          this.captionSizeInput.value = '30.00';
          this.captionGapInput.value = '3.00';
          this.captionExpandCheck.checked = false;
        }
      }

      // 선 탭 — 테두리 바인딩
      if (this.lineColorInput && pp.borderColor !== undefined) {
        this.lineColorInput.value = colorRefToHex(pp.borderColor);
      }
      if (this.lineWidthInput && pp.borderWidth !== undefined) {
        this.lineWidthInput.value = hwpToMm(pp.borderWidth).toFixed(2);
      }

      // 그림 탭 — 확대/축소 비율
      if (this.picScaleXInput && pp.originalWidth > 0) {
        this.picScaleXInput.value = ((pp.width / pp.originalWidth) * 100).toFixed(2);
        this.picScaleYInput.value = ((pp.height / pp.originalHeight) * 100).toFixed(2);
      }
      // 그림 탭 — 자르기
      if (this.picCropLeftInput) {
        this.picCropLeftInput.value = hwpToMm(pp.cropLeft ?? 0).toFixed(2);
        this.picCropTopInput.value = hwpToMm(pp.cropTop ?? 0).toFixed(2);
        this.picCropRightInput.value = hwpToMm(pp.cropRight ?? 0).toFixed(2);
        this.picCropBottomInput.value = hwpToMm(pp.cropBottom ?? 0).toFixed(2);
      }
      // 그림 탭 — 안쪽 여백 (그림 여백)
      if (this.picPadLeftInput) {
        this.picPadLeftInput.value = hwpToMm(pp.paddingLeft ?? 0).toFixed(2);
        this.picPadTopInput.value = hwpToMm(pp.paddingTop ?? 0).toFixed(2);
        this.picPadRightInput.value = hwpToMm(pp.paddingRight ?? 0).toFixed(2);
        this.picPadBottomInput.value = hwpToMm(pp.paddingBottom ?? 0).toFixed(2);
      }
      // 그림 탭 — 효과
      if (this.picEffectRadios.length > 0) {
        const effectVal = pp.effect ?? 'RealPic';
        this.picEffectRadios.forEach(r => {
          r.checked = r.value === effectVal || (r.value === 'Original' && effectVal === 'RealPic' && (pp.brightness !== 0 || pp.contrast !== 0));
        });
        if (!this.picEffectRadios.some(r => r.checked)) {
          this.picEffectRadios[0].checked = true;
        }
      }
      if (this.picBrightnessInput) this.picBrightnessInput.value = String(pp.brightness ?? 0);
      if (this.picContrastInput) this.picContrastInput.value = String(pp.contrast ?? 0);
      if (this.picWatermarkCheck) {
        this.picWatermarkCheck.checked = (pp.brightness === 70 && pp.contrast === -50);
      }
      if (this.picTransparencyInput) {
        this.picTransparencyInput.value = String(pp.transparency ?? 0);
        this.picTransparencyInput.disabled = false;
      }
    }
    this.updateSizeProtectControls();
  }

  updateSizeProtectControls(): void {
    if (!this.sizeFixedCheck) return;
    const locked = this.sizeFixedCheck.checked;
    const transformLocked = locked || this.objectType === 'ole';
    this.sizeLockControls.forEach((control) => {
      control.disabled = locked;
    });
    // OLE 개체는 한컴과 같이 변형 항목을 편집 대상으로 노출하지 않는다.
    if (this.rotationInput) this.rotationInput.disabled = transformLocked;
    if (this.horzFlipCheck) this.horzFlipCheck.disabled = transformLocked;
    if (this.vertFlipCheck) this.vertFlipCheck.disabled = transformLocked;
    if (this.skewHInput) this.skewHInput.disabled = true;
    if (this.skewVInput) this.skewVInput.disabled = true;
  }

  updatePositionVisibility(): void {
    const hidden = this.treatAsCharCheck.checked;
    this.posDetailEls.forEach(el => {
      el.style.display = hidden ? 'none' : '';
    });
    this.updateOverlapOption();
  }

  updateOverlapOption(): void {
    if (!this.overlapAllowCheck || !this.pageAreaLimitCheck) return;
    const restricted = this.pageAreaLimitCheck.checked;
    this.overlapAllowCheck.disabled = restricted || this.treatAsCharCheck.checked;
    if (restricted) {
      this.overlapAllowCheck.checked = false;
    }
  }

  selectWrap(idx: number): void {
    this.wrapBtns.forEach((b, i) => b.classList.toggle('active', i === idx));
    if (!this.horzRelSelect) return;
    if (this.wrapValues[idx] === 'TopAndBottom') {
      this.horzRelSelect.value = 'TakePlace';
    } else if (this.horzRelSelect.value === 'TakePlace') {
      this.horzRelSelect.value = this.props?.horzRelTo ?? 'Column';
    }
  }

  private getSelectedWrap(): string {
    const idx = this.wrapBtns.findIndex(b => b.classList.contains('active'));
    if (idx >= 0) return this.wrapValues[idx];
    // 활성 버튼 없음 = 현재 배치가 UI 버튼에 매핑되지 않는 값('Through' 등,
    // populateFromProps 의 indexOf = -1). 여기서 'Square' 로 대체하면 사용자가
    // 아무것도 바꾸지 않고 확인만 눌러도 diff 가 생겨 배치가 조용히 변경·
    // 저장되므로, 개체의 원래 배치 값을 그대로 보존한다.
    return this.props?.textWrap ?? 'Square';
  }

  /** 캡션 direction + vertAlign → 3×3 그리드 인덱스 */
  private captionGridIndex(dir: string, vAlign: string): number {
    // 0:왼위 1:위 2:오위 3:왼 4:중앙 5:오 6:왼아 7:아래 8:오아
    const col = dir === 'Left' ? 0 : dir === 'Right' ? 2 : 1;
    const row = (dir === 'Left' || dir === 'Right')
      ? (vAlign === 'Top' ? 0 : vAlign === 'Bottom' ? 2 : 1)
      : (dir === 'Top' ? 0 : 2);
    return row * 3 + col;
  }

  /**
   * 개체 설명문 서브 대화상자 표시
   */
  showDescriptionPrompt(): void {
    const overlay = document.createElement('div');
    overlay.className = 'modal-overlay';

    const dlg = document.createElement('div');
    dlg.className = 'dialog-wrap pp-desc-dialog';

    // 타이틀
    const titleBar = document.createElement('div');
    titleBar.className = 'dialog-title';
    titleBar.textContent = '개체 설명문';
    const closeBtn = document.createElement('button');
    closeBtn.className = 'dialog-close';
    closeBtn.textContent = '\u00D7';
    closeBtn.addEventListener('click', () => overlay.remove());
    titleBar.appendChild(closeBtn);
    dlg.appendChild(titleBar);

    // 메인: 좌측(textarea) + 우측(버튼)
    const mainRow = document.createElement('div');
    mainRow.className = 'cs-main-row';

    const leftCol = document.createElement('div');
    leftCol.className = 'cs-left-col';
    const textarea = document.createElement('textarea');
    textarea.className = 'pp-desc-textarea';
    textarea.maxLength = MAX_OBJECT_DESCRIPTION_LEN;
    textarea.value = this.descInput.value;
    leftCol.appendChild(textarea);

    const errorLabel = document.createElement('div');
    errorLabel.className = 'pp-desc-error';
    errorLabel.style.color = '#c00';
    errorLabel.style.fontSize = '11px';
    errorLabel.style.display = 'none';
    errorLabel.textContent = `개체 설명문은 ${MAX_OBJECT_DESCRIPTION_LEN}자를 넘을 수 없습니다.`;
    leftCol.appendChild(errorLabel);

    const rightCol = document.createElement('div');
    rightCol.className = 'cs-right-col';
    const okBtn = document.createElement('button');
    okBtn.className = 'dialog-btn dialog-btn-primary';
    okBtn.textContent = '확인(D)';
    okBtn.addEventListener('click', () => {
      if (textarea.value.length > MAX_OBJECT_DESCRIPTION_LEN) {
        errorLabel.style.display = '';
        return;
      }
      this.descInput.value = textarea.value;
      overlay.remove();
    });
    const cancelBtn = document.createElement('button');
    cancelBtn.className = 'dialog-btn';
    cancelBtn.textContent = '취소';
    cancelBtn.addEventListener('click', () => overlay.remove());
    rightCol.appendChild(okBtn);
    rightCol.appendChild(cancelBtn);

    mainRow.appendChild(leftCol);
    mainRow.appendChild(rightCol);
    dlg.appendChild(mainRow);
    overlay.appendChild(dlg);
    enableDialogDrag(dlg, titleBar);

    // Escape
    overlay.addEventListener('keydown', (e) => {
      if (e.key === 'Escape') { e.stopPropagation(); overlay.remove(); }
    });

    document.body.appendChild(overlay);
    setTimeout(() => textarea.focus(), 50);
  }

  // ════════════════════════════════════════════════════════
  //  유틸리티
  // ════════════════════════════════════════════════════════

  /** 영역 내 모든 input/select/button을 disabled 처리 */
  private setAreaDisabled(area: HTMLElement, disabled: boolean): void {
    setAreaDisabled(area, disabled);
  }
}
