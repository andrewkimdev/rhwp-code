/** canvaskit-renderer glyph-outline replay — 글리프 아웃라인(glyphRun·glyphOutline·bitmap/svg/monochrome 글리프) 렌더링 무리를 CanvasKitLayerRenderer 에서 추출한 free-function 모듈. */
/* eslint-disable @typescript-eslint/no-explicit-any */

import type { Canvas, Paint, Path, PathBuilder } from 'canvaskit-wasm';
import type {
  LayerGlyphOutlineOp,
  LayerGlyphRunOp,
  LayerImageOp,
} from '@/core/types';
import { base64ToBytes } from './image-cache';
import { drawCanvasKitGlyphRun } from './glyph-run-fonts';
import { staticSvgPathLayersAreReplayable } from './text-variant-selection';
import { layerResourceKeyMatches } from './resource-key';
import { glyphOutlinePayloadResourceKey } from '../glyph-outline-payload-status';
import { parseStaticSvgPathLayers, type StaticSvgPathLayer } from '../static-svg-path-layers';

type SkCanvas = Canvas;
type SkPaint = Paint;
type MutablePath = Path & Pick<PathBuilder, 'arcToRotated' | 'close' | 'cubicTo' | 'lineTo' | 'moveTo'>;
type LayerColorGraph = NonNullable<NonNullable<LayerGlyphOutlineOp['colorLayers']>['paintGraph']>;
type LayerColorGraphNode = NonNullable<LayerColorGraph['nodes']>[number];

const MAX_SVG_GLYPH_CACHE_ENTRIES = 128;
const MAX_BITMAP_GLYPH_BASE64_LENGTH = Math.ceil(4 * 1024 * 1024 / 3) * 4;
const MAX_STATIC_SVG_GLYPH_BYTES = 1024 * 1024;

export function layerResourceIndex(
  this: any,
  id: number | string | undefined,
  keys: string[] | undefined,
  length: number,
): number | null {
  if (typeof id === 'number' && Number.isInteger(id) && id >= 0 && id < length) return id;
  if (typeof id !== 'string') return null;
  const index = keys?.indexOf(id) ?? -1;
  return index >= 0 && index < length ? index : null;
}

export function bitmapGlyphImageOp(this: any, op: LayerGlyphOutlineOp): LayerImageOp | null {
  const payload = op.bitmapGlyph;
  const resources = this.currentResources;
  const index = this.layerResourceIndex(
    payload?.imageResourceId ?? payload?.imageRef,
    resources?.imageKeys,
    resources?.images?.length ?? 0,
  );
  if (!payload || index === null || !payload.placement) return null;
  const base64 = resources?.images?.[index];
  const resourceKey = resources?.imageKeys?.[index];
  const payloadResourceKey = glyphOutlinePayloadResourceKey(op);
  let bytes: Uint8Array;
  try {
    if (typeof base64 !== 'string'
      || base64.length > MAX_BITMAP_GLYPH_BASE64_LENGTH) {
      return null;
    }
    bytes = base64ToBytes(base64);
  } catch {
    return null;
  }
  if (
    typeof resourceKey !== 'string'
    || payloadResourceKey === null
    || op.payloadResourceKey !== `${payloadResourceKey}:resource:${resourceKey}`
    || !layerResourceKeyMatches('img', resourceKey, bytes)
  ) {
    return null;
  }
  return {
    type: 'image',
    bbox: payload.placement,
    base64,
    imageRef: `glyph:${resourceKey}`,
    fillMode: 'fitToSize',
  };
}

export function staticSvgGlyphPathLayers(this: any, op: LayerGlyphOutlineOp): StaticSvgPathLayer[] | null {
  const payload = op.svgGlyph;
  const resources = this.currentResources;
  const index = this.layerResourceIndex(
    payload?.vectorResourceId ?? payload?.svgRef,
    resources?.svgKeys,
    resources?.svgFragments?.length ?? 0,
  );
  if (!payload || index === null) return null;
  const fragment = resources?.svgFragments?.[index];
  const resourceKey = resources?.svgKeys?.[index];
  const payloadResourceKey = glyphOutlinePayloadResourceKey(op);
  if (typeof fragment !== 'string'
    || fragment.length > MAX_STATIC_SVG_GLYPH_BYTES) {
    return null;
  }
  const fragmentBytes = new TextEncoder().encode(fragment);
  if (
    fragmentBytes.byteLength > MAX_STATIC_SVG_GLYPH_BYTES
    || typeof resourceKey !== 'string'
    || payloadResourceKey === null
    || op.payloadResourceKey !== `${payloadResourceKey}:resource:${resourceKey}`
    || !layerResourceKeyMatches('svg', resourceKey, fragmentBytes)
  ) {
    return null;
  }
  const cached = this.svgGlyphPathCache.get(resourceKey);
  if (cached) {
    this.svgGlyphPathCache.delete(resourceKey);
    this.svgGlyphPathCache.set(resourceKey, cached);
    return cached;
  }
  if (this.svgGlyphParseFailures.has(resourceKey)) return null;

  const layers = parseStaticSvgPathLayers(fragment, op.paintStyle?.color ?? '#000000');
  if (layers.length === 0) {
    this.rememberSvgGlyphParseFailure(resourceKey);
    return null;
  }
  if (!staticSvgPathLayersAreReplayable(
    layers,
    pathData => this.canvasKit.Path.MakeFromSVGString(pathData),
  )) {
    this.rememberSvgGlyphParseFailure(resourceKey);
    return null;
  }
  if (this.svgGlyphPathCache.size >= MAX_SVG_GLYPH_CACHE_ENTRIES) {
    const oldestKey = this.svgGlyphPathCache.keys().next().value as string | undefined;
    if (oldestKey !== undefined) this.svgGlyphPathCache.delete(oldestKey);
  }
  this.svgGlyphPathCache.set(resourceKey, layers);
  return layers;
}

export function rememberSvgGlyphParseFailure(this: any, resourceKey: string): void {
  if (this.svgGlyphParseFailures.size >= MAX_SVG_GLYPH_CACHE_ENTRIES) {
    const oldestKey = this.svgGlyphParseFailures.values().next().value as string | undefined;
    if (oldestKey !== undefined) this.svgGlyphParseFailures.delete(oldestKey);
  }
  this.svgGlyphParseFailures.add(resourceKey);
}

export function renderGlyphRun(this: any, canvas: SkCanvas, op: LayerGlyphRunOp): void {
  const font = this.glyphRunFonts.font(op, this.currentFontResources);
  if (!font) {
    this.unsupportedOps.add('glyphRun:replayInvariant');
    return;
  }
  const paint = this.makeFillPaint(op.paintStyle.color ?? '#000000');
  try {
    if (drawCanvasKitGlyphRun(canvas, op, font, paint)) {
      this.currentReplayFeatureCounts.glyphRuns += 1;
    } else {
      this.unsupportedOps.add('glyphRun:replayFailed');
    }
  } finally {
    paint.delete?.();
  }
}

export function renderGlyphOutline(this: any, canvas: SkCanvas, op: LayerGlyphOutlineOp): void {
  if (op.payloadKind === 'bitmapGlyph') {
    this.renderBitmapGlyphOutline(canvas, op);
    return;
  }
  if (op.payloadKind === 'svgGlyph') {
    this.renderSvgGlyphOutline(canvas, op);
    return;
  }
  if (op.payloadKind === 'monochromeFill' || op.payloadKind === 'monochromeFillStroke') {
    this.renderMonochromeGlyphOutline(canvas, op);
    return;
  }
  const graph = op.colorLayers?.paintGraph;
  const nodes = graph?.nodes ?? [];
  if (!graph || nodes.length === 0 || graph.rootNodeId === undefined) {
    this.unsupportedOps.add('glyphOutline:replayInvariant');
    return;
  }
  const nodesById = new Map<number, LayerColorGraphNode>();
  for (const node of nodes) {
    if (node.nodeId !== undefined) {
      nodesById.set(node.nodeId, node);
    }
  }
  canvas.save();
  const matrix = this.affineToCanvasKitMatrix(op.placement?.runToPage);
  if (matrix) {
    (canvas as unknown as { concat?: (matrix: number[]) => void }).concat?.(matrix);
  }
  try {
    this.renderColorPaintGraphNode(canvas, nodesById, graph.rootNodeId, new Set());
  } finally {
    canvas.restore();
  }
}

export function renderBitmapGlyphOutline(this: any, canvas: SkCanvas, op: LayerGlyphOutlineOp): void {
  const imageOp = this.bitmapGlyphImageOp(op);
  const image = imageOp ? this.imageForOp(imageOp) : null;
  if (!imageOp || !image) {
    this.unsupportedOps.add('glyphOutline:bitmapReplayInvariant');
    return;
  }
  canvas.save();
  try {
    const transform = op.bitmapGlyph?.transformToRun;
    const matrix = this.affineToCanvasKitMatrix(transform);
    if (matrix) (canvas as unknown as { concat: (matrix: number[]) => void }).concat(matrix);
    this.drawImageOp(canvas, image, imageOp);
  } finally {
    canvas.restore();
  }
}

export function renderSvgGlyphOutline(this: any, canvas: SkCanvas, op: LayerGlyphOutlineOp): void {
  const payload = op.svgGlyph;
  const viewBox = payload?.viewBox;
  const layers = this.staticSvgGlyphPathLayers(op);
  if (!payload || !viewBox || !layers || !this.boundsAreDrawable(op.bbox) || !this.boundsAreDrawable(viewBox)) {
    this.unsupportedOps.add('glyphOutline:svgReplayInvariant');
    return;
  }
  canvas.save();
  try {
    const payloadMatrix = this.affineToCanvasKitMatrix(payload.transformToRun);
    if (payloadMatrix) {
      (canvas as unknown as { concat: (matrix: number[]) => void }).concat(payloadMatrix);
    }
    canvas.translate(op.bbox.x, op.bbox.y);
    canvas.scale(op.bbox.width / viewBox.width, op.bbox.height / viewBox.height);
    canvas.translate(-viewBox.x, -viewBox.y);
    for (const layer of layers) {
      canvas.save();
      let path: Path | null = null;
      try {
        const layerMatrix = this.affineToCanvasKitMatrix(layer.transform);
        if (layerMatrix) {
          (canvas as unknown as { concat: (matrix: number[]) => void }).concat(layerMatrix);
        }
        path = this.canvasKit.Path.MakeFromSVGString(layer.pathData);
        if (!path) continue;
        this.applyGlyphPathFillRule(path, layer.fillRule);
        if (layer.fill !== null) {
          let paint: SkPaint | null = null;
          try {
            paint = this.makeFillPaint(layer.fill, layer.opacity) as SkPaint;
            canvas.drawPath(path, paint);
          } finally {
            paint?.delete?.();
          }
        }
        if (layer.stroke) {
          const stroke = layer.stroke;
          let paint: SkPaint | null = null;
          let effect: ReturnType<typeof this.canvasKit.PathEffect.MakeDash> | null = null;
          try {
            paint = this.makeStrokePaint(stroke.color, stroke.width, stroke.opacity) as SkPaint;
            paint.setStrokeJoin(this.canvasKit.StrokeJoin[
              stroke.lineJoin === 'round' ? 'Round' : stroke.lineJoin === 'bevel' ? 'Bevel' : 'Miter'
            ]);
            paint.setStrokeCap(this.canvasKit.StrokeCap[
              stroke.lineCap === 'round' ? 'Round' : stroke.lineCap === 'square' ? 'Square' : 'Butt'
            ]);
            paint.setStrokeMiter(stroke.miterLimit);
            effect = stroke.dashArray
              ? this.canvasKit.PathEffect.MakeDash(stroke.dashArray, stroke.dashOffset)
              : null;
            if (effect) paint.setPathEffect(effect);
            canvas.drawPath(path, paint);
          } finally {
            effect?.delete?.();
            paint?.delete?.();
          }
        }
      } finally {
        path?.delete?.();
        canvas.restore();
      }
    }
  } finally {
    canvas.restore();
  }
}

export function renderMonochromeGlyphOutline(this: any, canvas: SkCanvas, op: LayerGlyphOutlineOp): void {
  const matrix = this.affineToCanvasKitMatrix(op.placement?.runToPage);
  if (!matrix || !op.paths?.length) {
    this.unsupportedOps.add('glyphOutline:replayInvariant');
    return;
  }
  const fill = this.makeFillPaint(op.paintStyle?.color ?? '#000000');
  const stroke = op.payloadKind === 'monochromeFillStroke' && op.stroke
    ? this.makeStrokePaint(op.stroke.color ?? op.paintStyle?.color ?? '#000000', op.stroke.width ?? 1)
    : null;
  canvas.save();
  try {
    (canvas as unknown as { concat: (matrix: number[]) => void }).concat(matrix);
    for (const outline of op.paths) {
      const path = new this.canvasKit.Path() as MutablePath;
      let currentX = 0;
      let currentY = 0;
      try {
        for (const command of outline.commands ?? []) {
          [currentX, currentY] = this.applyPathCommand(path, command, currentX, currentY);
        }
        this.applyGlyphPathFillRule(path, outline.fillRule);
        canvas.drawPath(path, fill);
        if (stroke) canvas.drawPath(path, stroke);
      } finally {
        path.delete?.();
      }
    }
  } finally {
    canvas.restore();
    stroke?.delete?.();
    fill.delete?.();
  }
}

export function applyGlyphPathFillRule(this: any, path: Path, fillRule: string | undefined): void {
  path.setFillType(fillRule === 'evenodd' ? this.canvasKit.FillType.EvenOdd : this.canvasKit.FillType.Winding);
}

export function renderColorPaintGraphNode(
  this: any,
  canvas: SkCanvas,
  nodesById: Map<number, LayerColorGraphNode>,
  nodeId: number,
  visited: Set<number>,
): void {
  if (visited.has(nodeId)) {
    this.unsupportedOps.add('glyphOutline:replayInvariant');
    return;
  }
  visited.add(nodeId);
  const node = nodesById.get(nodeId);
  if (!node) {
    this.unsupportedOps.add('glyphOutline:replayInvariant');
    return;
  }
  if (node.kind === 'transform') {
    const transformNode = node.transform;
    const matrix = this.affineToCanvasKitMatrix(transformNode?.transform);
    if (!matrix || transformNode?.childNodeId === undefined) {
      this.unsupportedOps.add('glyphOutline:replayInvariant');
      return;
    }
    canvas.save();
    (canvas as unknown as { concat?: (matrix: number[]) => void }).concat?.(matrix);
    try {
      this.renderColorPaintGraphNode(canvas, nodesById, transformNode.childNodeId, visited);
    } finally {
      canvas.restore();
    }
    return;
  }
  const pathNode = node.solidPath ?? node.linearGradientPath ?? node.radialGradientPath ?? node.sweepGradientPath;
  if (!pathNode?.commands) {
    this.unsupportedOps.add('glyphOutline:replayInvariant');
    return;
  }
  const path = new this.canvasKit.Path() as MutablePath;
  let currentX = 0;
  let currentY = 0;
  for (const command of pathNode.commands) {
    [currentX, currentY] = this.applyPathCommand(path, command, currentX, currentY);
  }
  this.applyFillRule(path, pathNode.fillRule);
  const paint = new this.canvasKit.Paint();
  let shader: unknown | undefined;
  try {
    paint.setAntiAlias?.(true);
    paint.setStyle(this.canvasKit.PaintStyle.Fill);
    if (node.kind === 'solidPath' && node.solidPath?.fill) {
      paint.setColor(this.resolvedColor(node.solidPath.fill));
    } else if (node.kind === 'linearGradientPath' && node.linearGradientPath?.gradient) {
      shader = this.makeLinearGradientShader(node.linearGradientPath.gradient);
      if (!shader) {
        return;
      }
      (paint as unknown as { setShader: (shader: unknown) => void }).setShader(shader);
    } else if (node.kind === 'radialGradientPath' && node.radialGradientPath?.gradient) {
      shader = this.makeRadialGradientShader(node.radialGradientPath.gradient);
      if (!shader) {
        return;
      }
      (paint as unknown as { setShader: (shader: unknown) => void }).setShader(shader);
    } else if (node.kind === 'sweepGradientPath' && node.sweepGradientPath?.gradient) {
      shader = this.makeSweepGradientShader(node.sweepGradientPath.gradient);
      if (!shader) {
        return;
      }
      (paint as unknown as { setShader: (shader: unknown) => void }).setShader(shader);
    } else {
      return;
    }
    canvas.drawPath(path, paint);
  } finally {
    (shader as { delete?: () => void } | undefined)?.delete?.();
    paint.delete?.();
    path.delete?.();
  }
}
