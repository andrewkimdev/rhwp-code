/** input-handler object-placement/drawing-mode methods — extracted from InputHandler class */
/* eslint-disable @typescript-eslint/no-explicit-any */

import type { WasmBridge } from '@/core/wasm-bridge';
import type { DocumentPosition, PageInfo } from '@/core/types';

const SVG_NS = 'http://www.w3.org/2000/svg';
const PX_TO_HWPUNIT = 75;

function availableDropWidthPx(pageInfo: PageInfo, pageX: number): number {
  const bodyWidth = Math.max(1, pageInfo.width - pageInfo.marginLeft - pageInfo.marginRight);
  const columns = pageInfo.columns?.filter((column) => column.width > 0) ?? [];
  if (columns.length === 0) return bodyWidth;

  const containing = columns.find((column) => pageX >= column.x && pageX <= column.x + column.width);
  if (containing) return Math.min(containing.width, bodyWidth);

  const nearest = columns.reduce((best, column) => {
    const bestCenter = best.x + best.width / 2;
    const columnCenter = column.x + column.width / 2;
    return Math.abs(columnCenter - pageX) < Math.abs(bestCenter - pageX) ? column : best;
  }, columns[0]);
  return Math.min(nearest.width, bodyWidth);
}

function fitDroppedImageSizeRaw(
  naturalWidth: number,
  naturalHeight: number,
  pageInfo: PageInfo | null,
  pageX: number,
): { width: number; height: number } {
  const originalWidth = Math.round(naturalWidth * PX_TO_HWPUNIT);
  const originalHeight = Math.round(naturalHeight * PX_TO_HWPUNIT);
  if (!pageInfo || originalWidth <= 0 || originalHeight <= 0) {
    return { width: originalWidth, height: originalHeight };
  }

  const maxWidth = Math.floor(availableDropWidthPx(pageInfo, pageX) * PX_TO_HWPUNIT);
  const maxHeight = Math.floor(
    Math.max(1, pageInfo.height - pageInfo.marginTop - pageInfo.marginBottom) * PX_TO_HWPUNIT,
  );
  const scale = Math.min(1, maxWidth / originalWidth, maxHeight / originalHeight);
  if (!Number.isFinite(scale) || scale <= 0) {
    return { width: originalWidth, height: originalHeight };
  }
  return {
    width: Math.max(1, Math.round(originalWidth * scale)),
    height: Math.max(1, Math.round(originalHeight * scale)),
  };
}

function createOverlaySvg(): SVGSVGElement {
  const svg = document.createElementNS(SVG_NS, 'svg');
  svg.style.width = '100%';
  svg.style.height = '100%';
  svg.style.overflow = 'visible';
  return svg;
}

function setSvgAttrs(el: SVGElement, attrs: Record<string, string | number>): void {
  for (const [key, value] of Object.entries(attrs)) {
    el.setAttribute(key, String(value));
  }
}

function appendOverlayLine(
  svg: SVGSVGElement,
  x1: number,
  y1: number,
  x2: number,
  y2: number,
  dashed = false,
): void {
  const line = document.createElementNS(SVG_NS, 'line');
  setSvgAttrs(line, {
    x1,
    y1,
    x2,
    y2,
    stroke: '#333',
    'stroke-width': 2,
  });
  if (dashed) line.setAttribute('stroke-dasharray', '6,3');
  svg.appendChild(line);
}

function createOverlayLabel(x: number, y: number, text: string): HTMLDivElement {
  const label = document.createElement('div');
  label.style.cssText =
    `position:fixed;left:${x}px;top:${y}px;` +
    'background:rgba(0,0,0,0.75);color:#fff;font-size:11px;padding:2px 6px;' +
    'border-radius:3px;white-space:nowrap;pointer-events:none';
  label.textContent = text;
  return label;
}

/** 그림 배치 모드 진입: 파일 선택 후 호출. 마우스로 영역 지정 대기 */
export function enterImagePlacementMode(this: any, data: Uint8Array, ext: string, naturalWidth: number, naturalHeight: number, fileName: string = ''): void {
  this.imagePlacementMode = true;
  this.imagePlacementData = { data, ext, fileName, naturalWidth, naturalHeight };
  this.imagePlacementDrag = null;
  this.container.style.cursor = 'crosshair';
}

/** 외부 파일 드롭 그림 삽입: 한컴처럼 원본 크기, 글자처럼 취급으로 바로 넣는다. */
export function insertDroppedImageAtClientPoint(
  this: any,
  data: Uint8Array,
  ext: string,
  naturalWidth: number,
  naturalHeight: number,
  fileName: string,
  clientX: number,
  clientY: number,
): { ok: boolean; error?: string } {
  const pagePoint = this.pagePointFromClientPoint(clientX, clientY);
  if (!pagePoint) {
    return { ok: false, error: '그림을 넣을 문단을 찾지 못했습니다.' };
  }
  if (naturalWidth <= 0 || naturalHeight <= 0) {
    return { ok: false, error: '이미지 크기를 확인할 수 없습니다.' };
  }

  let hit: DocumentPosition | null = null;
  try {
    hit = this.wasm.hitTest(pagePoint.pageIdx, pagePoint.pageX, pagePoint.pageY);
  } catch {
    hit = null;
  }
  if (!hit) {
    return { ok: false, error: '그림을 넣을 문단을 찾지 못했습니다.' };
  }

  const sec = hit.sectionIndex;
  const isTextBoxHit = hit.isTextBox === true;
  const hasPath = (hit.cellPath?.length ?? 0) > 0 && hit.parentParaIndex !== undefined;
  const inCell = hasPath && !isTextBoxHit;
  const inTextBox = hasPath && isTextBoxHit;
  const paraIdx = (inCell || inTextBox) && hit.parentParaIndex !== undefined
    ? hit.parentParaIndex
    : hit.paragraphIndex;
  const cellPath = (inCell || inTextBox) ? hit.cellPath ?? [] : [];
  const cellPathJson = cellPath.length > 0 ? JSON.stringify(cellPath) : '';
  const pageInfo = this.getPageInfoForDrop(pagePoint.pageIdx);
  const { width, height } = fitDroppedImageSizeRaw(naturalWidth, naturalHeight, pageInfo, pagePoint.pageX);
  const desc =
    `그림입니다.\r\n원본 그림의 이름: ${fileName}\r\n원본 그림의 크기: 가로 ${naturalWidth}pixel, 세로 ${naturalHeight}pixel`;

  try {
    // 삽입 + 인라인 전환을 하나의 스냅샷으로 기록 (Undo 지원, pasteImage 경로와 동일 패턴)
    let insertError: string | null = null;
    this.executeOperation({ kind: 'snapshot', operationType: 'insertPicture', operation: (wasm: WasmBridge) => {
      const result = wasm.insertPicture(
        sec,
        paraIdx,
        hit.charOffset,
        cellPathJson,
        data,
        width,
        height,
        naturalWidth,
        naturalHeight,
        ext,
        desc,
        undefined,
        undefined,
      );
      if (!result.ok) {
        insertError = (result as any).error || '삽입 위치 또는 이미지 정보를 확인할 수 없습니다.';
        return hit;
      }

      const logicalOffset = typeof result.logicalOffset === 'number'
        ? result.logicalOffset
        : hit.charOffset + 1;
      const cursorAfter: DocumentPosition = inTextBox
        ? { ...hit, charOffset: logicalOffset }
        : {
            sectionIndex: sec,
            paragraphIndex: result.paraIdx ?? paraIdx,
            charOffset: logicalOffset,
          };

      if (inTextBox && cellPath.length > 0) {
        wasm.setCellPicturePropertiesByPath(
          sec,
          paraIdx,
          cellPath,
          result.controlIdx,
          { treatAsChar: true },
        );
      } else {
        wasm.setPictureProperties(
          sec,
          result.paraIdx ?? paraIdx,
          result.controlIdx,
          { treatAsChar: true },
        );
      }
      this.cursor.clearSelection();
      return cursorAfter;
    }});
    if (insertError) {
      return { ok: false, error: insertError };
    }
    this.active = true;
    this.focusTextarea();
    return { ok: true };
  } catch (err) {
    console.warn('[InputHandler] 드롭 그림 삽입 실패:', err);
    return { ok: false, error: err instanceof Error ? err.message : String(err) };
  }
}

/** 글상자 배치 모드 진입: 메뉴에서 호출. 마우스로 영역 지정 대기 */
export function enterTextboxPlacementMode(this: any): void {
  // 글상자는 백엔드에서 text_box(내부 문단)를 가진 도형으로 생성되어야 한다.
  // 'rectangle'을 전달하면 text_box 없는 Rectangle이 만들어져 커서 진입·타이핑·붙여넣기가 모두 실패한다(#1280).
  this.shapePlacementType = 'textbox';
  this.textboxPlacementMode = true;
  this.textboxPlacementDrag = null;
  this.container.style.cursor = 'crosshair';
}

/** 도형 배치 모드 진입 (도형 타입 지정) */
export function enterShapePlacementMode(this: any, shapeType: string): void {
  this.shapePlacementType = shapeType;
  if (shapeType.startsWith('connector-')) {
    // 연결선: 개체 연결점 클릭→드래그→연결점 모드
    this.connectorDrawingMode = true;
    this.connectorType = shapeType;
    this.connectorStartRef = null;
    this.container.style.cursor = 'crosshair';
  } else if (shapeType === 'polygon') {
    // 다각형: 클릭-클릭-더블클릭 모드
    this.polygonDrawingMode = true;
    this.polygonPoints = [];
    this.polygonMousePos = null;
    this.container.style.cursor = 'crosshair';
  } else {
    this.textboxPlacementMode = true;
    this.textboxPlacementDrag = null;
    this.container.style.cursor = 'crosshair';
  }
}

/** 다각형 그리기: 꼭짓점 추가 (클릭) */
export function polygonAddPoint(this: any, clientX: number, clientY: number): void {
  this.polygonPoints.push({ x: clientX, y: clientY });
  this.updatePolygonOverlay(clientX, clientY);
}

/** 다각형 그리기: 마우스 이동 시 프리뷰 갱신 */
export function updatePolygonOverlay(this: any, mx: number, my: number): void {
  this.polygonMousePos = { x: mx, y: my };
  if (!this.polygonOverlay) {
    this.polygonOverlay = document.createElement('div');
    this.polygonOverlay.style.cssText =
      'position:fixed;left:0;top:0;width:100vw;height:100vh;pointer-events:none;z-index:9999;';
    document.body.appendChild(this.polygonOverlay);
  }
  const pts = this.polygonPoints as { x: number; y: number }[];
  if (pts.length === 0) {
    this.polygonOverlay.replaceChildren();
    return;
  }

  const svg = createOverlaySvg();
  // 확정된 변
  for (let i = 0; i < pts.length - 1; i++) {
    appendOverlayLine(svg, pts[i].x, pts[i].y, pts[i + 1].x, pts[i + 1].y);
  }
  // 마지막 점 → 마우스 위치 (프리뷰)
  const last = pts[pts.length - 1];
  appendOverlayLine(svg, last.x, last.y, mx, my, true);
  // 꼭짓점 마커
  for (const p of pts) {
    const circle = document.createElementNS(SVG_NS, 'circle');
    setSvgAttrs(circle, {
      cx: p.x,
      cy: p.y,
      r: 3,
      fill: '#fff',
      stroke: '#333',
      'stroke-width': 1,
    });
    svg.appendChild(circle);
  }
  // 크기 표시
  const allX = [...pts.map(p => p.x), mx];
  const allY = [...pts.map(p => p.y), my];
  const minX = Math.min(...allX), maxX = Math.max(...allX);
  const minY = Math.min(...allY), maxY = Math.max(...allY);
  const zoom = this.viewportManager.getZoom();
  const wMm = ((maxX - minX) / zoom * 25.4 / 96).toFixed(1);
  const hMm = ((maxY - minY) / zoom * 25.4 / 96).toFixed(1);
  const sizeLabel = createOverlayLabel(maxX + 4, maxY + 4, `${wMm} × ${hMm} mm`);

  this.polygonOverlay.replaceChildren(svg, sizeLabel);
}

/** 다각형 그리기: 완료 (더블클릭 또는 시작점 근접) */
export function finishPolygonDrawing(this: any): void {
  const pts = this.polygonPoints as { x: number; y: number }[];
  if (pts.length < 2) { this.cancelPolygonDrawing(); return; }

  // 화면 좌표 → 종이 좌표 (HWPUNIT)
  const zoom = this.viewportManager.getZoom();
  const scrollContent = this.container.querySelector('#scroll-content');
  const contentRect = scrollContent?.getBoundingClientRect();
  if (!contentRect) { this.cancelPolygonDrawing(); return; }

  // bbox 계산
  const xs = pts.map(p => p.x), ys = pts.map(p => p.y);
  const minX = Math.min(...xs), minY = Math.min(...ys);
  const maxX = Math.max(...xs), maxY = Math.max(...ys);
  const wPx = (maxX - minX) / zoom;
  const hPx = (maxY - minY) / zoom;
  const wHwp = Math.round(wPx * 75);
  const hHwp = Math.round(hPx * 75);

  // 종이 좌표로 오프셋 계산
  const centerX = (minX + maxX) / 2;
  const centerY = (minY + maxY) / 2;
  const cX = centerX - contentRect.left;
  const cY = centerY - contentRect.top;
  const pageIdx = this.virtualScroll.getPageAtPoint(cX, cY);
  const pageOffset = this.virtualScroll.getPageOffset(pageIdx);
  const pageDisplayWidth = this.virtualScroll.getPageWidth(pageIdx);
  const pageLeft = this.virtualScroll.getPageLeftResolved(pageIdx, (scrollContent as HTMLElement).clientWidth);
  const paperX = ((cX - pageLeft) / zoom) * 75;
  const paperY = ((cY - pageOffset) / zoom) * 75;
  const horzOffset = Math.max(0, Math.round(paperX - wHwp / 2));
  const vertOffset = Math.max(0, Math.round(paperY - hHwp / 2));

  // 꼭짓점을 HWPUNIT 로컬 좌표로 변환 (bbox 기준)
  const pointsHwp = pts.map(p => ({
    x: Math.round(((p.x - minX) / zoom) * 75),
    y: Math.round(((p.y - minY) / zoom) * 75),
  }));

  // 커서 위치
  const cursorPos = this.cursor.getPosition();
  const sec = cursorPos.sectionIndex;
  const paraIdx = cursorPos.paragraphIndex;
  const charOffset = cursorPos.charOffset;

  try {
    const result = this.wasm.createShapeControl({
      sectionIdx: sec,
      paraIdx,
      charOffset,
      width: wHwp || 2250,
      height: hHwp || 2250,
      horzOffset,
      vertOffset,
      shapeType: 'polygon',
      polygonPoints: pointsHwp,
    });
    if (result.ok) {
      this.eventBus.emit('document-changed');
      this.cursor.enterPictureObjectSelectionDirect(sec, result.paraIdx, result.controlIdx, 'shape');
      this.caret.hide();
      this.selectionRenderer.clear();
      this.renderPictureObjectSelection();
      this.eventBus.emit('picture-object-selection-changed', true);
    }
  } catch (err) {
    console.warn('[InputHandler] 다각형 삽입 실패:', err);
  }

  this.cancelPolygonDrawing();
}

/** 다각형 그리기: 취소 */
export function cancelPolygonDrawing(this: any): void {
  this.polygonDrawingMode = false;
  this.polygonPoints = [];
  this.polygonMousePos = null;
  if (this.polygonOverlay) {
    this.polygonOverlay.remove();
    this.polygonOverlay = null;
  }
  this.container.style.cursor = '';
}

/** 글상자 배치 모드 취소 */
export function cancelTextboxPlacement(this: any): void {
  this.textboxPlacementMode = false;
  this.textboxPlacementDrag = null;
  this.hideTextboxPlacementOverlay();
  this.container.style.cursor = '';
}

/** 도형 배치 오버레이 표시/갱신 (도형 타입별 SVG) */
export function showTextboxPlacementOverlay(this: any, x1: number, y1: number, x2: number, y2: number, shiftKey = false): void {
  if (!this.textboxPlacementOverlay) {
    this.textboxPlacementOverlay = document.createElement('div');
    this.textboxPlacementOverlay.style.cssText =
      'position:fixed;left:0;top:0;width:100vw;height:100vh;pointer-events:none;z-index:9999;';
    document.body.appendChild(this.textboxPlacementOverlay);
  }
  const type = this.shapePlacementType;

  const zoom = this.viewportManager.getZoom();
  const left = Math.min(x1, x2);
  const top = Math.min(y1, y2);
  const w = Math.abs(x2 - x1);
  const h = Math.abs(y2 - y1);
  // mm 크기 계산 (96dpi 기준: 1px = 25.4/96 mm)
  const wMm = (w / zoom * 25.4 / 96).toFixed(1);
  const hMm = (h / zoom * 25.4 / 96).toFixed(1);
  const sizeLabel = createOverlayLabel(left + w + 4, top + h + 4, `${wMm} × ${hMm} mm`);

  const svg = createOverlaySvg();
  let customLabel: HTMLDivElement | null = null;
  if (type === 'line') {
    let ex = x2, ey = y2;
    if (shiftKey) {
      const dx = x2 - x1, dy = y2 - y1;
      const angle = Math.atan2(dy, dx);
      const snapAngle = Math.round(angle / (Math.PI / 4)) * (Math.PI / 4);
      const dist = Math.sqrt(dx * dx + dy * dy);
      ex = x1 + dist * Math.cos(snapAngle);
      ey = y1 + dist * Math.sin(snapAngle);
    }
    if (this.textboxPlacementDrag && shiftKey) {
      this.textboxPlacementDrag.currentClientX = ex;
      this.textboxPlacementDrag.currentClientY = ey;
    }
    appendOverlayLine(svg, x1, y1, ex, ey, true);
    // 직선: 길이 표시
    const lenPx = Math.hypot(ex - x1, ey - y1);
    const lenMm = (lenPx / zoom * 25.4 / 96).toFixed(1);
    const mx = (x1 + ex) / 2, my = (y1 + ey) / 2;
    customLabel = createOverlayLabel(mx + 8, my + 8, `${lenMm} mm`);
  } else if (type === 'ellipse') {
    const cx = left + w / 2, cy = top + h / 2;
    const ellipse = document.createElementNS(SVG_NS, 'ellipse');
    setSvgAttrs(ellipse, {
      cx,
      cy,
      rx: w / 2,
      ry: h / 2,
      fill: 'rgba(0,0,0,0.05)',
      stroke: '#333',
      'stroke-width': 2,
      'stroke-dasharray': '6,3',
    });
    svg.appendChild(ellipse);
  } else if (type === 'arc') {
    // 호: 사각형에 내접하는 타원의 1/4 호
    // 우상 사분면: 상단 중앙 → 우측 중앙
    const rx = w / 2, ry = h / 2;
    if (rx > 1 && ry > 1) {
      const cx = left + w / 2, cy = top + h / 2;
      // 시작: 상단 중앙 (cx, top), 끝: 우측 중앙 (left+w, cy)
      const path = document.createElementNS(SVG_NS, 'path');
      setSvgAttrs(path, {
        d: `M ${cx} ${top} A ${rx} ${ry} 0 0 1 ${left + w} ${cy}`,
        fill: 'none',
        stroke: '#333',
        'stroke-width': 2,
        'stroke-dasharray': '6,3',
      });
      svg.appendChild(path);
      // 보조선: 내접 사각형
      const guide = document.createElementNS(SVG_NS, 'rect');
      setSvgAttrs(guide, {
        x: left,
        y: top,
        width: w,
        height: h,
        fill: 'none',
        stroke: '#ccc',
        'stroke-width': 1,
        'stroke-dasharray': '3,3',
      });
      svg.appendChild(guide);
    }
  } else if (type === 'polygon') {
    // 다각형: 삼각형 프리뷰
    const tx = left + w / 2, ty = top;
    const polygon = document.createElementNS(SVG_NS, 'polygon');
    setSvgAttrs(polygon, {
      points: `${tx},${ty} ${left + w},${top + h} ${left},${top + h}`,
      fill: 'rgba(0,0,0,0.05)',
      stroke: '#333',
      'stroke-width': 2,
      'stroke-dasharray': '6,3',
    });
    svg.appendChild(polygon);
  } else {
    // rectangle / textbox
    const rect = document.createElementNS(SVG_NS, 'rect');
    setSvgAttrs(rect, {
      x: left,
      y: top,
      width: w,
      height: h,
      fill: 'rgba(0,0,0,0.05)',
      stroke: '#333',
      'stroke-width': 2,
      'stroke-dasharray': '6,3',
    });
    svg.appendChild(rect);
  }

  const label = customLabel || (w > 5 || h > 5 ? sizeLabel : null);
  this.textboxPlacementOverlay.replaceChildren(...(label ? [svg, label] : [svg]));
}

/** 도형 배치 오버레이 제거 */
export function hideTextboxPlacementOverlay(this: any): void {
  if (this.textboxPlacementOverlay) {
    this.textboxPlacementOverlay.remove();
    this.textboxPlacementOverlay = null;
  }
}

/** 글상자 배치 완료: 마우스업 시 호출 */
export function finishTextboxPlacement(this: any, e: MouseEvent): void {
  const drag = this.textboxPlacementDrag;
  if (!drag) { this.cancelTextboxPlacement(); return; }

  this.hideTextboxPlacementOverlay();

  // 커서 위치에 도형 컨트롤 삽입 (한컴 동작: 커서 위치에 인라인 컨트롤 배치)
  const cursorPos = this.cursor.getPosition();
  const hit = {
    sectionIndex: cursorPos.sectionIndex,
    paragraphIndex: cursorPos.paragraphIndex,
    charOffset: cursorPos.charOffset,
  };
  if (hit.sectionIndex === undefined) { this.cancelTextboxPlacement(); return; }

  const sec = hit.sectionIndex;
  const paraIdx = hit.paragraphIndex;
  const charOffset = hit.charOffset;

  // 크기 결정
  const zoom = this.viewportManager.getZoom();
  let wPx: number, hPx: number;
  if (drag.isDragging) {
    wPx = Math.abs(drag.currentClientX - drag.startClientX) / zoom;
    hPx = Math.abs(drag.currentClientY - drag.startClientY) / zoom;
    const isLineType = this.shapePlacementType === 'line' || this.shapePlacementType.startsWith('connector-');
    if (!isLineType) {
      if (wPx < 10) wPx = 10;
      if (hPx < 10) hPx = 10;
    }
  } else {
    // 클릭만 한 경우
    const mm30 = 30 * 96 / 25.4; // ≈113.4 px
    if (this.shapePlacementType === 'line' || this.shapePlacementType.startsWith('connector-')) {
      wPx = mm30; hPx = 0; // 수평 직선/연결선
    } else {
      wPx = mm30; hPx = mm30;
    }
  }

  // px → HWPUNIT (1px = 75 HWPUNIT at 96 DPI)
  let wHwp = Math.round(wPx * 75);
  let hHwp = Math.round(hPx * 75);

  // 열 폭 초과 시 비례 축소
  try {
    const pageDef = this.wasm.getPageDef(sec);
    const colWidth = pageDef.width - pageDef.marginLeft - pageDef.marginRight;
    if (wHwp > colWidth) {
      const ratio = colWidth / wHwp;
      wHwp = Math.round(colWidth);
      hHwp = Math.round(hHwp * ratio);
    }
  } catch { /* 페이지 정보 없으면 그대로 */ }

  // 도형 위치 계산 (종이 기준 오프셋, HWPUNIT)
  // [Task #1280 v2] 글상자도 floating(InFrontOfText)으로 삽입하므로 종이 기준 오프셋을
  //   계산한다(기존 사각형 등과 동일 경로). 수정 전엔 글상자만 인라인이라 offset=0 으로 스킵했다.
  let horzOffset = 0;
  let vertOffset = 0;
  {
    // 드래그 영역 중심점의 화면 좌표
    const centerX = (drag.startClientX + drag.currentClientX) / 2;
    const centerY = (drag.startClientY + drag.currentClientY) / 2;
    // 화면 좌표 → 종이 좌표 (px, 줌 보정 전)
    const scrollContent = this.container.querySelector('#scroll-content');
    if (scrollContent) {
      const contentRect = scrollContent.getBoundingClientRect();
      const cX = centerX - contentRect.left;
      const cY = centerY - contentRect.top;
      const pageIdx = this.virtualScroll.getPageAtPoint(cX, cY);
      const pageOffset = this.virtualScroll.getPageOffset(pageIdx);
      const pageLeft = this.virtualScroll.getPageLeftResolved(pageIdx, scrollContent.clientWidth);
      // 종이 좌표 (px → HWPUNIT)
      const paperX = ((cX - pageLeft) / zoom) * 75;
      const paperY = ((cY - pageOffset) / zoom) * 75;
      // 도형 좌상단 = 중심점 - 반폭/반높이
      horzOffset = Math.max(0, Math.round(paperX - wHwp / 2));
      vertOffset = Math.max(0, Math.round(paperY - hHwp / 2));
    }
  }

  // 직선 방향 결정: 드래그 시작→끝의 X/Y 방향
  let lineFlipX = false;
  let lineFlipY = false;
  if ((this.shapePlacementType === 'line' || this.shapePlacementType.startsWith('connector-')) && drag.isDragging) {
    lineFlipX = drag.currentClientX < drag.startClientX;
    lineFlipY = drag.currentClientY < drag.startClientY;
  }

  // WASM 호출로 도형 생성
  try {
    // [Task #1280 v2] 삽입 글상자는 한컴 정답값 floating(treat_as_char=false) + 글앞으로
    //   (InFrontOfText)로 생성한다. 그래야 글상자 위 어울림(Square) 이미지가 글상자 뒤로 가고
    //   (plane 3>2), 로드된 기존 글상자(이미 floating)와도 정합한다.
    const isTextbox = this.shapePlacementType === 'textbox';
    const result = this.wasm.createShapeControl({
      sectionIdx: sec,
      paraIdx,
      charOffset,
      width: wHwp,
      height: hHwp,
      horzOffset,
      vertOffset,
      shapeType: this.shapePlacementType,
      lineFlipX,
      lineFlipY,
      ...(isTextbox ? { treatAsChar: false, textWrap: 'InFrontOfText' } : {}),
    });
    if (result.ok) {
      this.eventBus.emit('document-changed');
      // 생성된 도형을 선택 상태로 진입
      const selType = (this.shapePlacementType === 'line' || this.shapePlacementType.startsWith('connector-')) ? 'line' : 'shape';
      this.cursor.enterPictureObjectSelectionDirect(sec, result.paraIdx, result.controlIdx, selType);
      this.caret.hide();
      this.selectionRenderer.clear();
      this.renderPictureObjectSelection();
      this.eventBus.emit('picture-object-selection-changed', true);
    }
  } catch (err) {
    console.warn('[InputHandler] 글상자 삽입 실패:', err);
  }

  // 모드 종료
  this.textboxPlacementMode = false;
  this.textboxPlacementDrag = null;
  this.container.style.cursor = '';
}
