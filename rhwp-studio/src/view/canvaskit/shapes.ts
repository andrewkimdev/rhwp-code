/** canvaskit-renderer shape replay — 벡터 프리미티브·스타일 도형·페인트 헬퍼 무리를 CanvasKitLayerRenderer 에서 추출한 free-function 모듈. */
/* eslint-disable @typescript-eslint/no-explicit-any */

import type { Canvas, Color, Paint, Path, PathBuilder, Rect } from 'canvaskit-wasm';
import type {
  LayerBounds,
  LayerEllipseOp,
  LayerLineOp,
  LayerPageBackgroundOp,
  LayerPathCommand,
  LayerPathOp,
  LayerRectangleOp,
  LayerShapeStyle,
  LayerStrokeDash,
} from '@/core/types';
import { parseCssColor } from './colors';

type SkCanvas = Canvas;
type SkPaint = Paint;
type MutablePath = Path & Pick<PathBuilder, 'arcToRotated' | 'close' | 'cubicTo' | 'lineTo' | 'moveTo'>;

export function renderPageBackground(this: any, canvas: SkCanvas, op: LayerPageBackgroundOp): void {
  if (op.backgroundColor) {
    const paint = this.makeFillPaint(op.backgroundColor);
    canvas.drawRect(this.rect(op.bbox), paint);
    paint.delete?.();
  }
  if (op.borderColor && (op.borderWidth ?? 0) > 0) {
    const paint = this.makeStrokePaint(op.borderColor, op.borderWidth ?? 1);
    canvas.drawRect(this.rect(op.bbox), paint);
    paint.delete?.();
  }
}

export function renderRectangle(this: any, canvas: SkCanvas, op: LayerRectangleOp): void {
  this.drawStyledShape(canvas, op.bbox, op.style, (paint: SkPaint) => {
    const cornerRadius = op.cornerRadius ?? 0;
    if (cornerRadius > 0) {
      canvas.drawRRect(this.canvasKit.RRectXY(this.rect(op.bbox), cornerRadius, cornerRadius), paint);
    } else {
      canvas.drawRect(this.rect(op.bbox), paint);
    }
  });
}

export function renderEllipse(this: any, canvas: SkCanvas, op: LayerEllipseOp): void {
  this.drawStyledShape(canvas, op.bbox, op.style, (paint: SkPaint) => {
    canvas.drawOval(this.rect(op.bbox), paint);
  });
}

export function renderLine(this: any, canvas: SkCanvas, op: LayerLineOp): void {
  const paint = this.makeStrokePaint(op.style?.color ?? '#000000', op.style?.width ?? 1);
  try {
    this.drawStrokeWithDash(op.style?.dash, paint, () => {
      canvas.drawLine(op.x1, op.y1, op.x2, op.y2, paint);
    });
  } finally {
    paint.delete?.();
  }
}

export function renderPath(this: any, canvas: SkCanvas, op: LayerPathOp): void {
  const path = new this.canvasKit.Path() as MutablePath;
  let currentX = op.bbox.x;
  let currentY = op.bbox.y;
  for (const command of op.commands ?? []) {
    [currentX, currentY] = this.applyPathCommand(path, command, currentX, currentY);
  }
  const style: LayerShapeStyle = op.style ?? (op.lineStyle ? {} : {
    strokeColor: '#000000',
    strokeWidth: 1,
    fillColor: null,
  });
  const replayStyle: LayerShapeStyle = {
    ...style,
    strokeColor: style.strokeColor ?? op.lineStyle?.color,
    strokeWidth: op.lineStyle?.width ?? style.strokeWidth,
    strokeDash: op.lineStyle?.dash ?? style.strokeDash,
  };

  // [Task #1067] HWPX/HWP 도형의 회전 + flip 변환 적용.
  // Rust paint pipeline (src/paint/json.rs::write_transform) 이 emit 하는
  // {"rotation": <degrees>, "horzFlip": <bool>, "vertFlip": <bool>} 매핑.
  // renderTextRun (line 410-416) 패턴 정합.
  const tr = op.transform;
  const rotation = tr?.rotation ?? 0;
  const horzFlip = tr?.horzFlip ?? false;
  const vertFlip = tr?.vertFlip ?? false;
  const needsTransform = rotation !== 0 || horzFlip || vertFlip;
  if (needsTransform) {
    const cx = op.bbox.x + (op.bbox.width ?? 0) / 2;
    const cy = op.bbox.y + (op.bbox.height ?? 0) / 2;
    canvas.save();
    if (horzFlip || vertFlip) {
      canvas.translate(cx, cy);
      canvas.scale(horzFlip ? -1 : 1, vertFlip ? -1 : 1);
      canvas.translate(-cx, -cy);
    }
    if (rotation !== 0) {
      canvas.rotate(rotation, cx, cy);
    }
  }
  this.drawStyledPath(canvas, path, replayStyle);
  if (needsTransform) {
    canvas.restore();
  }
  path.delete?.();
}

export function applyPathCommand(this: any, path: MutablePath, command: LayerPathCommand, currentX: number, currentY: number): [number, number] {
  switch (command.type) {
    case 'moveTo':
      path.moveTo(command.x, command.y);
      return [command.x, command.y];
    case 'lineTo':
      path.lineTo(command.x, command.y);
      return [command.x, command.y];
    case 'curveTo':
      path.cubicTo(command.x1, command.y1, command.x2, command.y2, command.x3, command.y3);
      return [command.x3, command.y3];
    case 'arcTo':
      if (typeof path.arcToRotated === 'function') {
        path.arcToRotated(command.rx, command.ry, command.rotation, command.largeArc, command.sweep, command.x, command.y);
      } else {
        path.lineTo(command.x, command.y);
      }
      return [command.x, command.y];
    case 'closePath':
      path.close();
      return [currentX, currentY];
  }
}

export function drawStyledShape(
  this: any,
  canvas: SkCanvas,
  bounds: LayerBounds,
  style: LayerShapeStyle | undefined,
  draw: (paint: SkPaint) => void,
): void {
  if (style?.fillColor) {
    const paint = this.makeFillPaint(style.fillColor, style.opacity);
    draw(paint);
    paint.delete?.();
  }
  if (style?.strokeColor && (style.strokeWidth ?? 0) > 0) {
    const paint = this.makeStrokePaint(style.strokeColor, style.strokeWidth ?? 1, style.opacity);
    try {
      this.drawStrokeWithDash(style.strokeDash, paint, () => draw(paint));
    } finally {
      paint.delete?.();
    }
  }
  if (!style?.fillColor && !style?.strokeColor) {
    const paint = this.makeStrokePaint('#000000', 1);
    draw(paint);
    paint.delete?.();
  }
}

export function drawStyledPath(this: any, canvas: SkCanvas, path: Path, style: LayerShapeStyle): void {
  let drawn = false;
  if (style.fillColor) {
    const paint = this.makeFillPaint(style.fillColor, style.opacity);
    canvas.drawPath(path, paint);
    paint.delete?.();
    drawn = true;
  }
  if (style.strokeColor && (style.strokeWidth ?? 0) > 0) {
    const paint = this.makeStrokePaint(style.strokeColor, style.strokeWidth ?? 1, style.opacity);
    try {
      this.drawStrokeWithDash(style.strokeDash, paint, () => canvas.drawPath(path, paint));
    } finally {
      paint.delete?.();
    }
    drawn = true;
  }
  if (!drawn) {
    const paint = this.makeStrokePaint('#000000', 1);
    canvas.drawPath(path, paint);
    paint.delete?.();
  }
}

export function drawStrokeWithDash(
  this: any,
  dash: LayerStrokeDash | undefined,
  paint: SkPaint,
  draw: () => void,
): void {
  const intervals = dash === undefined || dash === 'solid'
    ? null
    : dash === 'dash'
      ? [6, 3]
      : dash === 'dot'
        ? [2, 2]
        : dash === 'dashDot'
          ? [6, 3, 2, 3]
          : dash === 'dashDotDot'
            ? [6, 3, 2, 3, 2, 3]
            : undefined;
  if (intervals === undefined) {
    this.unsupportedOps.add(`strokeDash:${String(dash)}`);
    return;
  }
  if (intervals === null) {
    draw();
    return;
  }

  const effect = this.canvasKit.PathEffect.MakeDash(intervals, 0);
  if (!effect) {
    this.unsupportedOps.add('strokeDash:pathEffectUnavailable');
    return;
  }
  try {
    paint.setPathEffect(effect);
    draw();
    this.currentReplayFeatureCounts.dashedStrokes += 1;
  } finally {
    effect.delete?.();
  }
}

export function makeFillPaint(this: any, color: string, opacity = 1): SkPaint {
  const paint = new this.canvasKit.Paint();
  paint.setAntiAlias?.(true);
  paint.setStyle(this.canvasKit.PaintStyle.Fill);
  paint.setColor(this.color(color, opacity));
  return paint;
}

export function makeStrokePaint(this: any, color: string, width: number, opacity = 1): SkPaint {
  const paint = new this.canvasKit.Paint();
  paint.setAntiAlias?.(true);
  paint.setStyle(this.canvasKit.PaintStyle.Stroke);
  paint.setStrokeWidth(Math.max(0.1, width));
  paint.setColor(this.color(color, opacity));
  return paint;
}

export function rect(this: any, bounds: LayerBounds): Rect {
  return this.canvasKit.XYWHRect(bounds.x, bounds.y, bounds.width, bounds.height);
}

export function color(this: any, cssColor: string, opacity = 1): Color {
  const { r, g, b, a } = parseCssColor(cssColor);
  const alpha = Math.max(0, Math.min(1, a * opacity));
  return this.canvasKit.Color(r, g, b, alpha);
}
