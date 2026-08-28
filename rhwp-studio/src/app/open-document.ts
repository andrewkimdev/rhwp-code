/**
 * main.ts 문서 열기 파이프라인 — 로컬 글꼴 안내·암호 열기·파일/URL 로드·오류 표시.
 *
 * main.ts 의 모듈 싱글톤·공유 헬퍼는 OpenDocumentDeps 를 통해 매 호출 시점值로
 * 전달받는다(시그니처에서 구조분해해 본문 식별자는 원본과 동일 유지).
 * 본문은 원본 main.ts 에서 동결 이동 — 예외는 클러스터 내부 호출부에 deps 를
 * 흘려보내는 인자 추가뿐이다.
 */

/* eslint-disable @typescript-eslint/no-explicit-any */

import { WasmBridge } from '@/core/wasm-bridge';
import { EventBus } from '@/core/event-bus';
import type { DocumentInfo } from '@/core/types';
import type { ExtensionViewerSettings } from '@/core/extension-settings';
import type { PluginHostRegistry } from '@/plugin/host';
import type { AutosaveManager } from '@/recovery/autosave-manager';
import type { FileSystemFileHandleLike } from '@/command/file-system-access';
import { assertRemoteDocumentBytes } from '@/core/document-signature';
import { detectLocalFonts, getLocalFontState, loadStoredLocalFonts } from '@/core/local-fonts';
import { analyzeDocumentFonts } from '@/core/document-font-status';
import { addRecentDoc } from '@/recent/recent-store';
import { forgetConvertedHmlSaveHandle } from '@/command/save-target';
import { showHwpPasswordDialog } from '@/ui/hwp-password-dialog';
import { showLocalFontsModalIfNeeded } from '@/ui/local-fonts-modal';
import { showToast } from '@/ui/toast';

/** 문서 열기 파이프라인이 필요로 하는 main.ts 측 의존 — 호출 시점 값으로 전달. */
export interface OpenDocumentDeps {
  wasm: WasmBridge;
  eventBus: EventBus;
  autosaveManager: AutosaveManager;
  plugins: PluginHostRegistry;
  extensionViewerSettings: ExtensionViewerSettings;
  sbMessage: () => HTMLElement;
  prepareCanvasKitLocalFonts: (fontNames: readonly string[] | undefined) => void;
  updateLoadProgress: (percent: number, label: string) => Promise<void>;
  initializeDocument: (docInfo: DocumentInfo, displayName: string, options?: { suppressDialogs?: boolean }) => Promise<void>;
  canReplaceCurrentDocument: (skipUnsavedGuard?: boolean) => Promise<boolean>;
  prepareCanvasRendererDocument: () => void;
}

export async function promptLocalFontsIfNeeded(docInfo: DocumentInfo, displayName: string, deps: OpenDocumentDeps): Promise<void> {
  const { sbMessage, extensionViewerSettings, eventBus, prepareCanvasKitLocalFonts } = deps;
  if (!docInfo.fontsUsed?.length) return;

  const msg = sbMessage();
  try {
    await loadStoredLocalFonts();
    const report = analyzeDocumentFonts(docInfo.fontsUsed);
    if (!report.shouldPromptLocalAccess) return;

    const choice = await showLocalFontsModalIfNeeded(report, {
      disableExternalWebFonts: extensionViewerSettings.disableExternalWebFonts,
    });
    if (choice !== 'detect') return;

    msg.textContent = '로컬 글꼴 감지 중...';
    const fonts = await detectLocalFonts({
      force: true,
      includeRegistered: true,
      candidateFamilies: docInfo.fontsUsed,
    });
    const nextReport = analyzeDocumentFonts(docInfo.fontsUsed);
    eventBus.emit('local-fonts-changed', { fonts, report: nextReport });
    prepareCanvasKitLocalFonts(docInfo.fontsUsed);
    const state = getLocalFontState();
    const resultLabel = state.source === 'font-presence-probe' ? '확인됨' : '감지됨';
    msg.textContent = `${displayName} (로컬 글꼴 ${fonts.length}개 ${resultLabel})`;
    showToast({
      message: `로컬 글꼴 ${fonts.length}개를 ${resultLabel.replace('됨', '')}하고 저장했습니다.\n다음 문서 로드부터 감지 결과를 재사용합니다.`,
      durationMs: 5000,
    });
  } catch (error) {
    console.warn('[local-fonts] 감지 안내/실행 실패 (치명적이지 않음):', error);
    msg.textContent = displayName;
    showToast({
      message: '로컬 글꼴 감지에 실패했습니다.\n웹 대체 글꼴로 계속 표시합니다.',
      durationMs: 8000,
    });
  }
}

/**
 * 사용자가 암호 입력 대화상자에서 취소한 경우다. 일반 파싱 실패와 달리 오류 토스트나
 * 최근 문서·자동저장 변경을 만들지 않는다 (#3474).
 */
export class DocumentOpenCancelledError extends Error {
  constructor() {
    super('문서 열기가 취소되었습니다.');
    this.name = 'DocumentOpenCancelledError';
  }
}

const PASSWORD_REQUIRED_MESSAGE = '비밀번호가 필요한 암호 문서';
const PASSWORD_REJECTED_MESSAGE = '비밀번호가 일치하지 않거나 암호화 데이터가 손상되었습니다';

export function isDocumentOpenCancelled(error: unknown): error is DocumentOpenCancelledError {
  return error instanceof DocumentOpenCancelledError;
}

function isPasswordRequiredError(error: unknown): boolean {
  return String(error).includes(PASSWORD_REQUIRED_MESSAGE);
}

function isPasswordRejectedError(error: unknown): boolean {
  return String(error).includes(PASSWORD_REJECTED_MESSAGE);
}

function passwordOpenFailure(error: unknown): Error {
  const message = String(error);
  if (message.includes('지원하지 않는 암호화 방식')) {
    return new Error('지원하지 않는 암호화 방식의 문서입니다. 지원되는 HWP3/HWP5 암호 문서만 열 수 있습니다.');
  }
  if (message.includes('DRM')) {
    return new Error('DRM으로 보호된 문서는 지원하지 않습니다.');
  }
  // 입력값이 포함될 수 있는 원본 오류는 사용자 화면이나 콘솔에 전달하지 않는다. 현재
  // 암호화 포맷은 오입력과 암호문 훼손을 암호학적으로 판별할 수 없으므로 안전한 일반
  // 안내로 축약한다.
  return new Error('암호화된 문서를 열 수 없습니다. 문서가 손상되었는지 확인하세요.');
}

/**
 * 일반 열기를 먼저 시도하고, 지원되는 HWP3/HWP5 암호 문서가 감지된 경우에만 암호
 * 입력 UI로 전환한다. 암호 문자열은 이 함수의 단일 시도 범위를 벗어나 보관하지 않는다.
 */
export async function loadPasswordProtectedDocument(data: Uint8Array, fileName: string, deps: OpenDocumentDeps): Promise<DocumentInfo> {
  const { wasm } = deps;
  let retryMessage: string | undefined;

  while (true) {
    let password = await showHwpPasswordDialog(fileName, retryMessage);
    if (password === null) throw new DocumentOpenCancelledError();

    try {
      return wasm.loadDocumentWithPassword(data, password, fileName);
    } catch (error) {
      // CFB 암호문은 인증 태그가 없으므로 오입력과 암호화 데이터 손상을 완전히 구분할 수
      // 없다. 두 경우만 재입력 상태로 안내하고, 지원하지 않는 암호화/DRM 등은 원래의
      // 명시적 거부 오류를 유지한다.
      if (isPasswordRejectedError(error)) {
        retryMessage = '암호가 일치하지 않거나 문서가 손상되었습니다. 다시 입력하세요.';
        continue;
      }
      throw passwordOpenFailure(error);
    } finally {
      // JavaScript 문자열을 확실히 zeroize할 수는 없지만, 대화상자 DOM과 이 지역 참조는
      // 시도 직후 해제한다. 최근 문서·URL·저장소·문서 메타데이터에는 전달하지 않는다.
      password = '';
    }
  }
}

export async function loadDocumentForOpen(data: Uint8Array, fileName: string, deps: OpenDocumentDeps): Promise<DocumentInfo> {
  const { wasm } = deps;
  try {
    return wasm.loadDocument(data, fileName);
  } catch (error) {
    if (!isPasswordRequiredError(error)) throw error;
    return loadPasswordProtectedDocument(data, fileName, deps);
  }
}

export function showLoadErrorUnlessCancelled(error: unknown, deps: OpenDocumentDeps): void {
  const { sbMessage } = deps;
  if (isDocumentOpenCancelled(error)) {
    sbMessage().textContent = '문서 열기를 취소했습니다.';
    return;
  }
  showLoadError(error, deps);
}

export async function loadFile(
  file: File,
  options: { skipUnsavedGuard?: boolean; fileHandle?: FileSystemFileHandleLike | null } = {},
  deps: OpenDocumentDeps,
): Promise<boolean> {
  const { canReplaceCurrentDocument, updateLoadProgress } = deps;
  try {
    if (!await canReplaceCurrentDocument(options.skipUnsavedGuard)) return false;
    const startTime = performance.now();
    await updateLoadProgress(0, '파일 읽는 중...');
    const data = new Uint8Array(await file.arrayBuffer());
    await updateLoadProgress(15, '파일 읽기 완료');
    await loadBytes(data, file.name, options.fileHandle ?? null, startTime, { dataReadProgressShown: true }, deps);
    return true;
  } catch (error) {
    showLoadErrorUnlessCancelled(error, deps);
    return false;
  }
}

export async function loadBytes(
  data: Uint8Array,
  fileName: string,
  fileHandle: FileSystemFileHandleLike | null,
  startTime = performance.now(),
  options: { dataReadProgressShown?: boolean; skipRecent?: boolean; suppressDialogs?: boolean } = {},
  deps: OpenDocumentDeps,
): Promise<void> {
  const { wasm, autosaveManager, plugins, updateLoadProgress, initializeDocument, prepareCanvasRendererDocument } = deps;
  if (!options.dataReadProgressShown) {
    await updateLoadProgress(0, '문서 데이터 준비 중...');
  }
  await updateLoadProgress(25, '문서 파싱 및 쪽 계산 중...');
  const docInfo = await loadDocumentForOpen(data, fileName, deps);
  prepareCanvasRendererDocument();
  // 문서가 갈렸다 — 빌린 핸들을 쥔 플러그인에 새 lease 를 준다. 알리지 않으면 그쪽만 옛
  // 문서를 계속 만진다(세대 검사가 잡아 DOCUMENT_RELEASED 로 끊긴다).
  plugins.notifyDocumentSwap();
  await updateLoadProgress(45, '자동 저장 준비 중...');
  forgetConvertedHmlSaveHandle(fileHandle);
  wasm.currentFileHandle = fileHandle;

  // 최근 문서 기록 — 문서 로드 성공 직후, 폰트/모달 등 블로킹 UI 단계 이전에 기록한다.
  // 핸들이 있으면 라이브 재열기용으로 함께 기록하고, 없으면(드롭/input/URL 로드)
  // 메타-only 로 기록한다 — 목록에는 남기되 자동 재열기는 핸들 있는 항목만 가능하다.
  // 자동저장 복구본은 options.skipRecent 로 제외.
  if (!options.skipRecent) {
    void addRecentDoc({
      fileName: wasm.fileName,
      sourceFormat: wasm.getSourceFormat(),
      handle: fileHandle,
    }).catch((err) => console.warn('[recent] 최근 문서 기록 실패:', err));
  }

  await autosaveManager.beginDocument(
    { fileName: wasm.fileName, sourceFormat: wasm.getSourceFormat() },
    { discardPreviousDraft: true },
  );
  await updateLoadProgress(50, '문서 초기화 중...');
  const elapsed = performance.now() - startTime;
  await initializeDocument(docInfo, `${fileName} — ${docInfo.pageCount}페이지 (${elapsed.toFixed(1)}ms)`, {
    suppressDialogs: options.suppressDialogs,
  });
}

/**
 * URL 파라미터(?url=)로 전달된 HWP 파일을 자동 로드한다.
 * Chrome 확장 프로그램에서 뷰어 탭을 열 때 사용.
 */
export async function loadFromUrlParam(deps: OpenDocumentDeps): Promise<void> {
  const { sbMessage } = deps;
  const params = new URLSearchParams(window.location.search);
  const fileUrl = params.get('url');
  if (!fileUrl) return;

  const fileName = params.get('filename') || fileUrl.split('/').pop()?.split('?')[0] || 'document.hwp';
  const msg = sbMessage();

  try {
    msg.textContent = '파일 로딩 중...';
    console.log(`[loadFromUrlParam] ${fileUrl}`);

    let response: Response;

    // Chrome 확장 환경: Service Worker를 통한 CORS 우회 fetch
    if (typeof chrome !== 'undefined' && chrome.runtime?.sendMessage) {
      try {
        response = await fetch(fileUrl);
      } catch {
        // 직접 fetch 실패 시 Service Worker 프록시
        const result = await chrome.runtime.sendMessage({ type: 'fetch-file', url: fileUrl });
        if (result.error) throw new Error(result.error);
        const data = new Uint8Array(result.data);
        assertRemoteDocumentBytes(data);
        await loadBytes(data, fileName, null, undefined, undefined, deps);
        return;
      }
    } else {
      response = await fetch(fileUrl);
    }

    if (!response.ok) throw new Error(`HTTP ${response.status}: ${response.statusText}`);
    const contentType = response.headers.get('content-type');
    const buffer = await response.arrayBuffer();
    const data = new Uint8Array(buffer);
    assertRemoteDocumentBytes(data, contentType);
    await loadBytes(data, fileName, null, undefined, undefined, deps);
  } catch (error) {
    if (isDocumentOpenCancelled(error)) {
      showLoadErrorUnlessCancelled(error, deps);
      return;
    }
    // 로컬 file:// 로드 실패 + "파일 URL 액세스 허용" 미허용 → 전용 안내 (#1131)
    if (fileUrl.startsWith('file:') && typeof chrome !== 'undefined') {
      const allowed = await isFileSchemeAccessAllowed();
      if (allowed === false) {
        showFileUrlAccessGuidance(deps);
        return;
      }
    }
    showLoadErrorUnlessCancelled(error, deps);
  }
}

/**
 * 확장 프로그램의 "파일 URL에 대한 액세스 허용" 권한 상태를 조회한다 (#1131).
 *
 * 확장 페이지에서만 의미가 있다. API 부재(비-확장 환경 등) 시 판정 불가로
 * `null` 을 반환하여 호출부가 기존 동작(일반 에러)으로 폴백하도록 한다.
 *
 * @returns 허용=true, 미허용=false, 판정 불가=null
 */
export async function isFileSchemeAccessAllowed(): Promise<boolean | null> {
  const ext = (typeof chrome !== 'undefined' ? chrome.extension : undefined) as
    | { isAllowedFileSchemeAccess?: () => Promise<boolean> }
    | undefined;
  if (!ext?.isAllowedFileSchemeAccess) return null;
  try {
    return await ext.isAllowedFileSchemeAccess();
  } catch {
    return null;
  }
}

/**
 * 로컬 file:// 문서를 열 때 "파일 URL 액세스 허용" 권한이 꺼져 있어 로드가
 * 실패한 경우, 일반 "Failed to fetch" 대신 원인과 해결 방법을 안내한다 (#1131).
 *
 * 설정 화면(chrome://extensions/?id=...)은 일반 링크로는 열리지 않으므로
 * 확장 컨텍스트의 chrome.tabs.create 로 연다.
 */
export function showFileUrlAccessGuidance(deps: OpenDocumentDeps): void {
  const { sbMessage } = deps;
  const errMsg = '로컬 파일을 열려면 확장 프로그램의 "파일 URL에 대한 액세스 허용"을 켜야 합니다.\n설정에서 권한을 허용한 뒤 파일을 다시 열어 주세요.';
  const sb = sbMessage();
  if (sb) sb.textContent = '파일 로드 실패: 파일 URL 액세스 권한이 필요합니다.';
  console.error('[main] file:// 로드 실패 — 파일 URL 액세스 미허용 (#1131)');
  showToast({
    message: errMsg,
    durationMs: 0, // 사용자가 읽고 직접 닫기
    confirmLabel: '확인',
    action: {
      label: '설정 열기',
      onClick: () => {
        if (typeof chrome !== 'undefined' && chrome.tabs?.create && chrome.runtime?.id) {
          chrome.tabs.create({ url: `chrome://extensions/?id=${chrome.runtime.id}` });
        }
      },
    },
  });
}

/**
 * 파일 로드 실패 시 사용자에게 에러를 명확히 알린다 (#265).
 *
 * 상태 표시줄은 22px 한 줄로 긴 에러 메시지가 ellipsis 로 잘리므로,
 * 우상단 토스트 (긴 메시지 줄바꿈 지원 · 사용자 닫기 · action 링크) 를
 * 병행 사용한다.
 */
export function showLoadError(error: unknown, deps: OpenDocumentDeps): void {
  const { sbMessage } = deps;
  const raw = String(error).replace(/^Error:\s*/, '');
  const errMsg = `파일 로드 실패: ${raw}`;
  const sb = sbMessage();
  if (sb) sb.textContent = errMsg;
  console.error('[main] 파일 로드 실패:', error);
  showToast({
    message: errMsg,
    durationMs: 0, // 에러는 자동 페이드 없음 — 사용자가 읽고 닫기
    confirmLabel: '확인',
  });
}
