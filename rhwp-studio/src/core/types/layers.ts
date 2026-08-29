/**
 * Layer* 렌더 IR 타입 — 페이지 레이어 트리·페인트 오퍼레이션·글리프 페이로드.
 *
 * `core/types.ts`에서 관심사별 분할로 옮겨온 순수 선언(내용 동결).
 * 임포터는 여전히 `@/core/types` 재수출 심을 쓴다.
 */

import type { LayerRenderProfile } from './preflight';

export interface LayerBounds {
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface LayerAffineTransform {
  a: number;
  b: number;
  c: number;
  d: number;
  e: number;
  f: number;
}

export interface PageLayerTree {
  schemaVersion?: number;
  schemaMinorVersion?: number;
  schema?: {
    major: number;
    minor: number;
  };
  unit?: 'px';
  coordinateSystem?: string;
  profile?: LayerRenderProfile;
  buildOptions?: {
    showTransparentBorders?: boolean;
    clipEnabled?: boolean;
  };
  debugOptions?: {
    debugOverlay?: boolean;
  };
  pageWidth: number;
  pageHeight: number;
  outputOptions?: {
    showParagraphMarks?: boolean;
    showControlCodes?: boolean;
    /** Compatibility mirror; prefer buildOptions.showTransparentBorders. */
    showTransparentBorders?: boolean;
    /** Compatibility mirror; prefer buildOptions.clipEnabled. */
    clipEnabled?: boolean;
    /** Compatibility mirror; prefer debugOptions.debugOverlay. */
    debugOverlay?: boolean;
  };
  fontResources?: LayerFontResources;
  resources?: LayerResources;
  root: LayerNode;
}

export interface LayerResources {
  tableId?: number;
  images?: Array<Uint8Array | number[] | string | undefined>;
  imageHashes?: string[];
  imageKeys?: string[];
  svgFragments?: Array<string | undefined>;
  svgHashes?: string[];
  svgKeys?: string[];
  fontBlobs?: Array<Uint8Array | number[] | string | undefined>;
  fontBlobKeys?: string[];
}

export interface LayerFontResources {
  blobs: LayerFontBlobResource[];
  faces: LayerFontFaceResource[];
}

export interface LayerFontDigest {
  algorithm: string;
  value: string;
}

export interface LayerFontBlobResource {
  id: string;
  source: 'embedded' | 'bundled' | 'systemResolved' | 'externalUrl' | 'unresolvedFallback';
  portability:
    | 'portableBlob'
    | 'externalVerified'
    | 'resolvedButNotEmbedded'
    | 'systemNameOnly'
    | 'unresolvedFallback';
  digest?: LayerFontDigest;
  dataRef?: { kind: 'fontBlob' | 'externalFont'; id: string };
}

export interface LayerFontFaceResource {
  id: string;
  blobKey: string;
  faceIndex: number;
  postscriptName?: string;
  familyNames?: Array<{ value: string; locale?: string }>;
  styleNames?: Array<{ value: string; locale?: string }>;
  weightClass?: number;
  widthClass?: number;
  italic?: boolean;
}

export interface LayerInfo {
  textWrap?: string | null;
  zOrder: number;
  stableIndex: number;
  /** 바탕쪽 유래 여부 (#2318). true 면 replay plane 이 behindText 로 상한 고정된다. */
  masterPage?: boolean;
}

export type LayerNode = LayerGroupNode | LayerClipNode | LayerLeafNode;

export interface LayerGroupNode {
  kind: 'group';
  bounds: LayerBounds;
  layer?: LayerInfo;
  groupKind?: { kind: string; [key: string]: unknown };
  cacheHint?: LayerCacheHint;
  children: LayerNode[];
}

export interface LayerClipNode {
  kind: 'clipRect';
  bounds: LayerBounds;
  layer?: LayerInfo;
  clip: LayerBounds;
  clipKind: 'body' | 'tableCell' | 'textBox' | 'generic';
  child: LayerNode;
}

export interface LayerLeafNode {
  kind: 'leaf';
  bounds: LayerBounds;
  layer?: LayerInfo;
  ops: LayerPaintOp[];
}

export type LayerCacheHint =
  | 'none'
  | 'staticSubtree'
  | 'preferRaster'
  | 'preferVectorRecording';

export type LayerPaintOp =
  | LayerPageBackgroundOp
  | LayerTextRunOp
  | LayerFootnoteMarkerOp
  | LayerLineOp
  | LayerRectangleOp
  | LayerEllipseOp
  | LayerPathOp
  | LayerImageOp
  | LayerEquationOp
  | LayerFormObjectOp
  | LayerPlaceholderOp
  | LayerRawSvgOp
  | LayerTextDecorationOp
  | LayerTextControlMarkOp
  | LayerTabLeaderOp
  | LayerCharOverlapOp
  | LayerGlyphRunOp
  | LayerGlyphOutlineOp;

export interface LayerPageBackgroundOp {
  type: 'pageBackground';
  bbox: LayerBounds;
  backgroundColor?: string;
  borderColor?: string;
  borderWidth?: number;
}

export interface LayerTextStyle {
  fontFamily?: string;
  fontSize?: number;
  color?: string;
  bold?: boolean;
  italic?: boolean;
  ratio?: number;
  underline?: string;
  underlineShape?: number;
  strikethrough?: boolean;
  strikeShape?: number;
  outlineType?: number;
  shadowType?: number;
  shadowColor?: string;
  shadowOffsetX?: number;
  shadowOffsetY?: number;
  emboss?: boolean;
  engrave?: boolean;
  superscript?: boolean;
  subscript?: boolean;
  underlineColor?: string;
  strikeColor?: string;
  shadeColor?: string;
  emphasisDot?: number;
}

export interface LayerTextLegacyVisuals {
  charOverlap?: 'canonical' | 'mirror';
  controlMarks?: 'canonical' | 'mirror';
  tabLeaders?: 'canonical' | 'mirror';
  decorations?: 'canonical' | 'mirror';
}

export interface LayerCharOverlap {
  borderType: number;
  innerCharSize: number;
}

export interface LayerTabLeader {
  startX: number;
  endX: number;
  fillType: number;
}

export type LayerTextControlMarkKind = 'space' | 'tab' | 'paragraphEnd' | 'lineBreakEnd';

export interface LayerTextControlMark {
  kind: LayerTextControlMarkKind;
  text: string;
  /** X offset relative to the text run origin. */
  x: number;
  /** Y offset relative to the text run baseline. */
  y: number;
  fontSize: number;
}

export interface LayerTextRunOp {
  type: 'textRun';
  bbox: LayerBounds;
  text: string;
  displayText?: string;
  /** Run-local baseline offset from bbox.y when placement is absent. */
  baseline?: number;
  rotation?: number;
  isVertical?: boolean;
  orientation?: 'horizontal' | 'vertical-upright' | 'vertical-sideways';
  style?: LayerTextStyle;
  placement?: { runToPage?: LayerAffineTransform; baselineY?: number };
  positions?: number[];
  displayPositions?: number[];
  legacyVisuals?: LayerTextLegacyVisuals;
  controlMarks?: LayerTextControlMark[];
  controlMarksComplete?: boolean;
  tabLeaders?: LayerTabLeader[];
  charOverlap?: LayerCharOverlap | null;
  isParaEnd?: boolean;
  isLineBreakEnd?: boolean;
  fieldMarker?: { kind?: string; controlIndex?: number };
  variant?: LayerTextVariantMeta;
}

export interface LayerFootnoteMarkerOp {
  type: 'footnoteMarker';
  bbox: LayerBounds;
  text: string;
  fontFamily?: string;
  fontSize?: number;
  color?: string;
}

export type LayerStrokeDash = 'solid' | 'dash' | 'dot' | 'dashDot' | 'dashDotDot';

export interface LayerLineStyle {
  color?: string;
  width?: number;
  dash?: LayerStrokeDash;
  lineType?: string;
  startArrow?: string;
  endArrow?: string;
}

export interface LayerShapeStyle {
  fillColor?: string | null;
  strokeColor?: string | null;
  strokeWidth?: number;
  strokeDash?: LayerStrokeDash;
  opacity?: number;
}

export interface LayerLineOp {
  type: 'line';
  bbox: LayerBounds;
  x1: number;
  y1: number;
  x2: number;
  y2: number;
  style?: LayerLineStyle;
}

export interface LayerRectangleOp {
  type: 'rectangle';
  bbox: LayerBounds;
  cornerRadius?: number;
  style?: LayerShapeStyle;
}

export interface LayerEllipseOp {
  type: 'ellipse';
  bbox: LayerBounds;
  style?: LayerShapeStyle;
}

export type LayerPathCommand =
  | { type: 'moveTo'; x: number; y: number }
  | { type: 'lineTo'; x: number; y: number }
  | { type: 'curveTo'; x1: number; y1: number; x2: number; y2: number; x3: number; y3: number }
  | { type: 'arcTo'; rx: number; ry: number; rotation: number; largeArc: boolean; sweep: boolean; x: number; y: number }
  | { type: 'closePath' };

/**
 * [Task #1067] 도형(polygon, rectangle 등) path 의 회전/반전 변환.
 *
 * Rust paint pipeline (`src/paint/json.rs:write_transform`) 이 JSON 으로 다음 형식 emit:
 * `{"rotation": <degrees>, "horzFlip": <bool>, "vertFlip": <bool>}`
 *
 * 누락 시 HWPX/HWP 도형의 회전/flip 정보가 캔버스 렌더링에 반영되지 않아
 * 도형이 회전 없이 출력 (e.g. 두 도형이 거울 대칭이어야 하는데 같은 모양으로 보임).
 */
export interface LayerPathTransform {
  rotation?: number;
  horzFlip?: boolean;
  vertFlip?: boolean;
}

export interface LayerPathOp {
  type: 'path';
  bbox: LayerBounds;
  commands?: LayerPathCommand[];
  style?: LayerShapeStyle;
  lineStyle?: LayerLineStyle;
  transform?: LayerPathTransform;
}

export interface LayerImageOp {
  type: 'image';
  bbox: LayerBounds;
  mime?: string;
  base64?: string;
  imageRef?: number | string;
  /** 문서 세대와 BinData ID에서 만든 원본 그림 신원 키 (schema minor 20+). */
  sourceImageKey?: string;
  fillMode?: string;
  originalSize?: { width: number; height: number };
  crop?: { left: number; top: number; right: number; bottom: number };
  originalSizeHu?: [number, number];
  effect?: string;
  brightness?: number;
  contrast?: number;
  opacity?: number;
  bakedWatermark?: boolean;
  wrap?: 'behindText' | 'inFrontOfText' | string;
  transform?: LayerPathTransform;
}

export interface LayerEquationOp {
  type: 'equation';
  bbox: LayerBounds;
  svgContent?: string;
  color?: string;
  fontSize?: number;
  layoutBox?: LayerEquationLayoutBox;
}

export type LayerEquationMatrixStyle = 'plain' | 'paren' | 'bracket' | 'vert';
export type LayerEquationDecoration =
  | 'hat'
  | 'check'
  | 'tilde'
  | 'acute'
  | 'grave'
  | 'dot'
  | 'dDot'
  | 'bar'
  | 'vec'
  | 'dyad'
  | 'under'
  | 'arch'
  | 'underline'
  | 'overline'
  | 'strikeThrough';
export type LayerEquationFontStyle =
  | 'roman'
  | 'italic'
  | 'bold'
  | 'blackboard'
  | 'calligraphy'
  | 'fraktur'
  | 'sansSerif'
  | 'monospace';

export interface LayerEquationLayoutBox {
  x: number;
  y: number;
  width: number;
  height: number;
  baseline: number;
  kind: LayerEquationLayoutKind;
}

export type LayerEquationLayoutKind =
  | { type: 'row'; children: LayerEquationLayoutBox[] }
  | { type: 'text'; text: string }
  | { type: 'number'; text: string }
  | { type: 'symbol'; text: string }
  | { type: 'mathSymbol'; text: string }
  | { type: 'function'; name: string }
  | { type: 'fraction'; numer: LayerEquationLayoutBox; denom: LayerEquationLayoutBox }
  | { type: 'atop'; top: LayerEquationLayoutBox; bottom: LayerEquationLayoutBox }
  | { type: 'sqrt'; body: LayerEquationLayoutBox; index?: LayerEquationLayoutBox }
  | { type: 'superscript'; base: LayerEquationLayoutBox; sup: LayerEquationLayoutBox }
  | { type: 'subscript'; base: LayerEquationLayoutBox; sub: LayerEquationLayoutBox }
  | { type: 'subSup'; base: LayerEquationLayoutBox; sub: LayerEquationLayoutBox; sup: LayerEquationLayoutBox }
  | { type: 'bigOp'; symbol: string; sub?: LayerEquationLayoutBox; sup?: LayerEquationLayoutBox }
  | { type: 'limit'; isUpper: boolean; sub?: LayerEquationLayoutBox }
  | { type: 'matrix'; style: LayerEquationMatrixStyle; cells: LayerEquationLayoutBox[][] }
  | { type: 'rel'; arrow: LayerEquationLayoutBox; over: LayerEquationLayoutBox; under?: LayerEquationLayoutBox }
  | { type: 'eqAlign'; rows: Array<{ left: LayerEquationLayoutBox; right: LayerEquationLayoutBox }> }
  | { type: 'paren'; left: string; right: string; body: LayerEquationLayoutBox }
  | { type: 'decoration'; decoration: LayerEquationDecoration; body: LayerEquationLayoutBox }
  | { type: 'fontStyle'; fontStyle: LayerEquationFontStyle; body: LayerEquationLayoutBox }
  | { type: 'space'; width: number }
  | { type: 'newline' }
  | { type: 'empty' };

export interface LayerFormObjectOp {
  type: 'formObject';
  bbox: LayerBounds;
  formType?: string;
  caption?: string;
  text?: string;
  foreColor?: string;
  backColor?: string;
  value?: boolean;
  enabled?: boolean;
}

export interface LayerPlaceholderOp {
  type: 'placeholder';
  bbox: LayerBounds;
  kind?: 'ole' | 'missingPicture';
  fillColor?: string;
  strokeColor?: string;
  label?: string;
}

export interface LayerRawSvgOp {
  type: 'rawSvg';
  bbox: LayerBounds;
  svg?: string;
}

export interface LayerTextDecorationOp {
  type: 'textDecoration';
  bbox: LayerBounds;
  decoration: {
    kind: 'underline' | 'strikethrough' | 'emphasisDot';
    baseline: number;
    rotation: number;
    isVertical: boolean;
    fontSize: number;
    ratio: number;
    color: string;
    shape: number;
    underline: 'none' | 'bottom' | 'top';
    emphasisDot: number;
    positions: number[];
    positionsComplete: boolean;
  };
}

export interface LayerTextControlMarkOp {
  type: 'textControlMark';
  bbox: LayerBounds;
  fieldMarker: string;
  isParaEnd: boolean;
  isLineBreakEnd: boolean;
  baseline: number;
  rotation: number;
  isVertical: boolean;
  marks: LayerTextControlMark[];
  marksComplete: boolean;
  shapeMarkerIndex?: number;
}

export interface LayerTabLeaderOp {
  type: 'tabLeader';
  bbox: LayerBounds;
  leaders: LayerTabLeader[];
  color: string;
  fontSize: number;
  baseline: number;
  rotation: number;
  isVertical: boolean;
  leadersComplete: boolean;
}

export interface LayerCharOverlapOp {
  type: 'charOverlap';
  bbox: LayerBounds;
  text: string;
  baseline: number;
  rotation: number;
  isVertical: boolean;
  orientation?: 'horizontal' | 'vertical-upright' | 'vertical-sideways';
  style: LayerTextStyle;
  positions: number[];
  positionsComplete: boolean;
  charOverlap: LayerCharOverlap;
}

export interface LayerGlyphRunOp {
  type: 'glyphRun';
  bbox: LayerBounds;
  source: LayerTextSourceSpan;
  variant: LayerTextVariantMeta;
  paintStyle: LayerTextStyle;
  shapeKey: LayerShapeKey;
  placement: LayerTextRunPlacement;
  glyphIds: number[];
  positions: LayerPoint[];
  advances?: LayerVector[];
  clusters: LayerGlyphCluster[];
  direction: LayerTextDirection;
  bidiLevel?: number;
  writingMode: LayerWritingMode;
  orientation: LayerGlyphRunOrientation;
  glyphTransforms?: LayerGlyphTransform[];
  diagnostics: LayerGlyphRunDiagnostics;
}

export interface LayerGlyphOutlineOp {
  type: 'glyphOutline';
  bbox: LayerBounds;
  variant?: LayerTextVariantMeta;
  payloadKind?: LayerGlyphOutlinePayloadKind;
  payloadResourceKey?: string;
  paintStyle?: LayerTextStyle;
  placement?: { runToPage?: LayerAffineTransform; baselineY?: number };
  paths?: LayerGlyphOutlinePath[];
  stroke?: LayerGlyphOutlineStroke;
  colorLayers?: LayerColorLayersPayload;
  bitmapGlyph?: LayerBitmapGlyphPayload;
  svgGlyph?: LayerSvgGlyphPayload;
  diagnostics?: { strictVisualEligible?: boolean; [key: string]: unknown };
}

export interface LayerTextVariantMeta {
  equivalenceGroup?: string;
  variantId?: string;
  variantKind?: 'textRun' | 'glyphRun' | 'glyphOutline' | string;
  partIndex?: number;
  partCount?: number;
  isDefaultFallback?: boolean;
  requires?: string[];
  quality?: string;
  anchorOpId?: string;
  localPaintOrder?: number;
}

export interface LayerTextSourceRange {
  start: number;
  end: number;
}

export interface LayerTextSourceSpan {
  id: number;
  utf8Range: LayerTextSourceRange;
  utf16Range: LayerTextSourceRange;
  stableSourceKey?: string;
}

export interface LayerTextRunPlacement {
  runToPage: LayerAffineTransform;
  baselineY?: number;
}

export interface LayerPoint {
  x: number;
  y: number;
}

export interface LayerVector {
  dx: number;
  dy: number;
}

export interface LayerShapeKey {
  fontInstance: {
    faceKey: string;
    sizePx: number;
    variations?: Array<{ tag: string; value: number }>;
    syntheticBold?: boolean;
    syntheticItalic?: boolean;
  };
  direction: LayerTextDirection;
  writingMode: LayerWritingMode;
  script?: string;
  language?: string;
  features?: Array<{ tag: string; enabled: boolean; value?: number }>;
  shapingEngine: string;
  fallbackPolicy: string;
}

export type LayerTextDirection = 'ltr' | 'rtl' | 'auto';
export type LayerWritingMode = 'horizontal-tb' | 'vertical-rl' | 'vertical-lr';
export type LayerGlyphRunOrientation =
  | 'horizontal'
  | 'vertical-upright'
  | 'vertical-sideways'
  | 'mixedPerGlyph';

export interface LayerGlyphCluster {
  sourceRangeUtf8: LayerTextSourceRange;
  sourceRangeUtf16?: LayerTextSourceRange;
  textRangeUtf8?: LayerTextSourceRange;
  glyphRange: LayerTextSourceRange;
  flags?: Array<'ligature' | 'fallbackBoundary'>;
}

export interface LayerGlyphTransform {
  xx: number;
  xy: number;
  yx: number;
  yy: number;
  tx: number;
  ty: number;
}

export interface LayerGlyphRunDiagnostics {
  quality: 'exact' | 'positionAdjusted' | 'approximate' | 'diagnosticOnly' | 'omitted';
  replayEligibility: 'portable' | 'conditionalExternalFont' | 'localDiagnosticOnly' | 'notReplayable';
  strictVisualEligible: boolean;
  maxOriginDeltaPx: number;
  maxAdvanceDeltaPx: number;
  maxResidualAfterAdjustmentPx: number;
  clusterMismatchCount: number;
  missingGlyphCount: number;
  usedFallbackFontCount: number;
  reason?: string;
}

export type LayerGlyphOutlinePayloadKind =
  | 'monochromeFill'
  | 'monochromeFillStroke'
  | 'colorLayers'
  | 'bitmapGlyph'
  | 'svgGlyph'
  | string;

export interface LayerGlyphOutlinePath {
  glyphId?: number;
  sourceRangeUtf8?: LayerTextRange;
  glyphRange?: LayerTextRange;
  fillRule?: 'nonzero' | 'evenodd' | string;
  commands?: LayerPathCommand[];
}

export interface LayerGlyphOutlineStroke {
  color?: string;
  width?: number;
  join?: 'miter' | 'round' | 'bevel' | string;
  cap?: 'butt' | 'round' | 'square' | string;
  miterLimit?: number;
  paintOrder?: 'fillOnly' | 'strokeOnly' | 'fillThenStroke' | 'strokeThenFill' | string;
  strictSubset?: boolean;
}

export interface LayerTextRange {
  start?: number;
  end?: number;
}

export interface LayerResolvedColor {
  colorSpace?: string;
  rgba?: number[];
}

export interface LayerColorGradientStop {
  offset?: number;
  color?: LayerResolvedColor;
}

export interface LayerColorSolidPathNode {
  commands?: LayerPathCommand[];
  fill?: LayerResolvedColor;
  fillRule?: 'nonzero' | 'evenodd' | string;
  sourceGlyphId?: number;
  paletteIndex?: number;
}

export interface LayerColorLinearGradient {
  x0?: number;
  y0?: number;
  x1?: number;
  y1?: number;
  stops?: LayerColorGradientStop[];
}

export interface LayerColorRadialGradient {
  cx?: number;
  cy?: number;
  radius?: number;
  stops?: LayerColorGradientStop[];
}

export interface LayerColorSweepGradient {
  cx?: number;
  cy?: number;
  startAngleDegrees?: number;
  endAngleDegrees?: number;
  stops?: LayerColorGradientStop[];
}

export interface LayerColorLinearGradientPathNode {
  commands?: LayerPathCommand[];
  gradient?: LayerColorLinearGradient;
  fillRule?: 'nonzero' | 'evenodd' | string;
  sourceGlyphId?: number;
  paletteIndex?: number;
}

export interface LayerColorRadialGradientPathNode {
  commands?: LayerPathCommand[];
  gradient?: LayerColorRadialGradient;
  fillRule?: 'nonzero' | 'evenodd' | string;
  sourceGlyphId?: number;
  paletteIndex?: number;
}

export interface LayerColorSweepGradientPathNode {
  commands?: LayerPathCommand[];
  gradient?: LayerColorSweepGradient;
  fillRule?: 'nonzero' | 'evenodd' | string;
  sourceGlyphId?: number;
  paletteIndex?: number;
}

export interface LayerColorTransformNode {
  childNodeId?: number;
  transform?: LayerAffineTransform;
}

export interface LayerFontColorGlyphRef {
  faceKey?: string;
  glyphId?: number;
  paletteIndex?: number;
  colorFormat?: 'colrV0' | 'colrV1' | 'other' | string;
}

export interface LayerPaletteRef {
  id?: string;
  index?: number;
  cpalDigest?: string;
}

export interface LayerColorPaintGraphNode {
  nodeId?: number;
  kind?: string;
  solidPath?: LayerColorSolidPathNode;
  linearGradientPath?: LayerColorLinearGradientPathNode;
  radialGradientPath?: LayerColorRadialGradientPathNode;
  sweepGradientPath?: LayerColorSweepGradientPathNode;
  transform?: LayerColorTransformNode;
  sourceRangeUtf8?: LayerTextRange;
  glyphRange?: LayerTextRange;
  sourceFontRef?: LayerFontColorGlyphRef;
}

export interface LayerColorPaintGraphPayload {
  rootNodeId?: number;
  nodes?: LayerColorPaintGraphNode[];
}

export interface LayerColorLayersPayload {
  colorFormat?: 'colrV0' | 'colrV1' | 'other' | string;
  sourceFontRef?: LayerFontColorGlyphRef;
  paletteRef?: LayerPaletteRef;
  layers?: Array<{
    layerIndex?: number | null;
    glyphId?: number;
    glyphRange?: LayerTextRange;
    sourceRangeUtf8?: LayerTextRange;
    sourceFontRef?: LayerFontColorGlyphRef;
    commands?: LayerPathCommand[];
    fill?: LayerResolvedColor;
    fillRule?: 'nonzero' | 'evenodd' | string;
    paletteIndex?: number;
    color?: string;
    opacity?: number;
    transformToRun?: LayerAffineTransform;
  }>;
  paintGraph?: LayerColorPaintGraphPayload;
  sourceRangeUtf8?: LayerTextRange;
  glyphRange?: LayerTextRange;
}

export interface LayerBitmapGlyphPayload {
  imageRef?: number;
  imageResourceId?: number | string;
  sourceRangeUtf8?: LayerTextRange;
  glyphRange?: LayerTextRange;
  placement?: LayerBounds;
  alphaPremultiplied?: boolean;
  scalingPolicy?: 'sourceExact' | 'pixelAligned' | 'backendDefault' | string;
  filtering?: 'nearest' | 'linear' | string;
  transformToRun?: LayerAffineTransform;
}

export interface LayerSvgGlyphPayload {
  svgRef?: number;
  vectorResourceId?: number | string;
  sourceRangeUtf8?: LayerTextRange;
  glyphRange?: LayerTextRange;
  viewBox?: LayerBounds;
  intrinsicSize?: { width?: number; height?: number };
  staticSanitized?: boolean;
  scriptAllowed?: boolean;
  animationAllowed?: boolean;
  externalResourcesAllowed?: boolean;
  interactivityAllowed?: boolean;
  transformToRun?: LayerAffineTransform;
}
