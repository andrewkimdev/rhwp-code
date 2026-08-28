/** canvaskit-renderer text-run replay — 텍스트런 CORE 렌더링 무리를 CanvasKitLayerRenderer 에서 추출한 free-function 모듈. */
/* eslint-disable @typescript-eslint/no-explicit-any */

import type { Canvas, Font, FontMgr, Paint, Path } from 'canvaskit-wasm';
import type { LayerBounds, LayerTextRunOp } from '@/core/types';
import type {
  CanvasKitFontSubstitutionDiagnostic,
  CanvasKitLocalTypeface,
} from '../canvaskit-renderer';
import { localFontFaceKey, resolveLocalFont } from '@/core/local-fonts';

type SkCanvas = Canvas;
type SkPaint = Paint;

export const OLD_HANGUL_FONT_FAMILY = 'Source Han Serif K Old Hangul';
export const MAX_FONT_SUBSTITUTION_DIAGNOSTICS = 4096;
const MAX_TEXT_VISUAL_WAVE_SEGMENTS = 4096;
const MAX_TEXT_RUN_CODE_POINTS = 4096;
const MAX_TEXT_RUN_FALLBACK_SPANS = 4096;
// 단일 text run은 줄바꿈 없이 문서가 지정한 위치에 재생한다.
const MAX_SHAPED_TEXT_WIDTH = 1_000_000;
const COMPLEX_SHAPING_UNICODE_CATEGORY = /[\p{M}\p{Cf}]/u;
const VERTICAL_PRESENTATION_BASE_TEXT = new Map<string, string>([
  ['\uFE19', '\u2026'],
  ['\uFE31', '\u2014'],
  ['\uFE32', '\u2013'],
  ['\uFE33', '_'],
  ['\uFE34', '~'],
  ['\uFE35', '('],
  ['\uFE36', ')'],
  ['\uFE37', '{'],
  ['\uFE38', '}'],
  ['\uFE39', '['],
  ['\uFE3A', ']'],
  ['\uFE3B', '\u3010'],
  ['\uFE3C', '\u3011'],
  ['\uFE3D', '\u300A'],
  ['\uFE3E', '\u300B'],
  ['\uFE3F', '\u3008'],
  ['\uFE40', '\u3009'],
  ['\uFE41', '\u300C'],
  ['\uFE42', '\u300D'],
  ['\uFE43', '\u300E'],
  ['\uFE44', '\u300F'],
]);

export function primaryFontFamily(value: string | null | undefined): string {
  return (value ?? '')
    .split(',')[0]
    .trim()
    .replace(/^(["'])|(["'])$/g, '');
}

export function normalizedFontFamily(value: string | null | undefined): string {
  return primaryFontFamily(value)
    .replace(/\u0000/g, '')
    .normalize('NFC')
    .replace(/\s+/g, ' ')
    .trim()
    .toLocaleLowerCase('en-US');
}

function textRequiresComplexShaping(text: string): boolean {
  for (const character of text) {
    const codePoint = character.codePointAt(0) ?? 0;
    const isOldHangul = (codePoint >= 0x1100 && codePoint <= 0x11ff)
      || (codePoint >= 0xa960 && codePoint <= 0xa97f)
      || (codePoint >= 0xd7b0 && codePoint <= 0xd7ff);
    const isBoxedPua = codePoint >= 0xf02b1 && codePoint <= 0xf02c4;
    if (isOldHangul || isBoxedPua) continue;

    const nominalGlyphReplayIsSafe = codePoint <= 0x02ff
      || (codePoint >= 0x0370 && codePoint <= 0x058f)
      || (codePoint >= 0x1e00 && codePoint <= 0x1fff)
      || (codePoint >= 0x2000 && codePoint <= 0x2fff)
      || (codePoint >= 0x2e80 && codePoint <= 0xd7af)
      || (codePoint >= 0xe000 && codePoint <= 0xf8ff)
      || (codePoint >= 0xf900 && codePoint <= 0xfb06)
      || (codePoint >= 0xfe10 && codePoint <= 0xfe1f)
      || (codePoint >= 0xfe30 && codePoint <= 0xfe6f)
      || (codePoint >= 0xff00 && codePoint <= 0xffef)
      || (codePoint >= 0x1d400 && codePoint <= 0x1d7ff)
      || (codePoint >= 0x20000 && codePoint <= 0x323af);
    if (!nominalGlyphReplayIsSafe || COMPLEX_SHAPING_UNICODE_CATEGORY.test(character)) {
      return true;
    }
  }
  return false;
}

function textRunHasPaintEffects(style: NonNullable<LayerTextRunOp['style']>): boolean {
  const shadeColor = (style.shadeColor ?? '#ffffff').toLowerCase();
  return (style.outlineType ?? 0) !== 0
    || (style.shadowType ?? 0) !== 0
    || style.emboss === true
    || style.engrave === true
    || (shadeColor !== '#ffffff' && shadeColor !== '#000000')
    || Math.abs((style.ratio ?? 1) - 1) > Number.EPSILON;
}

export function recordTextRunCoverageGaps(this: any, op: LayerTextRunOp, codePoints: readonly string[]): boolean {
  const style = op.style ?? {};
  const decorationsAreExternal = op.legacyVisuals?.decorations === 'mirror';
  if (!decorationsAreExternal && style.underline && style.underline !== 'none') {
    this.unsupportedOps.add('textRun:textDecoration');
  }
  if (!decorationsAreExternal && style.strikethrough) {
    this.unsupportedOps.add('textRun:textDecoration');
  }
  if (!decorationsAreExternal && style.emphasisDot && style.emphasisDot !== 0) {
    this.unsupportedOps.add('textRun:emphasisDot');
  }
  const replayText = op.displayText ?? op.text;
  const hasOldHangul = codePoints.some((character) => {
    const codePoint = character.codePointAt(0) ?? 0;
    return (codePoint >= 0x1100 && codePoint <= 0x11ff)
      || (codePoint >= 0xa960 && codePoint <= 0xa97f)
      || (codePoint >= 0xd7b0 && codePoint <= 0xd7ff);
  });
  const hasBoxedPua = codePoints.some((character) => {
    const codePoint = character.codePointAt(0) ?? 0;
    return codePoint >= 0xf02b1 && codePoint <= 0xf02c4;
  });
  const requiresUnsupportedShaping = textRequiresComplexShaping(replayText)
    || (textRunHasPaintEffects(style) && (hasOldHangul || hasBoxedPua));
  if (requiresUnsupportedShaping) {
    this.unsupportedOps.add('textRun:scriptTextRequiresShaping');
  }
  return requiresUnsupportedShaping;
}

export function renderTextRun(this: any, canvas: SkCanvas, op: LayerTextRunOp): void {
  if (op.charOverlap && op.legacyVisuals?.charOverlap === 'mirror') {
    return;
  }
  if (op.charOverlap) {
    this.renderCharOverlap(canvas, {
      type: 'charOverlap',
      bbox: op.bbox,
      text: op.text,
      baseline: op.baseline ?? op.style?.fontSize ?? 12,
      rotation: op.rotation ?? 0,
      isVertical: op.isVertical === true,
      style: op.style ?? {},
      positions: op.positions ?? [],
      positionsComplete: op.positions !== undefined,
      charOverlap: op.charOverlap,
    });
    return;
  }
  const replayText = op.displayText ?? op.text;
  const replayPositions = op.displayText !== undefined ? op.displayPositions : op.positions;
  if (!replayText) return;
  const replayCodePoints: string[] = [];
  for (const character of replayText) {
    if (replayCodePoints.length >= MAX_TEXT_RUN_CODE_POINTS) {
      this.unsupportedOps.add('textRun:visualItemLimitExceeded');
      return;
    }
    replayCodePoints.push(character);
  }
  const style = op.style ?? {};
  if (this.recordTextRunCoverageGaps(op, replayCodePoints)) return;
  const ratio = style.ratio ?? 1;
  const outlineType = style.outlineType ?? 0;
  const shadowType = style.shadowType ?? 0;
  const shadowOffsetX = style.shadowOffsetX ?? 0;
  const shadowOffsetY = style.shadowOffsetY ?? 0;
  const shadeColor = (style.shadeColor ?? '#ffffff').toLowerCase();
  const baseFontSize = style.fontSize ?? Math.max(1, op.bbox.height || 12);
  const baseline = op.baseline ?? baseFontSize;
  const rotation = op.rotation ?? 0;
  if (![op.bbox.x, op.bbox.y, op.bbox.width, op.bbox.height, ratio, outlineType,
    shadowType, shadowOffsetX, shadowOffsetY, baseFontSize, baseline, rotation]
    .every(Number.isFinite)
    || op.bbox.width < 0
    || op.bbox.height < 0
    || ratio <= 0
    || baseFontSize <= 0
    || !Number.isInteger(outlineType)
    || outlineType < 0
    || !Number.isInteger(shadowType)
    || shadowType < 0) {
    this.unsupportedOps.add('textRun:invalidGeometry');
    return;
  }
  const verticalPresentationText = op.isVertical
    && op.orientation !== 'vertical-sideways'
    ? VERTICAL_PRESENTATION_BASE_TEXT.get(replayText)
    : undefined;
  const glyphReplayText = verticalPresentationText ?? replayText;
  const codePoints = verticalPresentationText === undefined
    ? replayCodePoints
    : [verticalPresentationText];
  const hasOldHangul = codePoints.some((codePoint) => {
    const code = codePoint.codePointAt(0) ?? 0;
    return (code >= 0x1100 && code <= 0x11ff)
      || (code >= 0xa960 && code <= 0xa97f)
      || (code >= 0xd7b0 && code <= 0xd7ff);
  });
  const effectPaints: SkPaint[] = [];
  let fontSize = baseFontSize;
  let baselineShift = 0;
  if (style.superscript) {
    fontSize = baseFontSize * 0.7;
    baselineShift -= baseFontSize * 0.3;
  } else if (style.subscript) {
    fontSize = baseFontSize * 0.7;
    baselineShift += baseFontSize * 0.15;
  }
  const placementMatrix = this.affineToCanvasKitMatrix(op.placement?.runToPage);
  const originX = placementMatrix ? 0 : op.bbox.x;
  const originY = placementMatrix
    ? (op.placement?.baselineY ?? 0)
    : op.bbox.y + baseline;
  const needsPreservedAdvances = style.superscript || style.subscript;
  const hasLayoutPositions = replayPositions?.length === codePoints.length + 1
    && replayPositions.every(Number.isFinite);
  const requestedFontFamily = primaryFontFamily(style.fontFamily);
  const preparedTypeface = this.findPreparedTypeface(requestedFontFamily);
  if (requestedFontFamily && !preparedTypeface && this.requirePreparedFontFamilies) {
    throw new Error(`CanvasKit font family가 준비되지 않았습니다: ${requestedFontFamily}`);
  }
  if (requestedFontFamily && !preparedTypeface) {
    this.recordFontSubstitution({
      requestedFamily: requestedFontFamily,
      resolvedFamily: this.defaultFontFamily ?? 'CanvasKit default',
      source: 'unregisteredDefault',
      kind: 'unregisteredFallback',
    });
  }
  const typeface = preparedTypeface?.typeface ?? this.defaultTypeface;
  const fontManager = preparedTypeface?.fontManager ?? this.defaultFontManager;
  const paint = this.makeFillPaint(style.color ?? '#000000');
  let font: Font | null = null;
  const fallbackFonts: Font[] = [];
  let boxedPuaFont: Font | null = null;
  let boxedPuaStrokePaint: SkPaint | null = null;
  let canvasSaved = false;
  try {
    paint.setAntiAlias?.(true);
    if (!typeface && !fontManager && !this.symbolFallbackTypeface
      && /[^\u0000-\u00ff]/.test(replayText)) {
      this.unsupportedOps.add('textRunFont');
      return;
    }
    canvas.save();
    canvasSaved = true;
    if (placementMatrix) {
      canvas.concat(placementMatrix);
    } else if (rotation !== 0) {
      canvas.rotate(rotation, originX, originY);
    }

    {
      const adjustFont = (target: Font) => {
        const adjustable = target as Font & {
          setEmbolden?: (enabled: boolean) => void;
          setSkewX?: (skew: number) => void;
          setScaleX?: (scale: number) => void;
        };
        adjustable.setEmbolden?.(style.bold === true);
        adjustable.setSkewX?.(style.italic === true ? -0.2 : 0);
        adjustable.setScaleX?.(ratio);
      };
        font = new this.canvasKit.Font(typeface, fontSize) as Font;
      adjustFont(font);
      const candidateFonts = [font];
      const candidateFontSources: CanvasKitFontSubstitutionDiagnostic['source'][] = [
        'unregisteredDefault',
      ];
      const candidateFontFamilies = [
        preparedTypeface?.fontFamily ?? this.defaultFontFamily ?? 'CanvasKit default',
      ];
      let candidateGlyphIds: Uint16Array[] = [];
      const fallbackSpans: Array<{ start: number; end: number; fontIndex: number }> = [];
      let oldHangulTypeface: CanvasKitLocalTypeface | null = null;
      if (hasLayoutPositions) {
        const primaryGlyphIds = font.getGlyphIDs(glyphReplayText, codePoints.length);
        candidateGlyphIds = [primaryGlyphIds];
        oldHangulTypeface = hasOldHangul
          ? this.findPreparedTypeface(OLD_HANGUL_FONT_FAMILY)
          : null;
        if (primaryGlyphIds.some(glyphId => glyphId === 0)
          && this.defaultTypeface !== null
          && typeface !== this.defaultTypeface) {
          const defaultFont = new this.canvasKit.Font(this.defaultTypeface, fontSize);
          adjustFont(defaultFont);
          fallbackFonts.push(defaultFont);
          candidateFonts.push(defaultFont);
          candidateFontSources.push('missingGlyphDefault');
          candidateFontFamilies.push(this.defaultFontFamily ?? 'CanvasKit default');
          candidateGlyphIds.push(defaultFont.getGlyphIDs(glyphReplayText, codePoints.length));
        }
        if (codePoints.some((_, index) => candidateGlyphIds.every(ids => (ids[index] ?? 0) === 0))
          && this.symbolFallbackTypeface !== null
          && typeface !== this.symbolFallbackTypeface
          && this.defaultTypeface !== this.symbolFallbackTypeface) {
          const symbolFont = new this.canvasKit.Font(this.symbolFallbackTypeface, fontSize);
          adjustFont(symbolFont);
          fallbackFonts.push(symbolFont);
          candidateFonts.push(symbolFont);
          candidateFontSources.push('missingGlyphSymbol');
          candidateFontFamilies.push('CanvasKit symbol fallback');
          candidateGlyphIds.push(symbolFont.getGlyphIDs(glyphReplayText, codePoints.length));
        }
        const selectedFontIndices = codePoints.map((codePoint, index) => {
          const code = codePoint.codePointAt(0) ?? 0;
          if ((code >= 0x1100 && code <= 0x11ff)
            || (code >= 0xa960 && code <= 0xa97f)
            || (code >= 0xd7b0 && code <= 0xd7ff)) {
            return -2;
          }
          const candidateIndex = candidateGlyphIds.findIndex(ids => (ids[index] ?? 0) !== 0);
          if (candidateIndex >= 0) return candidateIndex;
          return code >= 0xF02B1 && code <= 0xF02C4 ? -1 : 0;
        });
        for (const fontIndex of new Set(selectedFontIndices)) {
          if (fontIndex > 0) {
            this.recordFontSubstitution({
              requestedFamily: requestedFontFamily || this.defaultFontFamily || 'CanvasKit default',
              resolvedFamily: candidateFontFamilies[fontIndex],
              source: candidateFontSources[fontIndex],
              kind: 'glyphCoverageFallback',
            });
          } else if (fontIndex === -2 && oldHangulTypeface?.fontManager) {
            this.recordFontSubstitution({
              requestedFamily: requestedFontFamily || this.defaultFontFamily || 'CanvasKit default',
              resolvedFamily: oldHangulTypeface.fontFamily ?? OLD_HANGUL_FONT_FAMILY,
              source: 'oldHangul',
              kind: 'glyphCoverageFallback',
            });
          }
        }
        let spanStart = 0;
        while (spanStart < codePoints.length) {
          const fontIndex = selectedFontIndices[spanStart];
          let spanEnd = spanStart + 1;
          if (fontIndex !== -1) {
            while (spanEnd < codePoints.length && selectedFontIndices[spanEnd] === fontIndex) {
              spanEnd += 1;
            }
          }
          fallbackSpans.push({ start: spanStart, end: spanEnd, fontIndex });
          if (fallbackSpans.length > MAX_TEXT_RUN_FALLBACK_SPANS) {
            this.unsupportedOps.add('textRun:fallbackSpanLimitExceeded');
            return;
          }
          spanStart = spanEnd;
        }
      }

      const drawPass = (
        fillPaint: SkPaint,
        offsetX = 0,
        offsetY = 0,
        strokePaint?: SkPaint,
      ) => {
        if (verticalPresentationText !== undefined) {
          const selectedFont = hasLayoutPositions
            ? candidateFonts[fallbackSpans[0]?.fontIndex ?? 0] ?? font!
            : font!;
          const glyphIds = selectedFont.getGlyphIDs(verticalPresentationText, 1);
          const glyphBounds = (
            selectedFont as Font & {
              getGlyphBounds?: (ids: Uint16Array) => Float32Array;
            }
          ).getGlyphBounds?.(glyphIds) ?? new Float32Array();
          const left = glyphBounds[0] ?? 0;
          const top = glyphBounds[1] ?? -fontSize;
          const right = glyphBounds[2] ?? fontSize;
          const bottom = glyphBounds[3] ?? 0;
          const advance = hasLayoutPositions
            ? replayPositions![1] - replayPositions![0]
            : op.bbox.width;
          const targetCenterX = originX
            + (Number.isFinite(advance) ? advance : op.bbox.width) / 2;
          const targetCenterY = originY - baseline + baselineShift + op.bbox.height / 2;
          canvas.save();
          try {
            canvas.translate(targetCenterX + offsetX, targetCenterY + offsetY);
            canvas.rotate(90, 0, 0);
            canvas.drawText(
              verticalPresentationText,
              -(left + right) / 2,
              -(top + bottom) / 2,
              fillPaint,
              selectedFont,
            );
            if (strokePaint) {
              canvas.drawText(
                verticalPresentationText,
                -(left + right) / 2,
                -(top + bottom) / 2,
                strokePaint,
                selectedFont,
              );
            }
          } finally {
            canvas.restore();
          }
          return;
        }

        if (!hasLayoutPositions) {
          const y = originY + baselineShift + offsetY;
          canvas.drawText(glyphReplayText, originX + offsetX, y, fillPaint, font!);
          if (strokePaint) {
            canvas.drawText(glyphReplayText, originX + offsetX, y, strokePaint, font!);
          }
          return;
        }

        let hasMissingGlyph = false;
        for (const { start: runStart, end: runEnd, fontIndex } of fallbackSpans) {
          if (fontIndex === -2) {
            if (!this.renderShapedScriptText(
              canvas,
              codePoints.slice(runStart, runEnd).join(''),
              style.color ?? '#000000',
              fontSize,
              originX + replayPositions![runStart] + offsetX,
              originY + offsetY,
              baselineShift,
              oldHangulTypeface?.fontManager ?? null,
              oldHangulTypeface?.fontFamily ?? OLD_HANGUL_FONT_FAMILY,
              style.bold === true,
              style.italic === true,
            )) {
              hasMissingGlyph = true;
            }
            continue;
          }
          if (fontIndex === -1) {
            const codePoint = codePoints[runStart].codePointAt(0) ?? 0;
            const displayNumber = String(codePoint - 0xF02B0);
            const boxSize = Math.max(1, fontSize * 0.72);
            const boxX = originX + replayPositions![runStart];
            const boxY = originY + baselineShift - fontSize * 0.76;
            boxedPuaStrokePaint ??= this.makeStrokePaint(
              style.color ?? '#000000',
              Math.max(0.6, fontSize * 0.04),
            ) as SkPaint;
            boxedPuaFont ??= new this.canvasKit.Font(
              this.symbolFallbackTypeface ?? this.defaultTypeface ?? typeface,
              Math.max(1, fontSize * 0.5),
            ) as Font;
            adjustFont(boxedPuaFont);
            const numberGlyphIds = boxedPuaFont.getGlyphIDs(
              displayNumber,
              displayNumber.length,
            );
            const numberWidth = (boxedPuaFont.getGlyphWidths(numberGlyphIds) ?? [])
              .reduce((sum, width) => sum + width, 0);
            canvas.drawRect(
              this.canvasKit.XYWHRect(
                boxX + offsetX,
                boxY + offsetY,
                boxSize,
                boxSize,
              ),
              boxedPuaStrokePaint,
            );
            canvas.drawText(
              displayNumber,
              boxX + (boxSize - numberWidth) / 2 + offsetX,
              boxY + boxSize * 0.72 + offsetY,
              fillPaint,
              boxedPuaFont,
            );
            continue;
          }
          const runGlyphIds = new Uint16Array(runEnd - runStart);
          const runPositions = new Float32Array((runEnd - runStart) * 2);
          for (let index = runStart; index < runEnd; index += 1) {
            const glyphId = candidateGlyphIds[fontIndex][index] ?? 0;
            runGlyphIds[index - runStart] = glyphId;
            runPositions[(index - runStart) * 2] = replayPositions![index];
            runPositions[(index - runStart) * 2 + 1] = baselineShift;
            hasMissingGlyph ||= glyphId === 0;
          }
          canvas.drawGlyphs(
            runGlyphIds,
            runPositions,
            originX + offsetX,
            originY + offsetY,
            candidateFonts[fontIndex],
            fillPaint,
          );
          if (strokePaint) {
            canvas.drawGlyphs(
              runGlyphIds,
              runPositions,
              originX + offsetX,
              originY + offsetY,
              candidateFonts[fontIndex],
              strokePaint,
            );
          }
        }
        if (hasMissingGlyph) this.unsupportedOps.add('textRun:glyphMapping');
      };

      if (!hasLayoutPositions && needsPreservedAdvances) {
        this.unsupportedOps.add('textRun:layoutPositions');
      }
      const textWidth = hasLayoutPositions
        ? replayPositions!.at(-1) ?? op.bbox.width
        : op.bbox.width;
      if (textWidth > 0 && shadeColor !== '#ffffff' && shadeColor !== '#000000') {
        const shadePaint = this.makeFillPaint(shadeColor);
        effectPaints.push(shadePaint);
        canvas.drawRect(
          this.canvasKit.XYWHRect(
            originX,
            originY + baselineShift - fontSize,
            textWidth,
            fontSize * 1.2,
          ),
          shadePaint,
        );
      }

      if (style.emboss || style.engrave) {
        const offset = Math.max(fontSize / 20, 1);
        const firstPaint = this.makeFillPaint(style.emboss ? '#ffffff' : '#808080');
        effectPaints.push(firstPaint);
        const secondPaint = this.makeFillPaint(style.emboss ? '#808080' : '#ffffff');
        effectPaints.push(secondPaint);
        drawPass(firstPaint, -offset, -offset);
        drawPass(secondPaint, offset, offset);
        drawPass(paint);
      } else {
        if (shadowType > 0) {
          const shadowPaint = this.makeFillPaint(style.shadowColor ?? style.color ?? '#000000');
          effectPaints.push(shadowPaint);
          drawPass(shadowPaint, shadowOffsetX, shadowOffsetY);
        }
        if (outlineType > 0) {
          const outlineFillPaint = this.makeFillPaint('#ffffff');
          effectPaints.push(outlineFillPaint);
          const outlineStrokePaint = this.makeStrokePaint(
            style.color ?? '#000000',
            Math.max(fontSize / 25, 0.5),
          );
          effectPaints.push(outlineStrokePaint);
          drawPass(outlineFillPaint, 0, 0, outlineStrokePaint);
        } else {
          drawPass(paint);
        }
      }
    }
  } finally {
    try {
      if (canvasSaved) canvas.restore();
    } finally {
      font?.delete?.();
      for (const fallbackFont of fallbackFonts) fallbackFont.delete?.();
      (boxedPuaFont as Font | null)?.delete?.();
      (boxedPuaStrokePaint as SkPaint | null)?.delete?.();
      for (const effectPaint of effectPaints) effectPaint.delete?.();
      paint.delete?.();
    }
  }
  if (op.isVertical) {
    this.currentReplayFeatureCounts.verticalTextRuns += 1;
  }
  if (verticalPresentationText !== undefined) {
    this.currentReplayFeatureCounts.verticalPresentationPunctuation += 1;
  }
}

export function withHorizontalTextVisualOrigin(
  this: any,
  canvas: SkCanvas,
  bbox: LayerBounds,
  rotation: number,
  opType: 'charOverlap' | 'textControlMark' | 'tabLeader' | 'textDecoration',
  draw: (originX: number, originY: number) => void,
): void {
  if (![bbox.x, bbox.y, bbox.width, bbox.height, rotation].every(Number.isFinite)
    || bbox.width < 0
    || bbox.height < 0) {
    this.unsupportedOps.add(`${opType}:invalidGeometry`);
    return;
  }
  if (rotation !== 0) {
    this.unsupportedOps.add(`${opType}:rotatedText`);
    return;
  }
  draw(bbox.x, bbox.y);
}

export function drawTextVisualStroke(
  this: any,
  canvas: SkCanvas,
  x1: number,
  y1: number,
  x2: number,
  y2: number,
  color: string,
  width: number,
  dash: number[] = [],
  roundCap = false,
  waveHeight = 0,
  waveWidth = 0,
): void {
  const paint = this.makeStrokePaint(color, width);
  let effect: ReturnType<typeof this.canvasKit.PathEffect.MakeDash> | null = null;
  let path: Path | null = null;
  try {
    if (roundCap) paint.setStrokeCap(this.canvasKit.StrokeCap.Round);
    if (dash.length > 0) {
      effect = this.canvasKit.PathEffect.MakeDash(dash, 0);
      if (effect) paint.setPathEffect(effect);
    }
    if (waveHeight > 0 && waveWidth > 0) {
      const builder = new this.canvasKit.PathBuilder();
      try {
        builder.moveTo(x1, y1);
        let cursor = x1;
        let up = true;
        const step = Math.max(
          waveWidth,
          (x2 - x1) / MAX_TEXT_VISUAL_WAVE_SEGMENTS,
        );
        while (cursor < x2) {
          const next = Math.min(cursor + step, x2);
          builder.quadTo((cursor + next) / 2, up ? y1 - waveHeight : y1 + waveHeight, next, y1);
          cursor = next;
          up = !up;
        }
          path = builder.detach() as Path;
      } finally {
        builder.delete?.();
      }
      canvas.drawPath(path, paint);
    } else {
      canvas.drawLine(x1, y1, x2, y2, paint);
    }
  } finally {
    path?.delete?.();
    effect?.delete?.();
    paint.delete?.();
  }
}

export function renderShapedScriptText(
  this: any,
  canvas: SkCanvas,
  text: string,
  color: string,
  fontSize: number,
  originX: number,
  originY: number,
  baselineShift: number,
  fontManager: FontMgr | null,
  fontFamily: string | null,
  bold: boolean,
  italic: boolean,
): boolean {
  if (!fontManager) return false;
  const textStyle = {
    color: this.color(color),
    fontSize,
    ...(fontFamily ? { fontFamilies: [fontFamily] } : {}),
    ...(this.canvasKit.FontWeight && this.canvasKit.FontSlant ? {
      fontStyle: {
        weight: bold ? this.canvasKit.FontWeight.Bold : this.canvasKit.FontWeight.Normal,
        slant: italic ? this.canvasKit.FontSlant.Italic : this.canvasKit.FontSlant.Upright,
      },
    } : {}),
  };
  const paragraphStyle = new this.canvasKit.ParagraphStyle({
    maxLines: 1,
    textStyle,
  });
  const builder = this.canvasKit.ParagraphBuilder.Make(paragraphStyle, fontManager);
  try {
    builder.addText(text);
    const paragraph = builder.build();
    try {
      paragraph.layout(MAX_SHAPED_TEXT_WIDTH);
      canvas.drawParagraph(paragraph, originX, originY - fontSize + baselineShift);
      return true;
    } finally {
      paragraph.delete?.();
    }
  } finally {
    builder.delete?.();
  }
}

export function findPreparedTypeface(this: any, fontFamily: string | undefined): CanvasKitLocalTypeface | null {
  const key = normalizedFontFamily(fontFamily);
  if (!key) return null;
  const record = resolveLocalFont(primaryFontFamily(fontFamily));
  const local = record ? this.localTypefaces.get(localFontFaceKey(record)) ?? null : null;
  const bundled = this.bundledTypefaceAliases.get(key);
  if (key === normalizedFontFamily(OLD_HANGUL_FONT_FAMILY)) {
    return [this.oldHangulTypeface, local, bundled]
      .find(candidate => candidate?.fontManager) ?? null;
  }
  if (local) return local;
  if (bundled) return bundled;
  if (key === normalizedFontFamily(this.defaultFontFamily) || key === 'noto sans kr') {
    return this.defaultTypeface || this.defaultFontManager
      ? {
          typeface: this.defaultTypeface,
          fontManager: this.defaultFontManager,
          fontFamily: this.defaultFontFamily,
        }
      : null;
  }
  return null;
}

export function recordFontSubstitution(this: any, diagnostic: CanvasKitFontSubstitutionDiagnostic): void {
  const key = JSON.stringify([
    diagnostic.requestedFamily,
    diagnostic.resolvedFamily,
    diagnostic.source,
  ]);
  if (this.currentFontSubstitutions.has(key)
    || this.currentFontSubstitutions.size < MAX_FONT_SUBSTITUTION_DIAGNOSTICS) {
    this.currentFontSubstitutions.set(key, diagnostic);
  }
}
