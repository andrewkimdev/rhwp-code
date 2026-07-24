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

export interface ZoomPageBox {
  left: number;
  top: number;
  width: number;
  height: number;
}

export interface ZoomViewportState {
  width: number;
  height: number;
  scrollLeft: number;
  scrollTop: number;
}

export function calculateAnchoredScroll(
  oldBox: ZoomPageBox,
  newBox: ZoomPageBox,
  viewport: ZoomViewportState,
  requestedAnchor: ZoomAnchor,
): Pick<ZoomViewportState, 'scrollLeft' | 'scrollTop'> {
  const anchor = normalizeZoomAnchor(requestedAnchor);
  const viewportX = viewport.width * anchor.x;
  const viewportY = viewport.height * anchor.y;
  const documentX = viewport.scrollLeft + viewportX;
  const documentY = viewport.scrollTop + viewportY;
  const ratioX = oldBox.width > 0
    ? (documentX - oldBox.left) / oldBox.width
    : 0.5;
  const ratioY = oldBox.height > 0
    ? (documentY - oldBox.top) / oldBox.height
    : 0.5;

  return {
    scrollLeft: newBox.left + newBox.width * ratioX - viewportX,
    scrollTop: newBox.top + newBox.height * ratioY - viewportY,
  };
}
