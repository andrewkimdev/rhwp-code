/** canvaskit-renderer image replay — 이미지 그리기·디코드/실패 캐시 무리를 CanvasKitLayerRenderer 에서 추출한 free-function 모듈. */
/* eslint-disable @typescript-eslint/no-explicit-any */

import type { Canvas, Image as SkImage, Rect } from 'canvaskit-wasm';
import type { LayerBounds, LayerImageOp } from '@/core/types';
import type { CanvasKitImageFailureReason } from '../canvaskit-renderer';
import {
  boundedCanvasKitSourceImageKey,
  canvasKitImageCacheKey,
  canvasKitImageFillModeStretches,
  canvasKitImageFillModeTiles,
  canvasKitImagePlacement,
  canvasKitImageSourceRect,
} from './image-replay';
import {
  CANVASKIT_MAX_ENCODED_IMAGE_BASE64_LENGTH,
  decodedImageMatchesEncodedHeader,
  replayableEncodedImageHeader,
} from './image-header';

type SkCanvas = Canvas;

// Prevent pathological tiled fills from monopolizing the render loop.
const MAX_IMAGE_TILE_DRAWS = 4096;
export const MAX_IMAGE_CACHE_ENTRIES = 128;
const MAX_IMAGE_FAILURE_CACHE_ENTRIES = 128;
export const MAX_IMAGE_CACHE_PIXELS = 64 * 1024 * 1024;

export function base64ToBytes(base64: string): Uint8Array {
  const binary = atob(base64);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i += 1) {
    bytes[i] = binary.charCodeAt(i);
  }
  return bytes;
}

export function drawImageOp(this: any, canvas: SkCanvas, image: SkImage, op: LayerImageOp): void {
  const imageWithDimensions = image as SkImage & { width?: unknown; height?: unknown };
  const widthMember = imageWithDimensions.width;
  const heightMember = imageWithDimensions.height;
  const imageWidth = typeof widthMember === 'function'
    ? (widthMember as () => number).call(image)
    : typeof widthMember === 'number'
      ? widthMember
      : null;
  const imageHeight = typeof heightMember === 'function'
    ? (heightMember as () => number).call(image)
    : typeof heightMember === 'number'
      ? heightMember
      : null;
  if (!this.boundsAreDrawable(op.bbox)) {
    this.unsupportedOps.add('image:invalidBounds');
    return;
  }
  if (
    imageWidth === null
    || imageHeight === null
    || !Number.isFinite(imageWidth)
    || !Number.isFinite(imageHeight)
    || imageWidth <= 0
    || imageHeight <= 0
  ) {
    const paint = new this.canvasKit.Paint();
    paint.setAntiAlias?.(true);
    try {
      canvas.drawImage(image, op.bbox.x, op.bbox.y, paint);
      this.unsupportedOps.add('image:dimensionUnavailable');
    } finally {
      paint.delete?.();
    }
    return;
  }

  const crop = canvasKitImageSourceRect(
    imageWidth,
    imageHeight,
    op.crop,
    op.originalSizeHu,
  );
  const opacity = Number.isFinite(op.opacity) ? Math.max(0, Math.min(1, op.opacity ?? 1)) : 1;
  const drawImage = (dstX: number, dstY: number, dstW: number, dstH: number) => {
    const src = crop
      ? this.canvasKit.XYWHRect(crop.x, crop.y, crop.width, crop.height)
      : this.canvasKit.XYWHRect(0, 0, imageWidth, imageHeight);
    this.drawImageRect(canvas, image, src, this.canvasKit.XYWHRect(dstX, dstY, dstW, dstH), opacity);
  };

  const fillMode = op.fillMode ?? 'fitToSize';
  if (canvasKitImageFillModeStretches(fillMode)) {
    drawImage(op.bbox.x, op.bbox.y, op.bbox.width, op.bbox.height);
    return;
  }

  let tileWidth = op.originalSize?.width ?? imageWidth;
  let tileHeight = op.originalSize?.height ?? imageHeight;
  if (!Number.isFinite(tileWidth) || tileWidth <= 0) tileWidth = imageWidth;
  if (!Number.isFinite(tileHeight) || tileHeight <= 0) tileHeight = imageHeight;

  canvas.save();
  try {
    canvas.clipRect(this.rect(op.bbox), this.canvasKit.ClipOp?.Intersect ?? 0, true);
    if (canvasKitImageFillModeTiles(fillMode)) {
      this.drawTiledImage(canvas, op.bbox, fillMode, tileWidth, tileHeight, drawImage);
    } else {
      const placed = canvasKitImagePlacement(fillMode, op.bbox, tileWidth, tileHeight);
      drawImage(placed.x, placed.y, tileWidth, tileHeight);
    }
  } finally {
    canvas.restore();
  }
}

export function drawImageRect(this: any, canvas: SkCanvas, image: SkImage, source: Rect, dest: Rect, opacity = 1): void {
  const paint = new this.canvasKit.Paint();
  paint.setAntiAlias?.(true);
  if (opacity < 1) {
    paint.setAlphaf(opacity);
  }
  try {
    canvas.drawImageRect(image, source, dest, paint);
  } finally {
    paint.delete?.();
  }
}

export function drawTiledImage(
  this: any,
  canvas: SkCanvas,
  bbox: LayerBounds,
  fillMode: string,
  tileWidth: number,
  tileHeight: number,
  drawImage: (dstX: number, dstY: number, dstW: number, dstH: number) => void,
): void {
  const maxTileDraws = MAX_IMAGE_TILE_DRAWS;
  let tileDraws = 0;
  const drawTile = (x: number, y: number) => {
    if (tileDraws >= maxTileDraws) return;
    drawImage(x, y, tileWidth, tileHeight);
    tileDraws += 1;
  };

  if (fillMode === 'tileAll') {
    for (let y = bbox.y; y < bbox.y + bbox.height && tileDraws < maxTileDraws; y += tileHeight) {
      for (let x = bbox.x; x < bbox.x + bbox.width && tileDraws < maxTileDraws; x += tileWidth) {
        drawTile(x, y);
      }
    }
  } else if (fillMode === 'tileHorzTop' || fillMode === 'tileHorzBottom') {
    const y = fillMode === 'tileHorzTop' ? bbox.y : bbox.y + bbox.height - tileHeight;
    for (let x = bbox.x; x < bbox.x + bbox.width && tileDraws < maxTileDraws; x += tileWidth) {
      drawTile(x, y);
    }
  } else {
    const x = fillMode === 'tileVertLeft' ? bbox.x : bbox.x + bbox.width - tileWidth;
    for (let y = bbox.y; y < bbox.y + bbox.height && tileDraws < maxTileDraws; y += tileHeight) {
      drawTile(x, y);
    }
  }

  if (tileDraws >= maxTileDraws) {
    this.unsupportedOps.add('image:tileLimit');
  }
}

export function withImageTransform(
  this: any,
  canvas: SkCanvas,
  bounds: LayerBounds,
  transform: LayerImageOp['transform'],
  draw: () => void,
): void {
  const rotation = transform?.rotation ?? 0;
  const horzFlip = transform?.horzFlip ?? false;
  const vertFlip = transform?.vertFlip ?? false;
  if (rotation === 0 && !horzFlip && !vertFlip) {
    draw();
    return;
  }

  const cx = bounds.x + bounds.width / 2;
  const cy = bounds.y + bounds.height / 2;
  canvas.save();
  try {
    if (horzFlip || vertFlip) {
      canvas.translate(cx, cy);
      canvas.scale(horzFlip ? -1 : 1, vertFlip ? -1 : 1);
      canvas.translate(-cx, -cy);
    }
    if (rotation !== 0) {
      canvas.rotate(rotation, cx, cy);
    }
    draw();
  } finally {
    canvas.restore();
  }
}

export function recordImageCoverageGaps(this: any, op: LayerImageOp): void {
  if (op.bakedWatermark) return;
  if (op.effect && op.effect !== 'realPic') {
    this.unsupportedOps.add(`imageEffect:${op.effect}`);
  }
  if ((op.brightness ?? 0) !== 0 || (op.contrast ?? 0) !== 0) {
    this.unsupportedOps.add('imageEffect:brightnessContrast');
  }
}

export function imageForOp(this: any, op: LayerImageOp): SkImage | null {
  const base64 = op.base64 ?? '';
  if (!base64) {
    return null;
  }
  if (base64.length > CANVASKIT_MAX_ENCODED_IMAGE_BASE64_LENGTH) {
    this.recordImageFailure(op, 'encodedImageRejected', null);
    return null;
  }
  const key = canvasKitImageCacheKey(op, this.documentGeneration);
  if (!key) {
    this.recordImageFailure(op, 'cacheKeyMissing', null);
    return null;
  }
  const cached = this.imageCache.get(key);
  if (cached) {
    this.imageCache.delete(key);
    this.imageCache.set(key, cached);
    this.imageCacheHits += 1;
    return cached.image;
  }
  const cachedFailure = this.imageDecodeFailures.get(key);
  if (cachedFailure) {
    this.imageCacheHits += 1;
    this.imageFailureCacheHits += 1;
    this.recordImageFailure(op, cachedFailure, key);
    return null;
  }
  this.imageCacheMisses += 1;
  let bytes: Uint8Array;
  try {
    bytes = base64ToBytes(base64);
  } catch {
    this.recordImageFailure(op, 'base64DecodeFailed', key);
    return null;
  }
  const encodedHeader = replayableEncodedImageHeader(bytes);
  if (!encodedHeader) {
    this.recordImageFailure(op, 'encodedImageRejected', key);
    return null;
  }
  let image: SkImage | null = null;
  try {
    image = this.canvasKit.MakeImageFromEncoded(bytes);
  } catch {
    this.recordImageFailure(op, 'imageDecodeFailed', key);
    return null;
  }
  if (!image) {
    this.recordImageFailure(op, 'imageDecodeFailed', key);
    return null;
  }
  const imageWithDimensions = image as SkImage & { width?: (() => number) | number; height?: (() => number) | number };
  const width = typeof imageWithDimensions.width === 'function' ? imageWithDimensions.width() : imageWithDimensions.width;
  const height = typeof imageWithDimensions.height === 'function' ? imageWithDimensions.height() : imageWithDimensions.height;
  const decodedPixels = typeof width === 'number' && typeof height === 'number'
    ? width * height
    : Number.POSITIVE_INFINITY;
  if (!decodedImageMatchesEncodedHeader(encodedHeader, width, height)) {
    image.delete?.();
    this.recordImageFailure(op, 'decodedDimensionsMismatch', key);
    return null;
  }
  while (this.imageCache.size >= MAX_IMAGE_CACHE_ENTRIES
    || this.imageCachePixels + decodedPixels > MAX_IMAGE_CACHE_PIXELS) {
    const oldestKey = this.imageCache.keys().next().value as string | undefined;
    if (oldestKey === undefined) break;
    const oldest = this.imageCache.get(oldestKey);
    oldest?.image.delete?.();
    this.imageCache.delete(oldestKey);
    this.imageCachePixels = Math.max(0, this.imageCachePixels - (oldest?.pixels ?? 0));
    this.imageCacheEvictions += 1;
  }
  this.imageCache.set(key, { image, pixels: decodedPixels });
  this.imageCachePixels += decodedPixels;
  return image;
}

export function recordImageFailure(
  this: any,
  op: LayerImageOp,
  reason: CanvasKitImageFailureReason,
  key: string | null,
): void {
  if (key) {
    if (!this.imageDecodeFailures.has(key)
      && this.imageDecodeFailures.size >= MAX_IMAGE_FAILURE_CACHE_ENTRIES) {
      const oldestKey = this.imageDecodeFailures.keys().next().value as string | undefined;
      if (oldestKey !== undefined) this.imageDecodeFailures.delete(oldestKey);
    }
    this.imageDecodeFailures.set(key, reason);
  }

  const sourceImageKey = boundedCanvasKitSourceImageKey(op.sourceImageKey);
  const imageRef = (
    (typeof op.imageRef === 'number' && Number.isSafeInteger(op.imageRef))
    || (
      typeof op.imageRef === 'string'
      && op.imageRef.length > 0
      && op.imageRef.length <= 256
      && !/[\u0000-\u001f\u007f]/.test(op.imageRef)
    )
  ) ? op.imageRef : null;
  const source = sourceImageKey
    ? 'sourceKey'
    : imageRef !== null
      ? 'resource'
      : op.base64
        ? 'inline'
        : 'missing';
  const diagnosticKey = key
    ?? `${source}:${sourceImageKey ?? String(imageRef ?? op.base64?.length ?? 0)}:${reason}`;
  if (this.currentImageFailures.has(diagnosticKey)
    || this.currentImageFailures.size >= MAX_IMAGE_FAILURE_CACHE_ENTRIES) {
    return;
  }
  this.currentImageFailures.set(diagnosticKey, {
    source,
    sourceImageKey,
    imageRef,
    reason,
  });
}
