import CanvasKitInit from 'canvaskit-wasm';
import type {
  Canvas,
  CanvasKit,
  Color,
  Font,
  FontMgr,
  Image as SkImage,
  Paint,
  Path,
  PathBuilder,
  Rect,
  Surface,
  Typeface,
} from 'canvaskit-wasm';
import canvaskitWasmUrl from '@/view/canvaskit-wasm-url';

import type {
  LayerBounds,
  LayerCharOverlapOp,
  LayerClipNode,
  LayerEllipseOp,
  LayerEquationLayoutBox,
  LayerEquationOp,
  LayerFormObjectOp,
  LayerAffineTransform,
  LayerGlyphOutlineOp,
  LayerGlyphRunOp,
  LayerFontResources,
  LayerImageOp,
  LayerInfo,
  LayerLeafNode,
  LayerLineOp,
  LayerNode,
  LayerPageBackgroundOp,
  LayerPaintOp,
  LayerPathCommand,
  LayerPathOp,
  LayerPlaceholderOp,
  LayerRectangleOp,
  LayerRenderProfile,
  LayerResources,
  LayerShapeStyle,
  LayerStrokeDash,
  LayerTabLeaderOp,
  LayerTextControlMarkOp,
  LayerTextDecorationOp,
  LayerTextRunOp,
  PageInfo,
  PageLayerTree,
} from '@/core/types';
import {
  DEFAULT_CANVASKIT_SURFACE_REQUEST,
  type CanvasKitRenderMode,
  type CanvasKitSurfacePreference,
  type CanvasKitSurfaceRequest,
} from './render-backend';
import { canvaskitClipRightPad } from './canvaskit/policy';
import {
  CanvasKitGlyphRunFontCache,
  drawCanvasKitGlyphRun,
} from './canvaskit/glyph-run-fonts';
import {
  selectLayerTextVariantsForLeaf,
  staticSvgPathLayersAreReplayable,
} from './canvaskit/text-variant-selection';
import {
  CANVASKIT_REPLAY_PLANES,
  type CanvasKitReplayPlane,
  layerPaintOpReplayPlane,
} from './canvaskit/replay-plane';
import { isExpectedCanvasKitUnsupportedOp } from './canvaskit/diagnostics';
import { layerResourceKeyMatches } from './canvaskit/resource-key';
import {
  glyphOutlinePayloadResourceKey,
  glyphOutlinePayloadStatus,
} from './glyph-outline-payload-status';
import { parseStaticSvgPathLayers, type StaticSvgPathLayer } from './static-svg-path-layers';
import { loadLocalFontBytesFor, localFontFaceKey, resolveLocalFont, type LocalFontRecord } from '@/core/local-fonts';
import type { CanvasKitBundledFontSource } from '@/core/font-loader';
import { readBoundedResponseArrayBuffer } from './canvaskit/bounded-response';
import * as _equation from './canvaskit/equation';
import type { EquationRenderBudget } from './canvaskit/equation';
import * as _imageCache from './canvaskit/image-cache';
import {
  base64ToBytes,
  MAX_IMAGE_CACHE_ENTRIES,
  MAX_IMAGE_CACHE_PIXELS,
} from './canvaskit/image-cache';
import * as _shapes from './canvaskit/shapes';
import * as _colors from './canvaskit/colors';
import * as _textRun from './canvaskit/text-run';
import {
  MAX_FONT_SUBSTITUTION_DIAGNOSTICS,
  normalizedFontFamily,
  OLD_HANGUL_FONT_FAMILY,
  primaryFontFamily,
} from './canvaskit/text-run';

type CanvasKitApi = CanvasKit;
type SkCanvas = Canvas;
type SkPaint = Paint;
type SkSurface = Surface;

export interface CanvasKitLayerRendererOptions {
  defaultFontUrl?: string;
  symbolFallbackFontUrl?: string;
  oldHangulFontUrl?: string;
  requirePreparedFontFamilies?: boolean;
}

type MutablePath = Path & Pick<PathBuilder, 'arcToRotated' | 'close' | 'cubicTo' | 'lineTo' | 'moveTo'>;
type LayerColorGraph = NonNullable<NonNullable<LayerGlyphOutlineOp['colorLayers']>['paintGraph']>;
type LayerColorGraphNode = NonNullable<LayerColorGraph['nodes']>[number];
interface CanvasKitSurfaceTarget {
  surface: SkSurface;
  canvas: HTMLCanvasElement;
}

export interface CanvasKitLocalTypeface {
  typeface: Typeface | null;
  fontManager: FontMgr | null;
  fontFamily: string | null;
}

export interface CanvasKitReplayFeatureCounts {
  dashedStrokes: number;
  glyphRuns: number;
  verticalPresentationPunctuation: number;
  verticalTextRuns: number;
}

export interface CanvasKitRenderDiagnostics {
  mode: CanvasKitRenderMode;
  surfacePreference: CanvasKitSurfacePreference;
  surfaceBackend: 'default' | 'software' | null;
  surfaceFallbackReason: string | null;
  lastRenderCompleted: boolean;
  lastUnsupportedOps: string[];
  lastExpectedUnsupportedOps: string[];
  lastUnexpectedUnsupportedOps: string[];
  lastRenderError: string | null;
  passesRuntimeReadinessGate: boolean;
  readinessBlockers: CanvasKitReadinessBlocker[];
  hiddenCanvas2dOverlayUsed: false;
  lastRenderDurationMs: number | null;
  renderCount: number;
  imageCacheEntries: number;
  imageCacheLimit: number;
  imageCachePixels: number;
  imageCachePixelLimit: number;
  imageCacheHits: number;
  imageCacheMisses: number;
  imageCacheEvictions: number;
  imageFailureCacheHits: number;
  imageFailures: CanvasKitImageFailureDiagnostic[];
  localTypefaceCount: number;
  localTypefaceLoadFailureCount: number;
  localTypefacePendingCount: number;
  bundledTypefaceCount: number;
  bundledTypefaceLoadFailureCount: number;
  glyphRunFontBlobCount: number;
  glyphRunFontBlobBytes: number;
  glyphRunTypefaceCount: number;
  glyphRunFontCount: number;
  fontSubstitutionLimit: number;
  unregisteredFontFallbacks: number;
  fontSubstitutions: CanvasKitFontSubstitutionDiagnostic[];
  replayFeatureCounts: CanvasKitReplayFeatureCounts;
}

export interface CanvasKitFontSubstitutionDiagnostic {
  requestedFamily: string;
  resolvedFamily: string;
  source: 'unregisteredDefault' | 'missingGlyphDefault' | 'missingGlyphSymbol' | 'oldHangul';
  kind: 'unregisteredFallback' | 'glyphCoverageFallback';
}

export type CanvasKitImageFailureReason =
  | 'dataMissing'
  | 'cacheKeyMissing'
  | 'base64DecodeFailed'
  | 'encodedImageRejected'
  | 'imageDecodeFailed'
  | 'decodedDimensionsMismatch';

export interface CanvasKitImageFailureDiagnostic {
  source: 'sourceKey' | 'resource' | 'inline' | 'missing';
  sourceImageKey: string | null;
  imageRef: number | string | null;
  reason: CanvasKitImageFailureReason;
}

export type CanvasKitReadinessBlocker =
  | 'renderNotCompleted'
  | 'renderError'
  | 'unexpectedUnsupportedOps'
  | 'imageReplayFailure'
  | 'localFontsPending';

export class CanvasKitLayerRenderer {
  private static readonly MAX_SVG_GLYPH_CACHE_ENTRIES = 128;
  private static readonly MAX_BITMAP_GLYPH_BASE64_LENGTH = Math.ceil(4 * 1024 * 1024 / 3) * 4;
  private static readonly MAX_STATIC_SVG_GLYPH_BYTES = 1024 * 1024;
  private static readonly MAX_PLACEHOLDER_DASH_SEGMENTS_PER_AXIS = 2048;
  private static readonly MAX_TEXT_SPECIAL_VISUAL_ITEMS = 4096;
  private static readonly MAX_BUNDLED_FONT_BYTES = 32 * 1024 * 1024;

  private readonly imageCache = new Map<string, { image: SkImage; pixels: number }>();
  private readonly imageDecodeFailures = new Map<string, CanvasKitImageFailureReason>();
  private readonly currentImageFailures = new Map<string, CanvasKitImageFailureDiagnostic>();
  private readonly svgGlyphPathCache = new Map<string, StaticSvgPathLayer[]>();
  private readonly svgGlyphParseFailures = new Set<string>();
  private readonly localTypefaces = new Map<string, CanvasKitLocalTypeface>();
  private readonly localTypefaceLoadFailures = new Set<string>();
  private readonly localTypefacePending = new Map<string, number>();
  private readonly bundledTypefaces = new Map<string, CanvasKitLocalTypeface>();
  private readonly bundledTypefaceAliases = new Map<string, CanvasKitLocalTypeface>();
  private readonly bundledTypefaceLoadFailures = new Set<string>();
  private readonly currentFontSubstitutions = new Map<string, CanvasKitFontSubstitutionDiagnostic>();
  private readonly bundledFontRequests = new Set<AbortController>();
  private readonly glyphRunFonts: CanvasKitGlyphRunFontCache;
  private readonly unsupportedOps = new Set<string>();
  private surfaceBackend: 'default' | 'software' | null = null;
  private surfaceFallbackReason: string | null = null;
  private lastRenderError: string | null = null;
  private lastRenderCompleted = false;
  private lastRenderDurationMs: number | null = null;
  private renderCount = 0;
  private imageCacheHits = 0;
  private imageCacheMisses = 0;
  private imageCacheEvictions = 0;
  private imageFailureCacheHits = 0;
  private imageCachePixels = 0;
  private currentResources: LayerResources | undefined;
  private currentFontResources: LayerFontResources | undefined;
  private currentShowParagraphMarks = false;
  private currentShowControlCodes = false;
  private currentReplayFeatureCounts: CanvasKitReplayFeatureCounts = {
    dashedStrokes: 0,
    glyphRuns: 0,
    verticalPresentationPunctuation: 0,
    verticalTextRuns: 0,
  };
  private selectedTextVariantOps = new WeakSet<LayerPaintOp>();
  private documentGeneration = 0;
  private disposed = false;

  private constructor(
    private readonly canvasKit: CanvasKitApi,
    private readonly renderMode: CanvasKitRenderMode,
    private readonly surfaceRequest: CanvasKitSurfaceRequest,
    private readonly defaultTypeface: Typeface | null,
    private readonly symbolFallbackTypeface: Typeface | null,
    private readonly defaultFontManager: FontMgr | null = null,
    private readonly defaultFontFamily: string | null = null,
    private readonly defaultFontUrl: string = 'fonts/NotoSansKR-Regular.woff2',
    private readonly requirePreparedFontFamilies: boolean = false,
    private readonly oldHangulTypeface: CanvasKitLocalTypeface | null = null,
    private readonly oldHangulFontUrl: string = 'fonts/SourceHanSerifK-OldHangul-subset.woff2',
  ) {
    this.glyphRunFonts = new CanvasKitGlyphRunFontCache(canvasKit);
  }

  static async create(
    renderMode: CanvasKitRenderMode = 'default',
    surfaceRequest: CanvasKitSurfaceRequest | CanvasKitSurfacePreference = DEFAULT_CANVASKIT_SURFACE_REQUEST,
    options: CanvasKitLayerRendererOptions = {},
  ): Promise<CanvasKitLayerRenderer> {
    const canvasKit = await CanvasKitInit({
      locateFile: (file) => file === 'canvaskit.wasm' ? canvaskitWasmUrl : file,
    });
    const resolvedSurfaceRequest = typeof surfaceRequest === 'string'
      ? { ...DEFAULT_CANVASKIT_SURFACE_REQUEST, preference: surfaceRequest, requested: surfaceRequest }
      : surfaceRequest;
    // 기본 Noto는 local face가 없거나 등록에 실패한 text run의 안정적인 CJK fallback이다.
    let defaultTypeface: Typeface | null = null;
    let defaultFontManager: FontMgr | null = null;
    let defaultFontFamily: string | null = null;
    const defaultFontUrl = options.defaultFontUrl ?? 'fonts/NotoSansKR-Regular.woff2';
    try {
      const response = await fetch(defaultFontUrl);
      if (response.ok) {
        const bytes = await readBoundedResponseArrayBuffer(response, {
          maxBytes: CanvasKitLayerRenderer.MAX_BUNDLED_FONT_BYTES,
        });
        defaultTypeface = canvasKit.Typeface.MakeFreeTypeFaceFromData(bytes)
          ?? canvasKit.Typeface.MakeTypefaceFromData(bytes);
        defaultFontManager = canvasKit.FontMgr.FromData(bytes);
        if (defaultFontManager && defaultFontManager.countFamilies() > 0) {
          defaultFontFamily = defaultFontManager.getFamilyName(0);
        }
      }
    } catch (error) {
      console.warn('[CanvasKitLayerRenderer] 기본 CJK 폰트 로딩 실패:', error);
    }
    let symbolFallbackTypeface: Typeface | null = null;
    const symbolFallbackFontUrl = options.symbolFallbackFontUrl
      ?? 'fonts/D2Coding-Regular.woff2';
    try {
      const response = await fetch(symbolFallbackFontUrl);
      if (response.ok) {
        const bytes = await readBoundedResponseArrayBuffer(response, {
          maxBytes: CanvasKitLayerRenderer.MAX_BUNDLED_FONT_BYTES,
        });
        symbolFallbackTypeface = canvasKit.Typeface.MakeFreeTypeFaceFromData(bytes)
          ?? canvasKit.Typeface.MakeTypefaceFromData(bytes);
      }
    } catch (error) {
      console.warn('[CanvasKitLayerRenderer] 기호 폴백 폰트 로딩 실패:', error);
    }
    let oldHangulTypeface: CanvasKitLocalTypeface | null = null;
    const oldHangulFontUrl = options.oldHangulFontUrl
      ?? 'fonts/SourceHanSerifK-OldHangul-subset.woff2';
    let oldHangulNativeTypeface: Typeface | null = null;
    let oldHangulFontManager: FontMgr | null = null;
    try {
      const response = await fetch(oldHangulFontUrl);
      if (response.ok) {
        const bytes = await readBoundedResponseArrayBuffer(response, {
          maxBytes: CanvasKitLayerRenderer.MAX_BUNDLED_FONT_BYTES,
        });
        oldHangulNativeTypeface = canvasKit.Typeface.MakeFreeTypeFaceFromData(bytes)
          ?? canvasKit.Typeface.MakeTypefaceFromData(bytes);
        oldHangulFontManager = canvasKit.FontMgr.FromData(bytes.slice(0));
        const fontFamily = oldHangulFontManager && oldHangulFontManager.countFamilies() > 0
          ? oldHangulFontManager.getFamilyName(0)
          : OLD_HANGUL_FONT_FAMILY;
        if (oldHangulNativeTypeface || oldHangulFontManager) {
          oldHangulTypeface = {
            typeface: oldHangulNativeTypeface,
            fontManager: oldHangulFontManager,
            fontFamily,
          };
          oldHangulNativeTypeface = null;
          oldHangulFontManager = null;
        }
      }
    } catch (error) {
      oldHangulNativeTypeface?.delete?.();
      oldHangulFontManager?.delete?.();
      console.warn('[CanvasKitLayerRenderer] 옛한글 shaping 폰트 로딩 실패:', error);
    }
    return new CanvasKitLayerRenderer(
      canvasKit,
      renderMode,
      resolvedSurfaceRequest,
      defaultTypeface,
      symbolFallbackTypeface,
      defaultFontManager,
      defaultFontFamily,
      defaultFontUrl,
      options.requirePreparedFontFamilies ?? false,
      oldHangulTypeface,
      oldHangulFontUrl,
    );
  }

  /** Auto selection에서 승인된 문서 폰트를 첫 replay 전에 native Typeface로 등록한다. */
  async prepareBundledFonts(sources: readonly CanvasKitBundledFontSource[]): Promise<number> {
    if (this.disposed || sources.length === 0) return 0;
    const generation = this.documentGeneration;
    let registered = 0;
    for (const source of sources) {
      if (!source.url || source.aliases.length === 0) continue;
      const requiresShapingManager = source.aliases.some(alias => (
        normalizedFontFamily(alias) === normalizedFontFamily(OLD_HANGUL_FONT_FAMILY)
      ));
      let prepared = source.url === this.oldHangulFontUrl && this.oldHangulTypeface
        ? this.oldHangulTypeface
        : source.url === this.defaultFontUrl && (this.defaultTypeface || this.defaultFontManager)
          ? {
            typeface: this.defaultTypeface,
            fontManager: this.defaultFontManager,
            fontFamily: this.defaultFontFamily,
          }
          : this.bundledTypefaces.get(source.url) ?? null;
      if (prepared && requiresShapingManager && !prepared.fontManager) {
        throw new Error(`CanvasKit shaping font source 준비 실패: ${source.url}`);
      }
      if (!prepared) {
        if (this.bundledTypefaceLoadFailures.has(source.url)) {
          throw new Error(`CanvasKit font source 준비 실패: ${source.url}`);
        }
        let typeface: Typeface | null = null;
        let fontManager: FontMgr | null = null;
        const request = new AbortController();
        this.bundledFontRequests.add(request);
        try {
          if (this.disposed || generation !== this.documentGeneration) {
            throw new Error('문서 교체로 CanvasKit font 준비가 취소되었습니다');
          }
          const response = await fetch(source.url, { signal: request.signal });
          if (!response.ok) {
            throw new Error(`HTTP ${response.status}`);
          }
          const bytes = await readBoundedResponseArrayBuffer(response, {
            maxBytes: CanvasKitLayerRenderer.MAX_BUNDLED_FONT_BYTES,
            signal: request.signal,
            isCancelled: () => this.disposed || generation !== this.documentGeneration,
            cancelledMessage: '문서 교체로 CanvasKit font 준비가 취소되었습니다',
          });
          if (this.disposed || generation !== this.documentGeneration) {
            throw new Error('문서 교체로 CanvasKit font 준비가 취소되었습니다');
          }
          typeface = this.canvasKit.Typeface.MakeFreeTypeFaceFromData(bytes)
            ?? this.canvasKit.Typeface.MakeTypefaceFromData(bytes);
          fontManager = this.canvasKit.FontMgr.FromData(bytes.slice(0));
          if ((!typeface && !fontManager) || (requiresShapingManager && !fontManager)) {
            throw new Error('CanvasKit이 font payload를 해석하지 못했습니다');
          }
          const fontFamily = fontManager && fontManager.countFamilies() > 0
            ? fontManager.getFamilyName(0)
            : source.aliases[0];
          prepared = { typeface, fontManager, fontFamily };
          this.bundledTypefaces.set(source.url, prepared);
          registered += 1;
          typeface = null;
          fontManager = null;
        } catch (error) {
          typeface?.delete?.();
          fontManager?.delete?.();
          if (!request.signal.aborted
            && !this.disposed && generation === this.documentGeneration) {
            this.bundledTypefaceLoadFailures.add(source.url);
          }
          throw new Error(`CanvasKit font source 준비 실패 (${source.url}): ${error}`);
        } finally {
          this.bundledFontRequests.delete(request);
        }
      }
      for (const alias of source.aliases) {
        const key = normalizedFontFamily(alias);
        if (key) this.bundledTypefaceAliases.set(key, prepared);
      }
      await Promise.resolve();
    }
    return registered;
  }

  /** 현재 문서가 실제로 사용하는 설치 글꼴만 CanvasKit native 객체로 등록한다. */
  async prepareLocalFonts(fontNames: readonly string[] | undefined): Promise<number> {
    if (this.disposed || !fontNames?.length) return 0;
    const generation = this.documentGeneration;
    const pendingRecords = new Map<string, LocalFontRecord>();
    for (const fontName of fontNames) {
      const record = resolveLocalFont(fontName);
      const faceKey = record ? localFontFaceKey(record) : '';
      if (!record || !faceKey || this.localTypefaces.has(faceKey)
        || this.localTypefaceLoadFailures.has(faceKey) || this.localTypefacePending.has(faceKey)) continue;
      pendingRecords.set(faceKey, record);
      this.localTypefacePending.set(faceKey, generation);
    }

    let registered = 0;
    try {
      const bytesByFace = await loadLocalFontBytesFor([...pendingRecords.values()].map(record => record.fullName));
      for (const [faceKey, record] of pendingRecords) {
        const bytes = bytesByFace.get(faceKey);
        if (this.disposed || generation !== this.documentGeneration) return registered;
        if (this.localTypefaces.has(faceKey) || this.localTypefaceLoadFailures.has(faceKey)) continue;
        if (!bytes) {
          this.localTypefaceLoadFailures.add(faceKey);
          continue;
        }
        let typeface: Typeface | null = null;
        let fontManager: FontMgr | null = null;
        try {
          typeface = this.canvasKit.Typeface.MakeFreeTypeFaceFromData(bytes)
            ?? this.canvasKit.Typeface.MakeTypefaceFromData(bytes);
          fontManager = this.canvasKit.FontMgr.FromData(bytes.slice(0));
          if (!typeface && !fontManager) {
            this.localTypefaceLoadFailures.add(faceKey);
            continue;
          }
          const fontFamily = fontManager && fontManager.countFamilies() > 0
            ? fontManager.getFamilyName(0)
            : record.family;
          this.localTypefaces.set(faceKey, { typeface, fontManager, fontFamily });
          registered += 1;
        } catch (error) {
          typeface?.delete?.();
          fontManager?.delete?.();
          this.localTypefaceLoadFailures.add(faceKey);
          console.warn(`[CanvasKitLayerRenderer] ${record.displayName} local Typeface 등록 실패:`, error);
        }
        // native font parsing은 동기 작업이므로 face 사이에서 paint/event loop에 양보한다.
        await new Promise<void>(resolve => window.setTimeout(resolve, 0));
      }
    } finally {
      for (const faceKey of pendingRecords.keys()) {
        if (this.localTypefacePending.get(faceKey) === generation) {
          this.localTypefacePending.delete(faceKey);
        }
      }
    }
    return registered;
  }

  renderPage(
    tree: PageLayerTree,
    targetCanvas: HTMLCanvasElement,
    scale: number,
    pageInfo?: PageInfo,
  ): HTMLCanvasElement {
    if (this.disposed) {
      throw new Error('CanvasKit renderer가 이미 dispose되었습니다');
    }
    this.unsupportedOps.clear();
    this.currentImageFailures.clear();
    this.currentFontSubstitutions.clear();
    this.resetReplayFeatureCounts();
    this.lastRenderError = null;
    this.lastRenderCompleted = false;
    let surface: SkSurface | null = null;
    let renderedCanvas = targetCanvas;
    const renderStartedAt = performance.now();
    try {
      const surfaceTarget = this.makeSurface(targetCanvas);
      surface = surfaceTarget.surface;
      renderedCanvas = surfaceTarget.canvas;
      const canvas = surface.getCanvas();
      this.currentResources = tree.resources;
      this.currentFontResources = tree.fontResources;
      this.glyphRunFonts.registerResources(tree.fontResources, tree.resources);
      this.currentShowParagraphMarks = tree.outputOptions?.showParagraphMarks === true;
      this.currentShowControlCodes = tree.outputOptions?.showControlCodes === true;
      if (this.currentShowControlCodes) {
        this.unsupportedOps.add('viewOption:showControlCodes');
      }
      this.selectedTextVariantOps = new WeakSet<LayerPaintOp>();
      this.selectTextVariants(tree.root);
      let hasPageBackground = false;
      const stack: LayerNode[] = [tree.root];
      while (stack.length > 0 && !hasPageBackground) {
        const node = stack.pop()!;
        if (node.kind === 'group') {
          stack.push(...node.children);
        } else if (node.kind === 'clipRect') {
          stack.push(node.child);
        } else {
          hasPageBackground = node.ops.some((op) => op.type === 'pageBackground');
        }
      }
      canvas.save();
      canvas.clear(this.color(hasPageBackground ? 'rgba(0,0,0,0)' : '#ffffff'));
      canvas.scale(scale, scale);
      const rightOverflowSlop =
        tree.outputOptions?.showParagraphMarks || tree.outputOptions?.showControlCodes ? 48 : undefined;
      for (const replayPlane of CANVASKIT_REPLAY_PLANES) {
        this.renderNode(canvas, tree.root, tree.profile ?? 'screen', replayPlane, null, rightOverflowSlop);
      }
      if (pageInfo) {
        const paint = this.makeStrokePaint('#c0c0c0', 0.3);
        const left = pageInfo.marginLeft;
        const top = pageInfo.marginHeader + pageInfo.marginTop;
        const right = pageInfo.width - pageInfo.marginRight;
        const bottom = pageInfo.height - pageInfo.marginFooter - pageInfo.marginBottom;
        const length = 15;
        canvas.drawLine(left, top - length, left, top, paint);
        canvas.drawLine(left, top, left - length, top, paint);
        canvas.drawLine(right + length, top, right, top, paint);
        canvas.drawLine(right, top, right, top - length, paint);
        canvas.drawLine(left - length, bottom, left, bottom, paint);
        canvas.drawLine(left, bottom, left, bottom + length, paint);
        canvas.drawLine(right, bottom + length, right, bottom, paint);
        canvas.drawLine(right, bottom, right + length, bottom, paint);
        paint.delete();
      }
      canvas.restore();
      surface.flush();
      this.lastRenderCompleted = true;
    } catch (error) {
      this.recordRenderFailure(error);
      throw error;
    } finally {
      surface?.delete();
      this.currentResources = undefined;
      this.currentFontResources = undefined;
      this.currentShowParagraphMarks = false;
      this.currentShowControlCodes = false;
      this.lastRenderDurationMs = performance.now() - renderStartedAt;
      this.renderCount += 1;
    }
    return renderedCanvas;
  }

  releaseLayerTree(_tree: PageLayerTree): void {
    /* Per-tree native picture interning is not implemented yet. */
  }

  resetDocumentResources(): void {
    this.documentGeneration += 1;
    this.cancelDocumentPreparation();
    for (const entry of this.imageCache.values()) entry.image?.delete?.();
    this.imageCache.clear();
    this.imageCachePixels = 0;
    this.imageDecodeFailures.clear();
    this.svgGlyphPathCache.clear();
    this.svgGlyphParseFailures.clear();
    this.currentResources = undefined;
    this.currentFontResources = undefined;
    this.selectedTextVariantOps = new WeakSet<LayerPaintOp>();
    this.glyphRunFonts.clear();
    for (const { typeface, fontManager } of this.localTypefaces.values()) {
      typeface?.delete?.();
      fontManager?.delete?.();
    }
    this.localTypefaces.clear();
    this.localTypefaceLoadFailures.clear();
    this.localTypefacePending.clear();
    for (const { typeface, fontManager } of this.bundledTypefaces.values()) {
      typeface?.delete?.();
      fontManager?.delete?.();
    }
    this.bundledTypefaces.clear();
    this.bundledTypefaceAliases.clear();
    this.bundledTypefaceLoadFailures.clear();
    this.imageCacheHits = 0;
    this.imageCacheMisses = 0;
    this.imageCacheEvictions = 0;
    this.imageFailureCacheHits = 0;
    this.currentImageFailures.clear();
    this.currentFontSubstitutions.clear();
    this.resetReplayFeatureCounts();
    this.renderCount = 0;
    this.lastRenderDurationMs = null;
  }

  cancelDocumentPreparation(): void {
    for (const request of this.bundledFontRequests) {
      request.abort(new Error('문서 교체로 CanvasKit font 준비가 취소되었습니다'));
    }
    this.bundledFontRequests.clear();
  }

  diagnostics(): CanvasKitRenderDiagnostics {
    const lastUnsupportedOps = [...this.unsupportedOps].sort();
    const lastExpectedUnsupportedOps = lastUnsupportedOps.filter(isExpectedCanvasKitUnsupportedOp);
    const lastUnexpectedUnsupportedOps = lastUnsupportedOps.filter(
      (op) => !isExpectedCanvasKitUnsupportedOp(op),
    );
    const surfaceFallbackReason = this.surfaceFallbackReason ?? this.surfaceRequest.unsupportedReason ?? null;
    const fontSubstitutions = [...this.currentFontSubstitutions.values()]
      .map((substitution) => ({ ...substitution }));
    const glyphRunFontDiagnostics = this.glyphRunFonts.diagnostics();
    const readinessBlockers: CanvasKitReadinessBlocker[] = [];
    if (!this.lastRenderCompleted) readinessBlockers.push('renderNotCompleted');
    if (this.lastRenderError !== null) readinessBlockers.push('renderError');
    if (lastUnexpectedUnsupportedOps.length > 0) readinessBlockers.push('unexpectedUnsupportedOps');
    if (this.currentImageFailures.size > 0) readinessBlockers.push('imageReplayFailure');
    if (this.localTypefacePending.size > 0) readinessBlockers.push('localFontsPending');
    return {
      mode: this.renderMode,
      surfacePreference: this.surfaceRequest.preference,
      surfaceBackend: this.surfaceBackend,
      surfaceFallbackReason,
      lastRenderCompleted: this.lastRenderCompleted,
      lastUnsupportedOps,
      lastExpectedUnsupportedOps,
      lastUnexpectedUnsupportedOps,
      lastRenderError: this.lastRenderError,
      passesRuntimeReadinessGate: readinessBlockers.length === 0,
      readinessBlockers,
      hiddenCanvas2dOverlayUsed: false,
      lastRenderDurationMs: this.lastRenderDurationMs,
      renderCount: this.renderCount,
      imageCacheEntries: this.imageCache.size,
      imageCacheLimit: MAX_IMAGE_CACHE_ENTRIES,
      imageCachePixels: this.imageCachePixels,
      imageCachePixelLimit: MAX_IMAGE_CACHE_PIXELS,
      imageCacheHits: this.imageCacheHits,
      imageCacheMisses: this.imageCacheMisses,
      imageCacheEvictions: this.imageCacheEvictions,
      imageFailureCacheHits: this.imageFailureCacheHits,
      imageFailures: [...this.currentImageFailures.values()].map((failure) => ({ ...failure })),
      localTypefaceCount: this.localTypefaces.size,
      localTypefaceLoadFailureCount: this.localTypefaceLoadFailures.size,
      localTypefacePendingCount: this.localTypefacePending.size,
      bundledTypefaceCount: this.bundledTypefaces.size,
      bundledTypefaceLoadFailureCount: this.bundledTypefaceLoadFailures.size,
      glyphRunFontBlobCount: glyphRunFontDiagnostics.blobs,
      glyphRunFontBlobBytes: glyphRunFontDiagnostics.bytes,
      glyphRunTypefaceCount: glyphRunFontDiagnostics.typefaces,
      glyphRunFontCount: glyphRunFontDiagnostics.fonts,
      fontSubstitutionLimit: MAX_FONT_SUBSTITUTION_DIAGNOSTICS,
      unregisteredFontFallbacks: fontSubstitutions.filter(
        (substitution) => substitution.kind === 'unregisteredFallback',
      ).length,
      fontSubstitutions,
      replayFeatureCounts: { ...this.currentReplayFeatureCounts },
    };
  }

  private resetReplayFeatureCounts(): void {
    this.currentReplayFeatureCounts = {
      dashedStrokes: 0,
      glyphRuns: 0,
      verticalPresentationPunctuation: 0,
      verticalTextRuns: 0,
    };
  }

  recordRenderFailure(error: unknown, resetReplayState = false): void {
    if (resetReplayState) {
      this.unsupportedOps.clear();
      this.currentImageFailures.clear();
      this.currentFontSubstitutions.clear();
      this.resetReplayFeatureCounts();
      this.surfaceBackend = null;
      this.surfaceFallbackReason = null;
    }
    this.lastRenderCompleted = false;
    this.lastRenderError = error instanceof Error ? error.message : String(error);
    this.unsupportedOps.add('renderPage');
  }

  dispose(): void {
    this.disposed = true;
    this.resetDocumentResources();
    this.defaultTypeface?.delete();
    this.symbolFallbackTypeface?.delete();
    this.defaultFontManager?.delete();
    this.oldHangulTypeface?.typeface?.delete?.();
    this.oldHangulTypeface?.fontManager?.delete?.();
  }

  private makeSurface(
    targetCanvas: HTMLCanvasElement,
  ): CanvasKitSurfaceTarget {
    this.surfaceBackend = null;
    this.surfaceFallbackReason = this.surfaceRequest.unsupportedReason ?? null;
    if (this.surfaceRequest.preference === 'webgpu' && this.surfaceFallbackReason === null) {
      this.surfaceFallbackReason = 'webgpuSurfaceUnsupported';
    }
    const reuseSoftwareFallbackCanvas = targetCanvas.classList.contains('ck-replaced');
    if (this.surfaceRequest.preference === 'software' || reuseSoftwareFallbackCanvas) {
      const swSurface = this.canvasKit.MakeSWCanvasSurface(targetCanvas);
      if (swSurface) {
        this.surfaceBackend = 'software';
        if (reuseSoftwareFallbackCanvas && this.surfaceFallbackReason === null) {
          this.surfaceFallbackReason = 'defaultSurfaceUnavailableUsingSoftware';
        }
        return { surface: swSurface, canvas: targetCanvas };
      }
      this.surfaceFallbackReason = 'softwareSurfaceUnavailable';
    }
    const originalParent = targetCanvas.parentElement;
    const originalChildIndex = originalParent
      ? Array.prototype.indexOf.call(originalParent.children, targetCanvas)
      : -1;
    try {
      const surface = this.canvasKit.MakeCanvasSurface(targetCanvas);
      if (surface) {
        const replacement = originalParent && originalChildIndex >= 0
          ? originalParent.children.item(originalChildIndex)
          : null;
        if (targetCanvas.parentElement !== originalParent && replacement instanceof HTMLCanvasElement) {
          this.surfaceBackend = 'software';
          if (this.surfaceFallbackReason === null) {
            this.surfaceFallbackReason = 'defaultSurfaceUnavailableUsingSoftware';
          }
          return { surface, canvas: replacement };
        }
        this.surfaceBackend = 'default';
        return { surface, canvas: targetCanvas };
      }
    } catch {
      if (this.surfaceFallbackReason === null) {
        this.surfaceFallbackReason = 'defaultSurfaceCreationFailed';
      }
    }
    const internalReplacement = originalParent && originalChildIndex >= 0
      ? originalParent.children.item(originalChildIndex)
      : null;
    let softwareCanvas = targetCanvas.parentElement !== originalParent
      && internalReplacement instanceof HTMLCanvasElement
      ? internalReplacement
      : targetCanvas;
    if (softwareCanvas === targetCanvas && targetCanvas.parentElement) {
      const parent = targetCanvas.parentElement;
      const replacement = targetCanvas.cloneNode(true) as HTMLCanvasElement;
      replacement.classList.add('ck-replaced');
      parent.replaceChild(replacement, targetCanvas);
      softwareCanvas = replacement;
    }
    const softwareSurface = this.canvasKit.MakeSWCanvasSurface(softwareCanvas);
    if (softwareSurface) {
      this.surfaceBackend = 'software';
      if (this.surfaceFallbackReason === null) {
        this.surfaceFallbackReason = 'defaultSurfaceUnavailableUsingSoftware';
      }
      return { surface: softwareSurface, canvas: softwareCanvas };
    }
    throw new Error('CanvasKit surface를 만들 수 없습니다');
  }

  private selectTextVariants(node: LayerNode): void {
    if (node.kind === 'group') {
      for (const child of node.children) this.selectTextVariants(child);
      return;
    }
    if (node.kind === 'clipRect') {
      this.selectTextVariants(node.child);
      return;
    }

    const selected = selectLayerTextVariantsForLeaf(
      node.ops,
      op => this.glyphOutlineVariantReplayable(op),
      op => this.glyphRunVariantReplayable(op),
    );
    for (const op of selected) {
      this.selectedTextVariantOps.add(op);
    }
  }

  private glyphRunVariantReplayable(op: LayerGlyphRunOp): boolean {
    return this.glyphRunFonts.replayStatus(op, this.currentFontResources).replayable;
  }

  private glyphOutlineVariantReplayable(op: LayerGlyphOutlineOp): boolean {
    if (op.diagnostics?.strictVisualEligible !== true) return false;
    const status = glyphOutlinePayloadStatus(op, {
      allowMonochromeFillStroke: true,
      allowColrv1Stage1ColorGraph: true,
      allowBitmapGlyph: true,
      allowSvgGlyph: true,
    });
    if (!status.supported) return false;
    if (op.payloadKind === 'bitmapGlyph') {
      const imageOp = this.bitmapGlyphImageOp(op);
      return imageOp !== null && this.imageForOp(imageOp) !== null;
    }
    if (op.payloadKind === 'svgGlyph') {
      return this.staticSvgGlyphPathLayers(op) !== null;
    }
    return op.payloadKind === 'colorLayers'
      || op.payloadKind === 'monochromeFill'
      || op.payloadKind === 'monochromeFillStroke';
  }

  private layerResourceIndex(
    id: number | string | undefined,
    keys: string[] | undefined,
    length: number,
  ): number | null {
    if (typeof id === 'number' && Number.isInteger(id) && id >= 0 && id < length) return id;
    if (typeof id !== 'string') return null;
    const index = keys?.indexOf(id) ?? -1;
    return index >= 0 && index < length ? index : null;
  }

  private bitmapGlyphImageOp(op: LayerGlyphOutlineOp): LayerImageOp | null {
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
        || base64.length > CanvasKitLayerRenderer.MAX_BITMAP_GLYPH_BASE64_LENGTH) {
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

  private staticSvgGlyphPathLayers(op: LayerGlyphOutlineOp): StaticSvgPathLayer[] | null {
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
      || fragment.length > CanvasKitLayerRenderer.MAX_STATIC_SVG_GLYPH_BYTES) {
      return null;
    }
    const fragmentBytes = new TextEncoder().encode(fragment);
    if (
      fragmentBytes.byteLength > CanvasKitLayerRenderer.MAX_STATIC_SVG_GLYPH_BYTES
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
    if (this.svgGlyphPathCache.size >= CanvasKitLayerRenderer.MAX_SVG_GLYPH_CACHE_ENTRIES) {
      const oldestKey = this.svgGlyphPathCache.keys().next().value as string | undefined;
      if (oldestKey !== undefined) this.svgGlyphPathCache.delete(oldestKey);
    }
    this.svgGlyphPathCache.set(resourceKey, layers);
    return layers;
  }

  private rememberSvgGlyphParseFailure(resourceKey: string): void {
    if (this.svgGlyphParseFailures.size >= CanvasKitLayerRenderer.MAX_SVG_GLYPH_CACHE_ENTRIES) {
      const oldestKey = this.svgGlyphParseFailures.values().next().value as string | undefined;
      if (oldestKey !== undefined) this.svgGlyphParseFailures.delete(oldestKey);
    }
    this.svgGlyphParseFailures.add(resourceKey);
  }

  private renderNode(
    canvas: SkCanvas,
    node: LayerNode,
    profile: LayerRenderProfile,
    replayPlane: CanvasKitReplayPlane,
    inheritedLayer: LayerInfo | null = null,
    rightOverflowSlop?: number,
  ): void {
    const activeLayer = node.layer ?? inheritedLayer;
    if (node.kind === 'group') {
      for (const child of node.children) {
        this.renderNode(canvas, child, profile, replayPlane, activeLayer, rightOverflowSlop);
      }
      return;
    }
    if (node.kind === 'clipRect') {
      this.renderClipNode(canvas, node, profile, replayPlane, activeLayer, rightOverflowSlop);
      return;
    }
    this.renderLeaf(canvas, node, profile, replayPlane, activeLayer);
  }

  private renderClipNode(
    canvas: SkCanvas,
    node: LayerClipNode,
    profile: LayerRenderProfile,
    replayPlane: CanvasKitReplayPlane,
    inheritedLayer: LayerInfo | null,
    rightOverflowSlop?: number,
  ): void {
    const pad = canvaskitClipRightPad(this.renderMode, profile, node.clipKind, rightOverflowSlop);
    const clip = {
      ...node.clip,
      width: node.clip.width + pad,
    };
    canvas.save();
    canvas.clipRect(this.rect(clip), this.canvasKit.ClipOp?.Intersect ?? 0, true);
    this.renderNode(canvas, node.child, profile, replayPlane, inheritedLayer, rightOverflowSlop);
    canvas.restore();
  }

  private renderLeaf(
    canvas: SkCanvas,
    node: LayerLeafNode,
    profile: LayerRenderProfile,
    replayPlane: CanvasKitReplayPlane,
    inheritedLayer: LayerInfo | null,
  ): void {
    const activeLayer = node.layer ?? inheritedLayer;
    for (const op of node.ops) {
      if (layerPaintOpReplayPlane(op, activeLayer) !== replayPlane) {
        continue;
      }
      const equivalenceGroup = 'variant' in op ? op.variant?.equivalenceGroup : undefined;
      if (equivalenceGroup && !this.selectedTextVariantOps.has(op)) {
        continue;
      }
      this.renderOp(canvas, op, profile);
    }
  }

  private renderOp(canvas: SkCanvas, op: LayerPaintOp, profile: LayerRenderProfile): void {
    switch (op.type) {
      case 'pageBackground':
        this.renderPageBackground(canvas, op);
        return;
      case 'rectangle':
        this.renderRectangle(canvas, op);
        return;
      case 'ellipse':
        this.renderEllipse(canvas, op);
        return;
      case 'line':
        this.renderLine(canvas, op);
        return;
      case 'path':
        this.renderPath(canvas, op);
        return;
      case 'image':
        this.renderImage(canvas, op);
        return;
      case 'textRun':
        this.renderTextRun(canvas, op);
        return;
      case 'footnoteMarker':
        this.renderTextRun(canvas, {
          type: 'textRun',
          bbox: op.bbox,
          text: op.text,
          baseline: op.fontSize ?? 7,
          style: { fontFamily: op.fontFamily, fontSize: op.fontSize, color: op.color },
        });
        return;
      case 'formObject':
        this.renderFormObject(canvas, op);
        return;
      case 'placeholder':
        this.renderPlaceholder(canvas, op, profile);
        return;
      case 'equation':
        this.renderEquation(canvas, op);
        return;
      case 'rawSvg':
        this.unsupportedOps.add('rawSvg:unsupportedDirectReplay');
        return;
      case 'charOverlap':
        this.renderCharOverlap(canvas, op);
        return;
      case 'tabLeader':
        this.renderTabLeader(canvas, op);
        return;
      case 'textControlMark':
        this.renderTextControlMark(canvas, op);
        return;
      case 'textDecoration':
        this.renderTextDecoration(canvas, op);
        return;
      case 'glyphRun':
        this.renderGlyphRun(canvas, op);
        return;
      case 'glyphOutline': {
        const status = glyphOutlinePayloadStatus(op, {
          allowMonochromeFillStroke: true,
          allowColrv1Stage1ColorGraph: true,
          allowBitmapGlyph: true,
          allowSvgGlyph: true,
        });
        if (status.supported && this.glyphOutlineVariantReplayable(op)) {
          this.renderGlyphOutline(canvas, op);
          return;
        }
        this.unsupportedOps.add(status.reason ? `glyphOutline:${status.reason}` : 'glyphOutline');
        return;
      }
      default:
        this.unsupportedOps.add((op as { type?: string }).type ?? 'unknown');
    }
  }

  private renderPageBackground(canvas: SkCanvas, op: LayerPageBackgroundOp): void { _shapes.renderPageBackground.call(this, canvas, op); }

  private renderRectangle(canvas: SkCanvas, op: LayerRectangleOp): void { _shapes.renderRectangle.call(this, canvas, op); }

  private renderEllipse(canvas: SkCanvas, op: LayerEllipseOp): void { _shapes.renderEllipse.call(this, canvas, op); }

  private renderLine(canvas: SkCanvas, op: LayerLineOp): void { _shapes.renderLine.call(this, canvas, op); }

  private renderPath(canvas: SkCanvas, op: LayerPathOp): void { _shapes.renderPath.call(this, canvas, op); }

  private applyPathCommand(path: MutablePath, command: LayerPathCommand, currentX: number, currentY: number): [number, number] { return _shapes.applyPathCommand.call(this, path, command, currentX, currentY); }

  private renderImage(canvas: SkCanvas, op: LayerImageOp): void {
    if (!op.base64) {
      this.recordImageFailure(op, 'dataMissing', null);
      this.unsupportedOps.add('image:dataMissing');
      return;
    }
    const image = this.imageForOp(op);
    if (!image) {
      this.unsupportedOps.add('image:decodeFailed');
      return;
    }
    this.recordImageCoverageGaps(op);
    this.withImageTransform(canvas, op.bbox, op.transform, () => this.drawImageOp(canvas, image, op));
  }

  private renderGlyphRun(canvas: SkCanvas, op: LayerGlyphRunOp): void {
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

  private renderGlyphOutline(canvas: SkCanvas, op: LayerGlyphOutlineOp): void {
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

  private renderBitmapGlyphOutline(canvas: SkCanvas, op: LayerGlyphOutlineOp): void {
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

  private renderSvgGlyphOutline(canvas: SkCanvas, op: LayerGlyphOutlineOp): void {
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
              paint = this.makeFillPaint(layer.fill, layer.opacity);
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
              paint = this.makeStrokePaint(stroke.color, stroke.width, stroke.opacity);
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

  private renderMonochromeGlyphOutline(canvas: SkCanvas, op: LayerGlyphOutlineOp): void {
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

  private applyGlyphPathFillRule(path: Path, fillRule: string | undefined): void {
    path.setFillType(fillRule === 'evenodd' ? this.canvasKit.FillType.EvenOdd : this.canvasKit.FillType.Winding);
  }

  private renderColorPaintGraphNode(
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

  private affineToCanvasKitMatrix(transform: LayerAffineTransform | undefined): number[] | null { return _colors.affineToCanvasKitMatrix.call(this, transform); }

  private applyFillRule(path: MutablePath, fillRule: string | undefined): void { _colors.applyFillRule.call(this, path, fillRule); }

  private resolvedColor(color: { rgba?: number[] }): Color { return _colors.resolvedColor.call(this, color); }

  private makeLinearGradientShader(gradient: NonNullable<LayerColorGraphNode['linearGradientPath']>['gradient']): unknown { return _colors.makeLinearGradientShader.call(this, gradient); }

  private makeRadialGradientShader(gradient: NonNullable<LayerColorGraphNode['radialGradientPath']>['gradient']): unknown { return _colors.makeRadialGradientShader.call(this, gradient); }

  private makeSweepGradientShader(gradient: NonNullable<LayerColorGraphNode['sweepGradientPath']>['gradient']): unknown { return _colors.makeSweepGradientShader.call(this, gradient); }

  private drawImageOp(canvas: SkCanvas, image: SkImage, op: LayerImageOp): void { _imageCache.drawImageOp.call(this, canvas, image, op); }

  private drawImageRect(canvas: SkCanvas, image: SkImage, source: Rect, dest: Rect, opacity = 1): void { _imageCache.drawImageRect.call(this, canvas, image, source, dest, opacity); }

  private drawTiledImage(canvas: SkCanvas, bbox: LayerBounds, fillMode: string, tileWidth: number, tileHeight: number, drawImage: (dstX: number, dstY: number, dstW: number, dstH: number) => void): void { _imageCache.drawTiledImage.call(this, canvas, bbox, fillMode, tileWidth, tileHeight, drawImage); }

  private withImageTransform(canvas: SkCanvas, bounds: LayerBounds, transform: LayerImageOp['transform'], draw: () => void): void { _imageCache.withImageTransform.call(this, canvas, bounds, transform, draw); }

  private recordImageCoverageGaps(op: LayerImageOp): void { _imageCache.recordImageCoverageGaps.call(this, op); }

  private recordTextRunCoverageGaps(op: LayerTextRunOp, codePoints: readonly string[]): boolean { return _textRun.recordTextRunCoverageGaps.call(this, op, codePoints); }

  private boundsAreDrawable(bounds: LayerBounds): boolean {
    return Number.isFinite(bounds.x)
      && Number.isFinite(bounds.y)
      && Number.isFinite(bounds.width)
      && Number.isFinite(bounds.height)
      && bounds.width > 0
      && bounds.height > 0;
  }

  private renderTextRun(canvas: SkCanvas, op: LayerTextRunOp): void { _textRun.renderTextRun.call(this, canvas, op); }

  private renderCharOverlap(canvas: SkCanvas, op: LayerCharOverlapOp): void {
    if (typeof op.text !== 'string' || !op.charOverlap || !Array.isArray(op.positions)) {
      this.unsupportedOps.add('charOverlap:invalidGeometry');
      return;
    }
    if (op.positionsComplete !== true
      || op.positions.length > CanvasKitLayerRenderer.MAX_TEXT_SPECIAL_VISUAL_ITEMS + 1) {
      this.unsupportedOps.add('charOverlap:visualItemLimitExceeded');
      return;
    }
    const chars: string[] = [];
    for (const ch of op.text) {
      if (chars.length >= CanvasKitLayerRenderer.MAX_TEXT_SPECIAL_VISUAL_ITEMS) {
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
            );
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

        let textFont = new this.canvasKit.Font(primaryTypeface, innerFontSize);
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
              fallbackCandidate = new this.canvasKit.Font(fallbackTypeface, innerFontSize);
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
          paint = this.makeFillPaint(textColor);
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

  private renderTextControlMark(canvas: SkCanvas, op: LayerTextControlMarkOp): void {
    if (op.isVertical) {
      this.unsupportedOps.add('textRun:verticalText');
      return;
    }
    if (!Array.isArray(op.marks)) {
      this.unsupportedOps.add('textControlMark:invalidGeometry');
      return;
    }
    if (op.marksComplete !== true
      || op.marks.length > CanvasKitLayerRenderer.MAX_TEXT_SPECIAL_VISUAL_ITEMS) {
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
        paint = this.makeStrokePaint('#0066ff', 0.75);
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

  private renderTabLeader(canvas: SkCanvas, op: LayerTabLeaderOp): void {
    if (op.isVertical) {
      this.unsupportedOps.add('textRun:verticalText');
      return;
    }
    if (!Array.isArray(op.leaders)) {
      this.unsupportedOps.add('tabLeader:invalidGeometry');
      return;
    }
    if (op.leadersComplete !== true
      || op.leaders.length > CanvasKitLayerRenderer.MAX_TEXT_SPECIAL_VISUAL_ITEMS) {
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

  private renderTextDecoration(canvas: SkCanvas, op: LayerTextDecorationOp): void {
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
      || decoration.positions.length > CanvasKitLayerRenderer.MAX_TEXT_SPECIAL_VISUAL_ITEMS + 1) {
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
        fillPaint = this.makeFillPaint(decoration.color);
        strokePaint = this.makeStrokePaint(
          decoration.color,
          Math.max(dotSize * 0.12, 0.75),
        );
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

  private withHorizontalTextVisualOrigin(canvas: SkCanvas, bbox: LayerBounds, rotation: number, opType: 'charOverlap' | 'textControlMark' | 'tabLeader' | 'textDecoration', draw: (originX: number, originY: number) => void): void { _textRun.withHorizontalTextVisualOrigin.call(this, canvas, bbox, rotation, opType, draw); }

  private drawTextVisualStroke(canvas: SkCanvas, x1: number, y1: number, x2: number, y2: number, color: string, width: number, dash: number[] = [], roundCap = false, waveHeight = 0, waveWidth = 0): void { _textRun.drawTextVisualStroke.call(this, canvas, x1, y1, x2, y2, color, width, dash, roundCap, waveHeight, waveWidth); }

  private renderShapedScriptText(canvas: SkCanvas, text: string, color: string, fontSize: number, originX: number, originY: number, baselineShift: number, fontManager: FontMgr | null, fontFamily: string | null, bold: boolean, italic: boolean): boolean { return _textRun.renderShapedScriptText.call(this, canvas, text, color, fontSize, originX, originY, baselineShift, fontManager, fontFamily, bold, italic); }

  private findPreparedTypeface(fontFamily: string | undefined): CanvasKitLocalTypeface | null { return _textRun.findPreparedTypeface.call(this, fontFamily); }

  private recordFontSubstitution(diagnostic: CanvasKitFontSubstitutionDiagnostic): void { _textRun.recordFontSubstitution.call(this, diagnostic); }

  private renderEquation(canvas: SkCanvas, op: LayerEquationOp): void { _equation.renderEquation.call(this, canvas, op); }

  private renderEquationBox(canvas: SkCanvas, layout: LayerEquationLayoutBox, parentX: number, parentY: number, color: string, fontSize: number, italic: boolean, bold: boolean, depth: number, budget: EquationRenderBudget): boolean { return _equation.renderEquationBox.call(this, canvas, layout, parentX, parentY, color, fontSize, italic, bold, depth, budget); }

  private equationBoxIsFinite(layout: LayerEquationLayoutBox): boolean { return _equation.equationBoxIsFinite.call(this, layout); }

  private equationFontSizeFromBox(layout: LayerEquationLayoutBox, baseFontSize: number): number { return _equation.equationFontSizeFromBox.call(this, layout, baseFontSize); }

  private drawEquationText(canvas: SkCanvas, text: string, x: number, baselineY: number, fontSize: number, color: string, italic: boolean, bold: boolean, targetWidth: number, centered: boolean): boolean { return _equation.drawEquationText.call(this, canvas, text, x, baselineY, fontSize, color, italic, bold, targetWidth, centered); }

  private drawEquationLine(canvas: SkCanvas, x1: number, y1: number, x2: number, y2: number, color: string, width: number): boolean { return _equation.drawEquationLine.call(this, canvas, x1, y1, x2, y2, color, width); }

  private drawEquationBracket(canvas: SkCanvas, bracket: string, x: number, y: number, height: number, color: string, fontSize: number): boolean { return _equation.drawEquationBracket.call(this, canvas, bracket, x, y, height, color, fontSize); }

  private drawEquationDecoration(canvas: SkCanvas, decoration: string, centerX: number, y: number, width: number, color: string, fontSize: number): boolean { return _equation.drawEquationDecoration.call(this, canvas, decoration, centerX, y, width, color, fontSize); }

  private renderFormObject(canvas: SkCanvas, op: LayerFormObjectOp): void {
    const fill = op.backColor && op.backColor !== '#000000' ? op.backColor : '#f7f7f7';
    this.drawStyledShape(canvas, op.bbox, {
      fillColor: fill,
      strokeColor: op.foreColor ?? '#555555',
      strokeWidth: 1,
      opacity: op.enabled === false ? 0.55 : 1,
    }, (paint) => canvas.drawRect(this.rect(op.bbox), paint));
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

  private renderPlaceholder(canvas: SkCanvas, op: LayerPlaceholderOp, profile: LayerRenderProfile): void {
    if (op.kind === 'missingPicture') {
      if (profile === 'print' || profile === 'highQuality') return;
      if (![op.bbox.x, op.bbox.y, op.bbox.width, op.bbox.height].every(Number.isFinite)
        || op.bbox.width <= 0 || op.bbox.height <= 0) return;
      const paint = this.makeStrokePaint(op.strokeColor ?? '#999999', 1);
      const dash = 5;
      const gap = 3;
      const horizontalStep = Math.max(
        dash + gap,
        op.bbox.width / CanvasKitLayerRenderer.MAX_PLACEHOLDER_DASH_SEGMENTS_PER_AXIS,
      );
      const verticalStep = Math.max(
        dash + gap,
        op.bbox.height / CanvasKitLayerRenderer.MAX_PLACEHOLDER_DASH_SEGMENTS_PER_AXIS,
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
        iconFill = this.makeFillPaint('#ffffff');
        iconStroke = this.makeStrokePaint('#888888', 1);
        missingStroke = this.makeStrokePaint('#cc4444', 1.5);
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
    }, (paint) => canvas.drawRect(this.rect(op.bbox), paint));
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

  private drawStyledShape(canvas: SkCanvas, bounds: LayerBounds, style: LayerShapeStyle | undefined, draw: (paint: SkPaint) => void): void { _shapes.drawStyledShape.call(this, canvas, bounds, style, draw); }

  private drawStyledPath(canvas: SkCanvas, path: Path, style: LayerShapeStyle): void { _shapes.drawStyledPath.call(this, canvas, path, style); }

  private drawStrokeWithDash(dash: LayerStrokeDash | undefined, paint: SkPaint, draw: () => void): void { _shapes.drawStrokeWithDash.call(this, dash, paint, draw); }

  private imageForOp(op: LayerImageOp): SkImage | null { return _imageCache.imageForOp.call(this, op); }

  private recordImageFailure(op: LayerImageOp, reason: CanvasKitImageFailureReason, key: string | null): void { _imageCache.recordImageFailure.call(this, op, reason, key); }

  private makeFillPaint(color: string, opacity = 1): SkPaint { return _shapes.makeFillPaint.call(this, color, opacity); }

  private makeStrokePaint(color: string, width: number, opacity = 1): SkPaint { return _shapes.makeStrokePaint.call(this, color, width, opacity); }

  private rect(bounds: LayerBounds): Rect { return _shapes.rect.call(this, bounds); }

  private color(cssColor: string, opacity = 1): Color { return _shapes.color.call(this, cssColor, opacity); }
}
