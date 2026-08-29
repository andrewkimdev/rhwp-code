/**
 * CanvasKit 문서 사전점검(preflight) 타입 — 렌더 프로필·리플레이 요약·차단 요인.
 *
 * `core/types.ts`에서 관심사별 분할로 옮겨온 순수 선언(내용 동결).
 * 임포터는 여전히 `@/core/types` 재수출 심을 쓴다.
 */

export type LayerRenderProfile = 'fastPreview' | 'screen' | 'print' | 'highQuality';

export type CanvasKitDocumentPreflightStatus = 'eligible' | 'ineligible' | 'incomplete';

export interface CanvasKitReplaySummary {
  totalItems: number;
  directItems: number;
  directRequiredItems: number;
  compatOverlayItems: number;
  textFallbackItems: number;
  unsupportedItems: number;
  hiddenOverlayViolations: number;
}

export interface CanvasKitDocumentPreflightBlocker {
  code:
    | 'pageLimitExceeded'
    | 'workLimitExceeded'
    | 'pageBuildFailed'
    | 'hiddenCanvas2dOverlayRequired'
    | 'unsupported'
    | 'textFallback'
    | 'compatOverlay';
  pageIndex: number;
  opType?: string;
  detail?: string;
}

export interface CanvasKitDocumentPreflight {
  schemaVersion: 1;
  mode: 'default' | 'compat';
  profile: LayerRenderProfile;
  status: CanvasKitDocumentPreflightStatus;
  eligible: boolean;
  complete: boolean;
  pageCount: number;
  scannedPages: number;
  scannedWorkUnits: number;
  limits: {
    maxPages: number;
    maxWorkUnits: number;
    maxBlockers: number;
    maxRequiredFontFamilies: number;
  };
  summary: CanvasKitReplaySummary;
  blockers: CanvasKitDocumentPreflightBlocker[];
  requiredFontFamilies: string[];
  capabilityDigest: string;
}
