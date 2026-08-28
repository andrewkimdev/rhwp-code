/** canvaskit-renderer text-decoration replay — 텍스트 장식(charOverlap·textControlMark·tabLeader·textDecoration) 렌더링 무리를 CanvasKitLayerRenderer 에서 추출한 free-function 모듈. */
/* eslint-disable @typescript-eslint/no-explicit-any */

import type { Canvas, Font, Paint } from 'canvaskit-wasm';
import type {
  LayerCharOverlapOp,
  LayerTabLeaderOp,
  LayerTextControlMarkOp,
  LayerTextDecorationOp,
} from '@/core/types';
import { primaryFontFamily } from './text-run';

type SkCanvas = Canvas;
type SkPaint = Paint;

const MAX_TEXT_SPECIAL_VISUAL_ITEMS = 4096;

export function renderCharOverlap(this: any, canvas: SkCanvas, op: LayerCharOverlapOp): void {
  if (typeof op.text !== 'string' || !op.charOverlap || !Array.isArray(op.positions)) {
    this.unsupportedOps.add('charOverlap:invalidGeometry');
    return;
  }
  if (op.positionsComplete !== true
    || op.positions.length > MAX_TEXT_SPECIAL_VISUAL_ITEMS + 1) {
    this.unsupportedOps.add('charOverlap:visualItemLimitExceeded');
    return;
  }
  const chars: string[] = [];
  for (const ch of op.text) {
    if (chars.length >= MAX_TEXT_SPECIAL_VISUAL_ITEMS) {
      this.unsupportedOps.add('charOverlap:visualItemLimitExceeded');
      return;
    }
    chars.push(ch);
  }
  if (op.positions.length !== chars.length + 1) {
    this.unsupportedOps.add('charOverlap:invalidGeometry');
    return;
  }
  if (chars.length === 0) return;
  if (op.isVertical) {
    this.unsupportedOps.add('textRun:verticalText');
    return;
  }
  const style = op.style ?? {};
  const rawFontSize = style.fontSize ?? (op.bbox.height || 12);
  if (![op.baseline, op.rotation, rawFontSize, op.charOverlap.innerCharSize]
    .every(Number.isFinite)
    || op.positions.some(position => !Number.isFinite(position))
    || rawFontSize <= 0
    || !Number.isInteger(op.charOverlap.borderType)
    || op.charOverlap.borderType < 0
    || op.charOverlap.borderType > 4
    || !Number.isInteger(op.charOverlap.innerCharSize)
    || op.charOverlap.innerCharSize < -128
    || op.charOverlap.innerCharSize > 127) {
    this.unsupportedOps.add('charOverlap:invalidGeometry');
    return;
  }
  const fontSize = Math.max(1, rawFontSize);
  const rawRatio = op.charOverlap.innerCharSize > 0
    ? op.charOverlap.innerCharSize / 100
    : op.charOverlap.innerCharSize < 0
      ? 1 + op.charOverlap.innerCharSize * 0.1
      : 1;
  const innerFontSize = Math.max(1, fontSize * Math.min(4, Math.max(0.1, rawRatio)));
  const requestedFontFamily = primaryFontFamily(style.fontFamily);
  const preparedTypeface = this.findPreparedTypeface(requestedFontFamily);
  if (requestedFontFamily && !preparedTypeface && this.requirePreparedFontFamilies) {
    throw new Error(`CanvasKit font family가 준비되지 않았습니다: ${requestedFontFamily}`);
  }
  const primaryTypeface = preparedTypeface?.typeface ?? this.defaultTypeface;

  const overlapDigits: Array<[number, number]> = [];
  for (const ch of chars) {
    const codePoint = ch.codePointAt(0) ?? 0;
    const digit = codePoint >= 0xF0289 && codePoint <= 0xF0291
      ? [0, codePoint - 0xF0288] as [number, number]
      : codePoint >= 0xF0292 && codePoint <= 0xF029B
        ? [1, codePoint - 0xF0292] as [number, number]
        : codePoint >= 0xF0491 && codePoint <= 0xF0499
          ? [0, codePoint - 0xF0490] as [number, number]
          : codePoint >= 0xF049A && codePoint <= 0xF04A3
            ? [1, codePoint - 0xF049A] as [number, number]
            : codePoint >= 0xF04A4 && codePoint <= 0xF04AD
              ? [2, codePoint - 0xF04A4] as [number, number]
              : null;
    if (!digit) {
      overlapDigits.length = 0;
      break;
    }
    overlapDigits.push(digit);
  }
  const decodedNumber = overlapDigits.length === chars.length
    ? overlapDigits
        .sort(([left], [right]) => left - right)
        .map(([, digit]) => String.fromCharCode(0x30 + digit))
        .join('')
    : null;

  const draw = (originX: number, originY: number) => {
    const boxSize = fontSize;
    const centerY = originY + op.bbox.height - boxSize / 2;
    const drawCell = (
      displayText: string,
      centerX: number,
      drawShape: boolean,
      horizontalScale?: number,
    ) => {
      const borderType = horizontalScale !== undefined && op.charOverlap.borderType === 0
        ? 1
        : op.charOverlap.borderType;
      const reversed = borderType === 2 || borderType === 4;
      const circle = borderType === 1 || borderType === 2;
      const rectangle = borderType === 3 || borderType === 4;
      const textColor = reversed ? '#ffffff' : style.color ?? '#000000';
      if (drawShape && (circle || rectangle)) {
        let fill: SkPaint | null = null;
        let stroke: SkPaint | null = null;
        try {
          fill = reversed ? this.makeFillPaint('#000000') : null;
          stroke = this.makeStrokePaint(
            reversed ? '#000000' : style.color ?? '#000000',
            0.8,
          ) as SkPaint;
          if (circle) {
            const radiusY = boxSize / 2;
            const radiusX = radiusY * 0.85;
            const oval = this.canvasKit.XYWHRect(
              centerX - radiusX,
              centerY - radiusY,
              radiusX * 2,
              radiusY * 2,
            );
            if (fill) canvas.drawOval(oval, fill);
            canvas.drawOval(oval, stroke);
          } else {
            const rect = this.canvasKit.XYWHRect(
              centerX - boxSize / 2,
              centerY - boxSize / 2,
              boxSize,
              boxSize,
            );
            if (fill) canvas.drawRect(rect, fill);
            canvas.drawRect(rect, stroke);
          }
        } finally {
          fill?.delete?.();
          stroke?.delete?.();
        }
      }

      let textFont = new this.canvasKit.Font(primaryTypeface, innerFontSize) as Font;
      let fallbackCandidate: Font | null = null;
      let paint: SkPaint | null = null;
      const adjustFont = (target: Font) => {
        const adjustable = target as Font & {
          setEmbolden?: (enabled: boolean) => void;
          setSkewX?: (skew: number) => void;
        };
        adjustable.setEmbolden?.(style.bold === true);
        adjustable.setSkewX?.(style.italic === true ? -0.2 : 0);
      };
      try {
        adjustFont(textFont);
        let glyphIds = textFont.getGlyphIDs(displayText, Array.from(displayText).length);
        if (glyphIds.some(glyphId => glyphId === 0)) {
          for (const fallbackTypeface of [this.defaultTypeface, this.symbolFallbackTypeface]) {
            if (!fallbackTypeface || fallbackTypeface === primaryTypeface) continue;
            fallbackCandidate = new this.canvasKit.Font(fallbackTypeface, innerFontSize) as Font;
            adjustFont(fallbackCandidate);
            const fallbackGlyphIds = fallbackCandidate.getGlyphIDs(
              displayText,
              Array.from(displayText).length,
            );
            if (fallbackGlyphIds.every(glyphId => glyphId !== 0)) {
              textFont.delete?.();
              textFont = fallbackCandidate;
              fallbackCandidate = null;
              glyphIds = fallbackGlyphIds;
              break;
            }
            fallbackCandidate.delete?.();
            fallbackCandidate = null;
          }
        }
        if (glyphIds.some(glyphId => glyphId === 0)) {
          this.unsupportedOps.add('textRun:glyphMapping');
        }
        const widths = textFont.getGlyphWidths(glyphIds) ?? [];
        const measuredWidth = widths.reduce((sum, width) => sum + width, 0);
        const scaleX = horizontalScale ?? 1;
        if (scaleX < 1) {
          textFont.setScaleX(scaleX);
        }
        const drawWidth = measuredWidth * scaleX;
        paint = this.makeFillPaint(textColor) as SkPaint;
        const textY = (horizontalScale !== undefined ? centerY - fontSize * 0.08 : centerY)
          + innerFontSize * 0.35;
        canvas.drawText(displayText, centerX - Math.max(drawWidth, 1) / 2, textY, paint, textFont);
      } finally {
        paint?.delete?.();
        fallbackCandidate?.delete?.();
        textFont.delete?.();
      }
    };

    if (decodedNumber !== null) {
      const horizontalScale = decodedNumber.length > 1
        ? 0.7 / decodedNumber.length * 2
        : 1;
      drawCell(decodedNumber, originX + boxSize / 2, true, horizontalScale);
      return;
    }
    const centerX = chars.length > 1 ? originX + op.bbox.width / 2 : originX + boxSize / 2;
    chars.forEach((ch, index) => {
      const codePoint = ch.codePointAt(0) ?? 0;
      const displayText = codePoint >= 0x2460 && codePoint <= 0x2473
        ? String(codePoint - 0x2460 + 1)
        : codePoint >= 0xF02CE && codePoint <= 0xF02E1
          ? String(codePoint - 0xF02CD)
        : codePoint === 0xF012B
          ? '(인)'
          : codePoint === 0xF031C
            ? '■'
            : codePoint === 0xF02FC
              ? '►'
              : codePoint === 0xF03C5
                ? '□'
            : ch;
      drawCell(displayText, centerX, index === 0);
    });
  };

  this.withHorizontalTextVisualOrigin(canvas, op.bbox, op.rotation ?? 0, 'charOverlap', draw);
}

export function renderTextControlMark(this: any, canvas: SkCanvas, op: LayerTextControlMarkOp): void {
  if (op.isVertical) {
    this.unsupportedOps.add('textRun:verticalText');
    return;
  }
  if (!Array.isArray(op.marks)) {
    this.unsupportedOps.add('textControlMark:invalidGeometry');
    return;
  }
  if (op.marksComplete !== true
    || op.marks.length > MAX_TEXT_SPECIAL_VISUAL_ITEMS) {
    this.unsupportedOps.add('textControlMark:visualItemLimitExceeded');
    return;
  }
  if (![op.baseline, op.rotation].every(Number.isFinite)
    || op.marks.some(mark => !['space', 'tab', 'paragraphEnd', 'lineBreakEnd'].includes(mark.kind)
      || ![mark.x, mark.y, mark.fontSize].every(Number.isFinite)
      || mark.fontSize <= 0)) {
    this.unsupportedOps.add('textControlMark:invalidGeometry');
    return;
  }
  if (!this.currentShowParagraphMarks && !this.currentShowControlCodes) return;
  const draw = (originX: number, originY: number) => {
    const baselineY = originY + op.baseline;
    let paint: SkPaint | null = null;
    try {
      paint = this.makeStrokePaint('#0066ff', 0.75) as SkPaint;
      for (const mark of op.marks) {
        const x = originX + mark.x;
        const y = baselineY + mark.y;
        const size = mark.fontSize;
        if (mark.kind === 'space') {
          canvas.drawLine(x, y - size * 0.45, x + size * 0.25, y - size * 0.15, paint);
          canvas.drawLine(x + size * 0.25, y - size * 0.15, x + size * 0.5, y - size * 0.45, paint);
        } else if (mark.kind === 'tab') {
          const lineY = y - size * 0.3;
          const tipX = x + size * 0.85;
          canvas.drawLine(x, lineY, tipX, lineY, paint);
          canvas.drawLine(tipX, lineY, tipX - size * 0.25, lineY - size * 0.2, paint);
          canvas.drawLine(tipX, lineY, tipX - size * 0.25, lineY + size * 0.2, paint);
        } else if (mark.kind === 'paragraphEnd') {
          const topY = y - size * 0.8;
          const turnX = x + size * 0.4;
          const arrowY = y - size * 0.25;
          canvas.drawLine(turnX, topY, turnX, arrowY, paint);
          canvas.drawLine(turnX, arrowY, x, arrowY, paint);
          canvas.drawLine(x, arrowY, x + size * 0.2, arrowY - size * 0.18, paint);
          canvas.drawLine(x, arrowY, x + size * 0.2, arrowY + size * 0.18, paint);
        } else {
          const lineX = x + size * 0.25;
          const tipY = y - size * 0.1;
          canvas.drawLine(lineX, y - size * 0.85, lineX, tipY, paint);
          canvas.drawLine(lineX, tipY, lineX - size * 0.18, tipY - size * 0.22, paint);
          canvas.drawLine(lineX, tipY, lineX + size * 0.18, tipY - size * 0.22, paint);
        }
      }
    } finally {
      paint?.delete?.();
    }
  };

  this.withHorizontalTextVisualOrigin(canvas, op.bbox, op.rotation, 'textControlMark', draw);
}

export function renderTabLeader(this: any, canvas: SkCanvas, op: LayerTabLeaderOp): void {
  if (op.isVertical) {
    this.unsupportedOps.add('textRun:verticalText');
    return;
  }
  if (!Array.isArray(op.leaders)) {
    this.unsupportedOps.add('tabLeader:invalidGeometry');
    return;
  }
  if (op.leadersComplete !== true
    || op.leaders.length > MAX_TEXT_SPECIAL_VISUAL_ITEMS) {
    this.unsupportedOps.add('tabLeader:visualItemLimitExceeded');
    return;
  }
  if (![op.baseline, op.fontSize, op.rotation].every(Number.isFinite)
    || op.fontSize <= 0
    || op.leaders.some(leader => ![leader.startX, leader.endX].every(Number.isFinite)
      || leader.endX < leader.startX
      || !Number.isInteger(leader.fillType)
      || leader.fillType < 0
      || leader.fillType > 11)) {
    this.unsupportedOps.add('tabLeader:invalidGeometry');
    return;
  }
  const draw = (originX: number, originY: number) => {
    const baselineY = originY + op.baseline;
    for (const leader of op.leaders) {
      if (leader.fillType === 0 || leader.endX <= leader.startX) continue;
      const x1 = originX + leader.startX;
      const x2 = originX + leader.endX;
      const y = baselineY - op.fontSize * 0.35;
      switch (leader.fillType) {
        case 1:
          this.drawTextVisualStroke(canvas, x1, y, x2, y, op.color, 0.5);
          break;
        case 2:
          this.drawTextVisualStroke(canvas, x1, y, x2, y, op.color, 0.5, [3, 3]);
          break;
        case 3:
          this.drawTextVisualStroke(canvas, x1, y, x2, y, op.color, 1, [0.1, 3], true);
          break;
        case 4:
          this.drawTextVisualStroke(canvas, x1, y, x2, y, op.color, 0.5, [6, 2, 1, 2]);
          break;
        case 5:
          this.drawTextVisualStroke(canvas, x1, y, x2, y, op.color, 0.5, [6, 2, 1, 2, 1, 2]);
          break;
        case 6:
          this.drawTextVisualStroke(canvas, x1, y, x2, y, op.color, 0.5, [8, 4]);
          break;
        case 7:
          this.drawTextVisualStroke(canvas, x1, y, x2, y, op.color, 0.7, [0.1, 2.5], true);
          break;
        case 8:
          this.drawTextVisualStroke(canvas, x1, y - 1, x2, y - 1, op.color, 0.3);
          this.drawTextVisualStroke(canvas, x1, y + 1, x2, y + 1, op.color, 0.3);
          break;
        case 9:
          this.drawTextVisualStroke(canvas, x1, y - 1.2, x2, y - 1.2, op.color, 0.3);
          this.drawTextVisualStroke(canvas, x1, y + 0.8, x2, y + 0.8, op.color, 0.8);
          break;
        case 10:
          this.drawTextVisualStroke(canvas, x1, y - 0.8, x2, y - 0.8, op.color, 0.8);
          this.drawTextVisualStroke(canvas, x1, y + 1.2, x2, y + 1.2, op.color, 0.3);
          break;
        case 11:
          this.drawTextVisualStroke(canvas, x1, y - 2, x2, y - 2, op.color, 0.3);
          this.drawTextVisualStroke(canvas, x1, y, x2, y, op.color, 0.8);
          this.drawTextVisualStroke(canvas, x1, y + 2, x2, y + 2, op.color, 0.3);
          break;
      }
    }
  };

  this.withHorizontalTextVisualOrigin(canvas, op.bbox, op.rotation, 'tabLeader', draw);
}

export function renderTextDecoration(this: any, canvas: SkCanvas, op: LayerTextDecorationOp): void {
  const decoration = op.decoration;
  if (!decoration
    || !Array.isArray(decoration.positions)
    || !['underline', 'strikethrough', 'emphasisDot'].includes(decoration.kind)) {
    this.unsupportedOps.add('textDecoration:invalidGeometry');
    return;
  }
  if (decoration.isVertical) {
    this.unsupportedOps.add('textRun:verticalText');
    return;
  }
  if (decoration.positionsComplete !== true
    || decoration.positions.length > MAX_TEXT_SPECIAL_VISUAL_ITEMS + 1) {
    this.unsupportedOps.add('textDecoration:visualItemLimitExceeded');
    return;
  }
  if (![decoration.baseline, decoration.rotation, decoration.fontSize, decoration.ratio]
    .every(Number.isFinite)
    || decoration.fontSize <= 0
    || decoration.ratio <= 0
    || decoration.positions.some(position => !Number.isFinite(position))
    || !Number.isInteger(decoration.shape)
    || decoration.shape < 0
    || decoration.shape > 12
    || !Number.isInteger(decoration.emphasisDot)
    || decoration.emphasisDot < 0
    || decoration.emphasisDot > 6
    || !['none', 'bottom', 'top'].includes(decoration.underline)) {
    this.unsupportedOps.add('textDecoration:invalidGeometry');
    return;
  }
  const textWidth = decoration.positions.at(-1) ?? 0;
  if (!Number.isFinite(textWidth) || textWidth < 0) {
    this.unsupportedOps.add('textDecoration:invalidGeometry');
    return;
  }
  const drawLineShape = (originX: number, y: number) => {
    const x2 = originX + textWidth;
    switch (decoration.shape) {
      case 7:
        this.drawTextVisualStroke(canvas, originX, y - 1, x2, y - 1, decoration.color, 0.7);
        this.drawTextVisualStroke(canvas, originX, y + 1, x2, y + 1, decoration.color, 0.7);
        break;
      case 8:
        this.drawTextVisualStroke(canvas, originX, y - 1.2, x2, y - 1.2, decoration.color, 0.5);
        this.drawTextVisualStroke(canvas, originX, y + 0.8, x2, y + 0.8, decoration.color, 1.2);
        break;
      case 9:
        this.drawTextVisualStroke(canvas, originX, y - 0.8, x2, y - 0.8, decoration.color, 1.2);
        this.drawTextVisualStroke(canvas, originX, y + 1.2, x2, y + 1.2, decoration.color, 0.5);
        break;
      case 10:
        this.drawTextVisualStroke(canvas, originX, y - 1.5, x2, y - 1.5, decoration.color, 0.5);
        this.drawTextVisualStroke(canvas, originX, y, x2, y, decoration.color, 0.5);
        this.drawTextVisualStroke(canvas, originX, y + 1.5, x2, y + 1.5, decoration.color, 0.5);
        break;
      case 11:
        this.drawTextVisualStroke(canvas, originX, y, x2, y, decoration.color, 0.7, [], false, 1.5, 6);
        break;
      case 12:
        this.drawTextVisualStroke(canvas, originX, y - 1, x2, y - 1, decoration.color, 0.5, [], false, 1.2, 6);
        this.drawTextVisualStroke(canvas, originX, y + 1, x2, y + 1, decoration.color, 0.5, [], false, 1.2, 6);
        break;
      default: {
        const dash = decoration.shape === 1 ? [3, 3]
          : decoration.shape === 2 ? [1, 2]
            : decoration.shape === 3 ? [6, 2, 1, 2]
              : decoration.shape === 4 ? [6, 2, 1, 2, 1, 2]
                : decoration.shape === 5 ? [8, 4]
                  : decoration.shape === 6 ? [0.1, 2.5]
                    : [];
        this.drawTextVisualStroke(
          canvas,
          originX,
          y,
          x2,
          y,
          decoration.color,
          1,
          dash,
          decoration.shape === 6,
        );
      }
    }
  };
  const draw = (originX: number, originY: number) => {
    const baselineY = originY + decoration.baseline;
    if (decoration.kind === 'underline') {
      const y = decoration.underline === 'top'
        ? baselineY - decoration.fontSize + 1
        : baselineY + 2;
      drawLineShape(originX, y);
      return;
    }
    if (decoration.kind === 'strikethrough') {
      drawLineShape(originX, baselineY - decoration.fontSize * 0.3);
      return;
    }
    if (decoration.emphasisDot === 0) return;
    const dotSize = Math.max(1, decoration.fontSize * 0.3);
    let fillPaint: SkPaint | null = null;
    let strokePaint: SkPaint | null = null;
    try {
      fillPaint = this.makeFillPaint(decoration.color) as SkPaint;
      strokePaint = this.makeStrokePaint(
        decoration.color,
        Math.max(dotSize * 0.12, 0.75),
      ) as SkPaint;
      for (const position of decoration.positions.slice(0, -1)) {
        const x = originX + position + decoration.fontSize * decoration.ratio * 0.5;
        const y = baselineY - decoration.fontSize * 1.05;
        const centerY = y - dotSize * 0.45;
        if (decoration.emphasisDot === 1) {
          canvas.drawCircle(x, centerY, Math.max(dotSize * 0.48, 1), fillPaint);
        } else if (decoration.emphasisDot === 2) {
          canvas.drawCircle(x, centerY, Math.max(dotSize * 0.48, 1), strokePaint);
        } else if (decoration.emphasisDot === 3) {
          canvas.drawLine(x - dotSize * 0.45, centerY - dotSize * 0.2, x, centerY + dotSize * 0.25, strokePaint);
          canvas.drawLine(x, centerY + dotSize * 0.25, x + dotSize * 0.45, centerY - dotSize * 0.2, strokePaint);
        } else if (decoration.emphasisDot === 4) {
          canvas.drawLine(x - dotSize * 0.5, centerY, x - dotSize * 0.15, centerY - dotSize * 0.22, strokePaint);
          canvas.drawLine(x - dotSize * 0.15, centerY - dotSize * 0.22, x + dotSize * 0.15, centerY + dotSize * 0.22, strokePaint);
          canvas.drawLine(x + dotSize * 0.15, centerY + dotSize * 0.22, x + dotSize * 0.5, centerY, strokePaint);
        } else if (decoration.emphasisDot === 5) {
          canvas.drawCircle(x, centerY, Math.max(dotSize * 0.22, 0.75), fillPaint);
        } else {
          const radius = Math.max(dotSize * 0.18, 0.7);
          canvas.drawCircle(x, centerY - radius * 1.5, radius, fillPaint);
          canvas.drawCircle(x, centerY + radius * 1.5, radius, fillPaint);
        }
      }
    } finally {
      strokePaint?.delete?.();
      fillPaint?.delete?.();
    }
  };

  this.withHorizontalTextVisualOrigin(
    canvas,
    op.bbox,
    decoration.rotation,
    'textDecoration',
    draw,
  );
}
