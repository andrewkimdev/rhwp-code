/** canvaskit-renderer color replay — 색상·그래디언트 헬퍼 무리를 CanvasKitLayerRenderer 에서 추출한 free-function 모듈. */
/* eslint-disable @typescript-eslint/no-explicit-any */

import type { Color, Path, PathBuilder } from 'canvaskit-wasm';
import type { LayerAffineTransform, LayerGlyphOutlineOp } from '@/core/types';

type MutablePath = Path & Pick<PathBuilder, 'arcToRotated' | 'close' | 'cubicTo' | 'lineTo' | 'moveTo'>;
type LayerColorGraph = NonNullable<NonNullable<LayerGlyphOutlineOp['colorLayers']>['paintGraph']>;
type LayerColorGraphNode = NonNullable<LayerColorGraph['nodes']>[number];

export function affineToCanvasKitMatrix(this: any, transform: LayerAffineTransform | undefined): number[] | null {
  if (!transform) return null;
  return [
    transform.a,
    transform.c,
    transform.e,
    transform.b,
    transform.d,
    transform.f,
    0,
    0,
    1,
  ];
}

export function applyFillRule(this: any, path: MutablePath, fillRule: string | undefined): void {
  if (fillRule === 'evenodd') {
    (path as unknown as { setFillType?: (fillType: unknown) => void }).setFillType?.(this.canvasKit.FillType.EvenOdd);
  }
}

export function resolvedColor(this: any, color: { rgba?: number[] }): Color {
  const rgba = color.rgba ?? [0, 0, 0, 1];
  return this.canvasKit.Color(
    clampUnit(rgba[0]),
    clampUnit(rgba[1]),
    clampUnit(rgba[2]),
    clampUnit(rgba[3]),
  );
}

export function makeLinearGradientShader(this: any, gradient: NonNullable<LayerColorGraphNode['linearGradientPath']>['gradient']): unknown {
  const shaderApi = this.canvasKit.Shader as unknown as { MakeLinearGradient?: (...args: unknown[]) => unknown };
  return shaderApi.MakeLinearGradient?.(
    [gradient?.x0 ?? 0, gradient?.y0 ?? 0],
    [gradient?.x1 ?? 0, gradient?.y1 ?? 0],
    gradientColors(gradient?.stops),
    gradientPositions(gradient?.stops),
    this.canvasKit.TileMode.Clamp,
  );
}

export function makeRadialGradientShader(this: any, gradient: NonNullable<LayerColorGraphNode['radialGradientPath']>['gradient']): unknown {
  const shaderApi = this.canvasKit.Shader as unknown as { MakeRadialGradient?: (...args: unknown[]) => unknown };
  return shaderApi.MakeRadialGradient?.(
    [gradient?.cx ?? 0, gradient?.cy ?? 0],
    gradient?.radius ?? 1,
    gradientColors(gradient?.stops),
    gradientPositions(gradient?.stops),
    this.canvasKit.TileMode.Clamp,
  );
}

export function makeSweepGradientShader(this: any, gradient: NonNullable<LayerColorGraphNode['sweepGradientPath']>['gradient']): unknown {
  const shaderApi = this.canvasKit.Shader as unknown as { MakeSweepGradient?: (...args: unknown[]) => unknown };
  return shaderApi.MakeSweepGradient?.(
    gradient?.cx ?? 0,
    gradient?.cy ?? 0,
    gradientColors(gradient?.stops),
    gradientPositions(gradient?.stops),
    this.canvasKit.TileMode.Clamp,
    null,
    0,
    gradient?.startAngleDegrees ?? 0,
    gradient?.endAngleDegrees ?? 360,
  );
}

export function parseCssColor(value: string): { r: number; g: number; b: number; a: number } {
  const trimmed = value.trim();
  if (trimmed === 'transparent') {
    return { r: 0, g: 0, b: 0, a: 0 };
  }
  if (trimmed === 'black') {
    return { r: 0, g: 0, b: 0, a: 1 };
  }
  if (trimmed === 'white') {
    return { r: 255, g: 255, b: 255, a: 1 };
  }
  const shortHex = /^#?([0-9a-f]{3,4})$/i.exec(trimmed);
  if (shortHex) {
    const value = shortHex[1];
    return {
      r: Number.parseInt(value[0] + value[0], 16),
      g: Number.parseInt(value[1] + value[1], 16),
      b: Number.parseInt(value[2] + value[2], 16),
      a: value.length === 4 ? Number.parseInt(value[3] + value[3], 16) / 255 : 1,
    };
  }
  const hexWithAlpha = /^#?([0-9a-f]{8})$/i.exec(trimmed);
  if (hexWithAlpha) {
    const n = Number.parseInt(hexWithAlpha[1], 16);
    return {
      r: (n >> 24) & 0xff,
      g: (n >> 16) & 0xff,
      b: (n >> 8) & 0xff,
      a: (n & 0xff) / 255,
    };
  }
  const hex = /^#?([0-9a-f]{6})$/i.exec(trimmed);
  if (hex) {
    const n = Number.parseInt(hex[1], 16);
    return {
      r: (n >> 16) & 0xff,
      g: (n >> 8) & 0xff,
      b: n & 0xff,
      a: 1,
    };
  }
  const rgb = /^rgba?\((\d+),\s*(\d+),\s*(\d+)(?:,\s*([0-9.]+))?\)$/i.exec(trimmed);
  if (rgb) {
    return {
      r: Number(rgb[1]),
      g: Number(rgb[2]),
      b: Number(rgb[3]),
      a: rgb[4] === undefined ? 1 : Number(rgb[4]),
    };
  }
  return { r: 0, g: 0, b: 0, a: 1 };
}

export function clampUnit(value: number | undefined): number {
  return Math.max(0, Math.min(1, Number.isFinite(value) ? value ?? 0 : 0));
}

export function gradientColors(stops: Array<{ color?: { rgba?: number[] } }> | undefined): number[][] {
  return (stops ?? []).map((stop) => {
    const rgba = stop.color?.rgba ?? [0, 0, 0, 1];
    return [
      clampUnit(rgba[0]),
      clampUnit(rgba[1]),
      clampUnit(rgba[2]),
      clampUnit(rgba[3]),
    ];
  });
}

export function gradientPositions(stops: Array<{ offset?: number }> | undefined): number[] {
  return (stops ?? []).map((stop) => Math.max(0, Math.min(1, stop.offset ?? 0)));
}
