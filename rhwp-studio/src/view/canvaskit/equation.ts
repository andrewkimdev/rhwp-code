/** canvaskit-renderer equation replay — semantic equation layout 렌더링 무리를 CanvasKitLayerRenderer 에서 추출한 free-function 모듈. */
/* eslint-disable @typescript-eslint/no-explicit-any */

import type { Canvas, Font, Paint } from 'canvaskit-wasm';
import type { LayerEquationLayoutBox, LayerEquationOp } from '@/core/types';

type SkCanvas = Canvas;
type SkPaint = Paint;

const MAX_EQUATION_LAYOUT_DEPTH = 64;
const MAX_EQUATION_LAYOUT_NODES = 4096;
const MAX_EQUATION_TEXT_LENGTH = 4096;

export interface EquationRenderBudget {
  remainingNodes: number;
}

export function renderEquation(this: any, canvas: SkCanvas, op: LayerEquationOp): void {
  if (!op.layoutBox || !this.boundsAreDrawable(op.bbox)) {
    this.unsupportedOps.add('equation:unsupportedDirectReplay');
    return;
  }
  const scaleX = op.layoutBox.width > 0 && op.bbox.width > 0
    ? op.bbox.width / op.layoutBox.width
    : 1;
  const budget: EquationRenderBudget = {
    remainingNodes: MAX_EQUATION_LAYOUT_NODES,
  };
  const recorder = new this.canvasKit.PictureRecorder();
  let picture: ReturnType<typeof recorder.finishRecordingAsPicture> | null = null;
  let recordingFinished = false;
  let replayed = false;
  try {
    const recordingCanvas = recorder.beginRecording(this.rect(op.bbox));
    recordingCanvas.save();
    recordingCanvas.translate(op.bbox.x, op.bbox.y);
    if (Math.abs(scaleX - 1) > 0.01) recordingCanvas.scale(scaleX, 1);
    try {
      replayed = this.renderEquationBox(
        recordingCanvas,
        op.layoutBox,
        0,
        0,
        op.color ?? '#000000',
        Math.max(1, op.fontSize ?? op.bbox.height),
        false,
        false,
        0,
        budget,
      );
    } finally {
      recordingCanvas.restore();
    }
    picture = recorder.finishRecordingAsPicture();
    recordingFinished = true;
    if (replayed) canvas.drawPicture(picture);
  } catch {
    replayed = false;
  } finally {
    if (!recordingFinished) {
      try {
        picture = recorder.finishRecordingAsPicture();
      } catch {
        picture = null;
      }
    }
    picture?.delete?.();
    recorder.delete?.();
  }
  if (!replayed) {
    this.unsupportedOps.add('equation:invalidLayout');
  }
}

export function renderEquationBox(
  this: any,
  canvas: SkCanvas,
  layout: LayerEquationLayoutBox,
  parentX: number,
  parentY: number,
  color: string,
  fontSize: number,
  italic: boolean,
  bold: boolean,
  depth: number,
  budget: EquationRenderBudget,
): boolean {
  if (
    depth > MAX_EQUATION_LAYOUT_DEPTH
    || budget.remainingNodes <= 0
    || !this.equationBoxIsFinite(layout)
  ) {
    return false;
  }
  budget.remainingNodes -= 1;
  const x = parentX + layout.x;
  const y = parentY + layout.y;
  const child = (box: LayerEquationLayoutBox, size = fontSize, childItalic = italic, childBold = bold) => (
    this.renderEquationBox(canvas, box, x, y, color, size, childItalic, childBold, depth + 1, budget)
  );

  switch (layout.kind.type) {
    case 'row':
      return layout.kind.children.every((box) => child(box));
    case 'text':
    case 'number':
    case 'symbol':
    case 'mathSymbol':
      return this.drawEquationText(
        canvas,
        layout.kind.text,
        x,
        y + layout.baseline,
        this.equationFontSizeFromBox(layout, fontSize),
        color,
        layout.kind.type === 'text' || italic,
        bold,
        layout.width,
        layout.kind.type === 'symbol',
      );
    case 'function':
      return this.drawEquationText(
        canvas,
        layout.kind.name,
        x,
        y + layout.baseline,
        this.equationFontSizeFromBox(layout, fontSize),
        color,
        italic,
        bold,
        layout.width,
        false,
      );
    case 'fraction':
      return child(layout.kind.numer)
        && this.drawEquationLine(
          canvas,
          x + fontSize * 0.05,
          y + layout.baseline,
          x + layout.width - fontSize * 0.05,
          y + layout.baseline,
          color,
          fontSize * 0.04,
        )
        && child(layout.kind.denom);
    case 'atop':
      return child(layout.kind.top) && child(layout.kind.bottom);
    case 'sqrt': {
      const bodyLeft = x + layout.kind.body.x - fontSize * 0.1;
      const midX = bodyLeft - fontSize * 0.15;
      const midY = y + layout.height;
      const startX = midX - fontSize * 0.3;
      const startY = y + layout.height * 0.6;
      const tickX = startX - fontSize * 0.1;
      const tickY = startY - fontSize * 0.05;
      const linesDrawn = this.drawEquationLine(canvas, tickX, tickY, startX, startY, color, fontSize * 0.04)
        && this.drawEquationLine(canvas, startX, startY, midX, midY, color, fontSize * 0.04)
        && this.drawEquationLine(canvas, midX, midY, bodyLeft, y, color, fontSize * 0.04)
        && this.drawEquationLine(canvas, bodyLeft, y, x + layout.width, y, color, fontSize * 0.04);
      const indexDrawn = layout.kind.index
        ? child(layout.kind.index, fontSize * 0.7, false, false)
        : true;
      return linesDrawn && indexDrawn && child(layout.kind.body);
    }
    case 'superscript':
      return child(layout.kind.base)
        && child(layout.kind.sup, fontSize * 0.7);
    case 'subscript':
      return child(layout.kind.base)
        && child(layout.kind.sub, fontSize * 0.7);
    case 'subSup':
      return child(layout.kind.base)
        && child(layout.kind.sub, fontSize * 0.7)
        && child(layout.kind.sup, fontSize * 0.7);
    case 'bigOp': {
      const opSize = fontSize * 1.5;
      const supHeight = layout.kind.sup ? layout.kind.sup.height + fontSize * 0.05 : 0;
      const symbolDrawn = this.drawEquationText(
        canvas,
        layout.kind.symbol,
        x,
        y + supHeight + opSize * 0.8,
        opSize,
        color,
        false,
        false,
        layout.width,
        true,
      );
      const supDrawn = layout.kind.sup
        ? child(layout.kind.sup, fontSize * 0.7, false, false)
        : true;
      const subDrawn = layout.kind.sub
        ? child(layout.kind.sub, fontSize * 0.7, false, false)
        : true;
      return symbolDrawn && supDrawn && subDrawn;
    }
    case 'limit': {
      const size = this.equationFontSizeFromBox(layout, fontSize);
      const limitDrawn = this.drawEquationText(
        canvas,
        layout.kind.isUpper ? 'Lim' : 'lim',
        x,
        y + size * 0.8,
        size,
        color,
        false,
        false,
        layout.width,
        false,
      );
      return limitDrawn && (layout.kind.sub
        ? child(layout.kind.sub, fontSize * 0.7, false, false)
        : true);
    }
    case 'matrix': {
      let rendered = true;
      if (layout.kind.style !== 'plain') {
        const brackets = layout.kind.style === 'paren'
          ? ['(', ')']
          : layout.kind.style === 'bracket'
            ? ['[', ']']
            : ['|', '|'];
        rendered = this.drawEquationBracket(canvas, brackets[0], x, y, layout.height, color, fontSize)
          && this.drawEquationBracket(canvas, brackets[1], x + layout.width, y, layout.height, color, fontSize);
      }
      for (const row of layout.kind.cells) {
        for (const cell of row) rendered = child(cell) && rendered;
      }
      return rendered;
    }
    case 'rel':
      return child(layout.kind.over)
        && child(layout.kind.arrow)
        && (layout.kind.under ? child(layout.kind.under) : true);
    case 'eqAlign':
      return layout.kind.rows.every((row) => child(row.left) && child(row.right));
    case 'paren':
      return (layout.kind.left
        ? this.drawEquationBracket(canvas, layout.kind.left, x, y, layout.height, color, fontSize)
        : true)
        && child(layout.kind.body)
        && (layout.kind.right
          ? this.drawEquationBracket(canvas, layout.kind.right, x + layout.width, y, layout.height, color, fontSize)
          : true);
    case 'decoration':
      return child(layout.kind.body)
        && this.drawEquationDecoration(
          canvas,
          layout.kind.decoration,
          x + layout.kind.body.x + layout.kind.body.width / 2,
          y + fontSize * 0.05,
          layout.kind.body.width,
          color,
          fontSize,
        );
    case 'fontStyle': {
      if (!['roman', 'italic', 'bold'].includes(layout.kind.fontStyle)) return false;
      const nextItalic = layout.kind.fontStyle === 'roman'
        ? false
        : layout.kind.fontStyle === 'italic'
          || layout.kind.fontStyle === 'calligraphy'
          || layout.kind.fontStyle === 'fraktur'
          || italic;
      const nextBold = layout.kind.fontStyle === 'roman'
        ? false
        : layout.kind.fontStyle === 'bold'
          || layout.kind.fontStyle === 'blackboard'
          || bold;
      return child(layout.kind.body, fontSize, nextItalic, nextBold);
    }
    case 'space':
    case 'newline':
    case 'empty':
      return true;
  }
}

export function equationBoxIsFinite(this: any, layout: LayerEquationLayoutBox): boolean {
  return Number.isFinite(layout.x)
    && Number.isFinite(layout.y)
    && Number.isFinite(layout.width)
    && Number.isFinite(layout.height)
    && Number.isFinite(layout.baseline)
    && layout.width >= 0
    && layout.height >= 0;
}

export function equationFontSizeFromBox(this: any, layout: LayerEquationLayoutBox, baseFontSize: number): number {
  return Math.max(1, layout.height > 0 ? layout.height : baseFontSize);
}

export function drawEquationText(
  this: any,
  canvas: SkCanvas,
  text: string,
  x: number,
  baselineY: number,
  fontSize: number,
  color: string,
  italic: boolean,
  bold: boolean,
  targetWidth: number,
  centered: boolean,
): boolean {
  if (
    !text
    || text.length > MAX_EQUATION_TEXT_LENGTH
    || ![x, baselineY, fontSize, targetWidth].every(Number.isFinite)
  ) {
    return false;
  }
  let font: Font | null = null;
  let paint: SkPaint | null = null;
  try {
    font = new this.canvasKit.Font(this.defaultTypeface, Math.max(1, fontSize)) as Font;
    paint = this.makeFillPaint(color) as SkPaint;
    const glyphIds = font.getGlyphIDs(text, Array.from(text).length);
    if (!glyphIds || glyphIds.some((glyphId) => glyphId === 0)) return false;
    const glyphWidths = font.getGlyphWidths(glyphIds) ?? [];
    const measuredWidth = glyphWidths.reduce((sum, width) => sum + width, 0);
    const drawWidth = targetWidth > 0 && measuredWidth > 0 ? targetWidth : measuredWidth;
    if (targetWidth > 0 && measuredWidth > 0) {
      font.setScaleX(targetWidth / measuredWidth);
    }
    const adjustableFont = font as Font & {
      setEmbolden?: (enabled: boolean) => void;
      setSkewX?: (skew: number) => void;
    };
    adjustableFont.setEmbolden?.(bold);
    adjustableFont.setSkewX?.(italic ? -0.2 : 0);
    canvas.drawText(text, centered ? x + (targetWidth - drawWidth) / 2 : x, baselineY, paint, font);
    return true;
  } finally {
    font?.delete?.();
    paint?.delete?.();
  }
}

export function drawEquationLine(
  this: any,
  canvas: SkCanvas,
  x1: number,
  y1: number,
  x2: number,
  y2: number,
  color: string,
  width: number,
): boolean {
  if (![x1, y1, x2, y2, width].every(Number.isFinite)) return false;
  const paint = this.makeStrokePaint(color, Math.max(0.5, width));
  try {
    canvas.drawLine(x1, y1, x2, y2, paint);
    return true;
  } finally {
    paint.delete?.();
  }
}

export function drawEquationBracket(
  this: any,
  canvas: SkCanvas,
  bracket: string,
  x: number,
  y: number,
  height: number,
  color: string,
  fontSize: number,
): boolean {
  const width = Math.max(fontSize * 0.3, 1);
  if (bracket === '|') {
    return this.drawEquationLine(canvas, x, y, x, y + height, color, fontSize * 0.04);
  }
  return this.drawEquationText(
    canvas,
    bracket,
    x - width / 2,
    y + height * 0.7,
    Math.max(height, fontSize),
    color,
    false,
    false,
    width,
    true,
  );
}

export function drawEquationDecoration(
  this: any,
  canvas: SkCanvas,
  decoration: string,
  centerX: number,
  y: number,
  width: number,
  color: string,
  fontSize: number,
): boolean {
  const halfWidth = width / 2;
  const strokeWidth = Math.max(fontSize * 0.03, 0.5);
  switch (decoration) {
    case 'hat':
      return this.drawEquationLine(canvas, centerX - halfWidth * 0.6, y + fontSize * 0.15, centerX, y, color, strokeWidth)
        && this.drawEquationLine(canvas, centerX, y, centerX + halfWidth * 0.6, y + fontSize * 0.15, color, strokeWidth);
    case 'bar':
    case 'overline':
    case 'strikeThrough':
      return this.drawEquationLine(canvas, centerX - halfWidth, y + fontSize * 0.05, centerX + halfWidth, y + fontSize * 0.05, color, strokeWidth);
    case 'underline':
    case 'under':
      return this.drawEquationLine(canvas, centerX - halfWidth, y + fontSize * 1.1, centerX + halfWidth, y + fontSize * 1.1, color, strokeWidth);
    case 'vec':
    case 'dyad': {
      const lineY = y + fontSize * 0.05;
      const endX = centerX + halfWidth;
      return this.drawEquationLine(canvas, centerX - halfWidth, lineY, endX, lineY, color, strokeWidth)
        && this.drawEquationLine(canvas, endX - fontSize * 0.1, lineY - fontSize * 0.06, endX, lineY, color, strokeWidth)
        && this.drawEquationLine(canvas, endX, lineY, endX - fontSize * 0.1, lineY + fontSize * 0.06, color, strokeWidth);
    }
    case 'dot':
    case 'dDot': {
      const paint = this.makeFillPaint(color);
      const radius = Math.max(fontSize * 0.03, 1);
      try {
        if (decoration === 'dot') {
          canvas.drawCircle(centerX, y + fontSize * 0.06, radius, paint);
        } else {
          canvas.drawCircle(centerX - fontSize * 0.1, y + fontSize * 0.06, radius, paint);
          canvas.drawCircle(centerX + fontSize * 0.1, y + fontSize * 0.06, radius, paint);
        }
        return true;
      } finally {
        paint.delete?.();
      }
    }
    default:
      return false;
  }
}
