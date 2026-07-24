export interface ZoomAnchor {
  x: number;
  y: number;
}

export const CENTER_ZOOM_ANCHOR: ZoomAnchor = Object.freeze({ x: 0.5, y: 0.5 });

function normalizeAxis(value: unknown): number {
  return typeof value === 'number' && Number.isFinite(value)
    ? Math.max(0, Math.min(1, value))
    : 0.5;
}

export function normalizeZoomAnchor(
  anchor?: Partial<ZoomAnchor> | null,
): ZoomAnchor {
  return {
    x: normalizeAxis(anchor?.x),
    y: normalizeAxis(anchor?.y),
  };
}
