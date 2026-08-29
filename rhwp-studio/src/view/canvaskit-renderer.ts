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
import { CanvasKitGlyphRunFontCache } from './canvaskit/glyph-run-fonts';
import { selectLayerTextVariantsForLeaf } from './canvaskit/text-variant-selection';
import {
  CANVASKIT_REPLAY_PLANES,
  type CanvasKitReplayPlane,
  layerPaintOpReplayPlane,
} from './canvaskit/replay-plane';
import { isExpectedCanvasKitUnsupportedOp } from './canvaskit/diagnostics';
import { glyphOutlinePayloadStatus } from './glyph-outline-payload-status';
import type { StaticSvgPathLayer } from './static-svg-path-layers';
import { loadLocalFontBytesFor, localFontFaceKey, resolveLocalFont, type LocalFontRecord } from '@/core/local-fonts';
import type { CanvasKitBundledFontSource } from '@/core/font-loader';
import { readBoundedResponseArrayBuffer } from './canvaskit/bounded-response';
import * as _equation from './canvaskit/equation';
import type { EquationRenderBudget } from './canvaskit/equation';
import * as _imageCache from './canvaskit/image-cache';
import {
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
} from './canvaskit/text-run';
import * as _textDecorations from './canvaskit/text-decorations';
import * as _glyphOutline from './canvaskit/glyph-outline';
import * as _formObjects from './canvaskit/form-objects';

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
  ): number | null { return _glyphOutline.layerResourceIndex.call(this, id, keys, length); }

  private bitmapGlyphImageOp(op: LayerGlyphOutlineOp): LayerImageOp | null { return _glyphOutline.bitmapGlyphImageOp.call(this, op); }

  private staticSvgGlyphPathLayers(op: LayerGlyphOutlineOp): StaticSvgPathLayer[] | null { return _glyphOutline.staticSvgGlyphPathLayers.call(this, op); }

  private rememberSvgGlyphParseFailure(resourceKey: string): void { _glyphOutline.rememberSvgGlyphParseFailure.call(this, resourceKey); }

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

  private renderGlyphRun(canvas: SkCanvas, op: LayerGlyphRunOp): void { _glyphOutline.renderGlyphRun.call(this, canvas, op); }

  private renderGlyphOutline(canvas: SkCanvas, op: LayerGlyphOutlineOp): void { _glyphOutline.renderGlyphOutline.call(this, canvas, op); }

  private renderBitmapGlyphOutline(canvas: SkCanvas, op: LayerGlyphOutlineOp): void { _glyphOutline.renderBitmapGlyphOutline.call(this, canvas, op); }

  private renderSvgGlyphOutline(canvas: SkCanvas, op: LayerGlyphOutlineOp): void { _glyphOutline.renderSvgGlyphOutline.call(this, canvas, op); }

  private renderMonochromeGlyphOutline(canvas: SkCanvas, op: LayerGlyphOutlineOp): void { _glyphOutline.renderMonochromeGlyphOutline.call(this, canvas, op); }

  private applyGlyphPathFillRule(path: Path, fillRule: string | undefined): void { _glyphOutline.applyGlyphPathFillRule.call(this, path, fillRule); }

  private renderColorPaintGraphNode(
    canvas: SkCanvas,
    nodesById: Map<number, LayerColorGraphNode>,
    nodeId: number,
    visited: Set<number>,
  ): void { _glyphOutline.renderColorPaintGraphNode.call(this, canvas, nodesById, nodeId, visited); }

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

  private renderCharOverlap(canvas: SkCanvas, op: LayerCharOverlapOp): void { _textDecorations.renderCharOverlap.call(this, canvas, op); }

  private renderTextControlMark(canvas: SkCanvas, op: LayerTextControlMarkOp): void { _textDecorations.renderTextControlMark.call(this, canvas, op); }

  private renderTabLeader(canvas: SkCanvas, op: LayerTabLeaderOp): void { _textDecorations.renderTabLeader.call(this, canvas, op); }

  private renderTextDecoration(canvas: SkCanvas, op: LayerTextDecorationOp): void { _textDecorations.renderTextDecoration.call(this, canvas, op); }

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

  private renderFormObject(canvas: SkCanvas, op: LayerFormObjectOp): void { _formObjects.renderFormObject.call(this, canvas, op); }

  private renderPlaceholder(canvas: SkCanvas, op: LayerPlaceholderOp, profile: LayerRenderProfile): void { _formObjects.renderPlaceholder.call(this, canvas, op, profile); }

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
