/** canvaskit-renderer form-object replay — 양식 객체(formObject·placeholder) 렌더링 무리를 CanvasKitLayerRenderer 에서 추출한 free-function 모듈. */
/* eslint-disable @typescript-eslint/no-explicit-any */

import type { Canvas, Paint } from 'canvaskit-wasm';
import type {
  LayerFormObjectOp,
  LayerPlaceholderOp,
  LayerRenderProfile,
} from '@/core/types';

type SkCanvas = Canvas;
type SkPaint = Paint;

const MAX_PLACEHOLDER_DASH_SEGMENTS_PER_AXIS = 2048;

export function renderFormObject(this: any, canvas: SkCanvas, op: LayerFormObjectOp): void {
  const fill = op.backColor && op.backColor !== '#000000' ? op.backColor : '#f7f7f7';
  this.drawStyledShape(canvas, op.bbox, {
    fillColor: fill,
    strokeColor: op.foreColor ?? '#555555',
    strokeWidth: 1,
    opacity: op.enabled === false ? 0.55 : 1,
  }, (paint: SkPaint) => canvas.drawRect(this.rect(op.bbox), paint));
  if (op.value && (
    op.formType === 'checkBox'
    || op.formType === 'radioButton'
    || op.formType === 'checkbox'
    || op.formType === 'radio'
  )) {
    const paint = this.makeStrokePaint(op.foreColor ?? '#111111', 1.5);
    const b = op.bbox;
    canvas.drawLine(b.x + b.width * 0.25, b.y + b.height * 0.55, b.x + b.width * 0.45, b.y + b.height * 0.75, paint);
    canvas.drawLine(b.x + b.width * 0.45, b.y + b.height * 0.75, b.x + b.width * 0.78, b.y + b.height * 0.28, paint);
    paint.delete?.();
  }
  const label = op.caption || op.text;
  if (label) {
    this.renderTextRun(canvas, {
      type: 'textRun',
      bbox: { ...op.bbox, x: op.bbox.x + 4, width: Math.max(0, op.bbox.width - 8) },
      text: label,
      baseline: Math.max(10, op.bbox.height * 0.68),
      style: { fontSize: Math.max(9, Math.min(14, op.bbox.height * 0.55)), color: op.foreColor ?? '#111111' },
    });
  }
}

export function renderPlaceholder(this: any, canvas: SkCanvas, op: LayerPlaceholderOp, profile: LayerRenderProfile): void {
  if (op.kind === 'missingPicture') {
    if (profile === 'print' || profile === 'highQuality') return;
    if (![op.bbox.x, op.bbox.y, op.bbox.width, op.bbox.height].every(Number.isFinite)
      || op.bbox.width <= 0 || op.bbox.height <= 0) return;
    const paint = this.makeStrokePaint(op.strokeColor ?? '#999999', 1);
    const dash = 5;
    const gap = 3;
    const horizontalStep = Math.max(
      dash + gap,
      op.bbox.width / MAX_PLACEHOLDER_DASH_SEGMENTS_PER_AXIS,
    );
    const verticalStep = Math.max(
      dash + gap,
      op.bbox.height / MAX_PLACEHOLDER_DASH_SEGMENTS_PER_AXIS,
    );
    try {
      for (let x = op.bbox.x; x < op.bbox.x + op.bbox.width; x += horizontalStep) {
        const end = Math.min(x + horizontalStep * dash / (dash + gap), op.bbox.x + op.bbox.width);
        canvas.drawLine(x, op.bbox.y, end, op.bbox.y, paint);
        canvas.drawLine(x, op.bbox.y + op.bbox.height, end, op.bbox.y + op.bbox.height, paint);
      }
      for (let y = op.bbox.y; y < op.bbox.y + op.bbox.height; y += verticalStep) {
        const end = Math.min(y + verticalStep * dash / (dash + gap), op.bbox.y + op.bbox.height);
        canvas.drawLine(op.bbox.x, y, op.bbox.x, end, paint);
        canvas.drawLine(op.bbox.x + op.bbox.width, y, op.bbox.x + op.bbox.width, end, paint);
      }
    } finally {
      paint.delete?.();
    }
    const icon = Math.max(14, Math.min(36, Math.min(op.bbox.width, op.bbox.height) * 0.4));
    const ix = op.bbox.x + (op.bbox.width - icon) / 2;
    const iy = op.bbox.y + (op.bbox.height - icon * 0.75) / 2;
    const iconBounds = this.canvasKit.XYWHRect(ix, iy, icon, icon * 0.75);
    let iconFill: SkPaint | null = null;
    let iconStroke: SkPaint | null = null;
    let missingStroke: SkPaint | null = null;
    try {
      iconFill = this.makeFillPaint('#ffffff') as SkPaint;
      iconStroke = this.makeStrokePaint('#888888', 1) as SkPaint;
      missingStroke = this.makeStrokePaint('#cc4444', 1.5) as SkPaint;
      canvas.drawRect(iconBounds, iconFill);
      canvas.drawRect(iconBounds, iconStroke);
      canvas.drawLine(ix + icon * 0.08, iy + icon * 0.62, ix + icon * 0.32, iy + icon * 0.30, iconStroke);
      canvas.drawLine(ix + icon * 0.32, iy + icon * 0.30, ix + icon * 0.52, iy + icon * 0.62, iconStroke);
      canvas.drawLine(ix + icon * 0.52, iy + icon * 0.62, ix + icon * 0.68, iy + icon * 0.42, iconStroke);
      canvas.drawLine(ix + icon * 0.68, iy + icon * 0.42, ix + icon * 0.92, iy + icon * 0.62, iconStroke);
      canvas.drawCircle(ix + icon * 0.72, iy + icon * 0.20, icon * 0.07, iconStroke);
      canvas.drawLine(ix, iy + icon * 0.75, ix + icon, iy, missingStroke);
    } finally {
      missingStroke?.delete?.();
      iconStroke?.delete?.();
      iconFill?.delete?.();
    }
    return;
  }
  this.drawStyledShape(canvas, op.bbox, {
    fillColor: op.fillColor ?? '#f2f2f2',
    strokeColor: op.strokeColor ?? '#999999',
    strokeWidth: 1,
  }, (paint: SkPaint) => canvas.drawRect(this.rect(op.bbox), paint));
  if (op.label) {
    this.renderTextRun(canvas, {
      type: 'textRun',
      bbox: { ...op.bbox, x: op.bbox.x + 4 },
      text: op.label,
      baseline: Math.max(10, op.bbox.height * 0.65),
      style: { fontSize: Math.max(9, Math.min(14, op.bbox.height * 0.45)), color: '#555555' },
    });
  }
}
