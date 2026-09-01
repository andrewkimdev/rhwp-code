/**
 * main.ts UI 배선 — 전역 단축키·파일 입력·줌 컨트롤·이벤트 리스너 setup 무리.
 *
 * main.ts 의 모듈 싱글톤(wasm·eventBus·dispatcher 등)을 명시 매개변수로 받는
 * 자유함수로 분리했다. 본문은 원본 main.ts 에서 동결 이동이며, 예외는
 * setupEventListeners 의 2줄뿐이다 — renderBackendFallbackReason 대입 →
 * setter 호출, totalSections 읽기 → getter 호출(둘 다 main.ts 모듈 상태라
 * setup 시점 스냅샷이 아니라 살아있는 접근이 필요).
 */

/* eslint-disable @typescript-eslint/no-explicit-any */

import { WasmBridge } from '@/core/wasm-bridge';
import { EventBus } from '@/core/event-bus';
import { DocumentDirtyState } from '@/core/document-dirty-state';
import type { InputHandler } from '@/engine/input-handler';
import type { CanvasView } from '@/view/canvas-view';
import type { CommandDispatcher } from '@/command/dispatcher';
import type { AutosaveManager, AutosaveScheduleSettings } from '@/recovery/autosave-manager';
import type { RendererSessionDiagnostics } from '@/view/renderer-session';
import type { RenderBackendFallbackReason } from '@/view/render-backend';
import type { resolveChromeModeRequest } from '@/ui/chrome-mode';
import { isSupportedDocumentFileName } from '@/command/file-system-access';
import { showToast } from '@/ui/toast';
import { applyDocumentTitle } from '@/app/document-title';
import { calculateFitPageZoom, calculateFitWidthZoom } from '@/view/zoom-fit';

export function setupGlobalShortcuts(inputHandler: InputHandler | null, dispatcher: CommandDispatcher): void {
  document.addEventListener('keydown', (e) => {
    // input/textarea 등 편집 가능 요소 내부에서는 무시
    const target = e.target as HTMLElement;
    if (target instanceof HTMLInputElement || target instanceof HTMLTextAreaElement) return;
    // InputHandler가 활성 상태이면 자체 처리에 맡김
    if (inputHandler?.isActive()) return;

    const ctrlOrMeta = e.ctrlKey || e.metaKey;

    // Alt+N / Alt+ㅜ → 새 문서 (문서 미로드 상태에서도 동작)
    if (e.altKey && !ctrlOrMeta && !e.shiftKey) {
      if (e.key === 'n' || e.key === 'N' || e.key === 'ㅜ') {
        e.preventDefault();
        dispatcher.dispatch('file:new-doc');
        return;
      }
    }
    // Ctrl/Cmd+O → 열기 (문서 미로드 상태에서도 동작)
    if (ctrlOrMeta && !e.altKey && !e.shiftKey) {
      if (e.key === 'o' || e.key === 'O' || e.key === 'ㅐ') {
        e.preventDefault();
        dispatcher.dispatch('file:open');
        return;
      }
    }
  }, false);
}

export function setupFileInput(
  chromeMode: ReturnType<typeof resolveChromeModeRequest>['mode'],
  wasm: WasmBridge,
  inputHandler: InputHandler | null,
  loadFile: (file: File, options?: { skipUnsavedGuard?: boolean }) => Promise<boolean>,
): void {
  const fileInput = document.getElementById('file-input') as HTMLInputElement;

  fileInput.addEventListener('change', async (e) => {
    const input = e.target as HTMLInputElement;
    const skipUnsavedGuard = input.dataset.skipUnsavedGuard === 'true';
    delete input.dataset.skipUnsavedGuard;
    const file = input.files?.[0];
    if (!file) return;
    if (!isSupportedDocumentFileName(file.name)) {
      alert('HWP/HWPX/HML 파일만 지원합니다.');
      fileInput.value = '';
      return;
    }
    await loadFile(file, { skipUnsavedGuard });
    fileInput.value = '';
  });

  // 문서 전체에서 브라우저 기본 드롭 동작 방지 (파일 열기/다운로드 방지)
  document.addEventListener('dragover', (e) => e.preventDefault());
  document.addEventListener('drop', (e) => e.preventDefault());

  // 드래그 앤 드롭 지원 (scroll-container 영역)
  const container = document.getElementById('scroll-container')!;
  container.addEventListener('dragover', (e) => {
    e.preventDefault();
    container.classList.add('drag-over');
  });
  container.addEventListener('dragleave', () => {
    container.classList.remove('drag-over');
  });
  container.addEventListener('drop', async (e) => {
    e.preventDefault();
    container.classList.remove('drag-over');
    const file = e.dataTransfer?.files[0];
    if (!file) return;
    const dropName = file.name.toLowerCase();
    const imageExts = ['.png', '.jpg', '.jpeg', '.gif', '.bmp', '.webp'];
    const isImage = imageExts.some(ext => dropName.endsWith(ext));
    const isDoc = isSupportedDocumentFileName(dropName);
    // embed 프로파일: 문서 드롭은 호스트가 감지할 수 없는 문서 교체 경로이므로 무시한다.
    // 이미지 드롭은 수명주기가 아니라 편집 기능이라 그대로 둔다.
    if (chromeMode === 'embed' && isDoc) return;
    if (!isImage && !isDoc) {
      alert('HWP/HWPX/HML 파일 또는 이미지 파일만 지원합니다.');
      return;
    }

    if (isImage) {
      if (!inputHandler || wasm.pageCount === 0) return;
      const data = new Uint8Array(await file.arrayBuffer());
      const ext = file.name.split('.').pop()?.toLowerCase() || 'png';
      const img = new Image();
      const url = URL.createObjectURL(file);
      try {
        img.src = url;
        await img.decode();
        const result = inputHandler.insertDroppedImageAtClientPoint(
          data,
          ext,
          img.naturalWidth,
          img.naturalHeight,
          file.name,
          e.clientX,
          e.clientY,
        );
        if (!result.ok) {
          showToast({
            message: `그림 삽입에 실패했습니다.\n${result.error ?? '삽입 위치 또는 이미지 정보를 확인할 수 없습니다.'}`,
            durationMs: 6000,
          });
        }
      } catch {
        console.warn('[drop] 이미지 디코딩 실패:', file.name);
        showToast({
          message: '그림을 삽입할 수 없습니다.\n브라우저가 이 이미지 파일을 읽지 못했습니다.',
          durationMs: 6000,
        });
      } finally {
        URL.revokeObjectURL(url);
      }
      return;
    }

    // HWP/HWPX/HML — Finder/Explorer drop에서는 File System Access handle을 capture하지
    // 않는다. macOS Chromium에서 encrypted HWPX drag/drop 시 해당 IPC가 renderer를 종료시키는
    // 사례가 있어, 열기에 충분한 File bytes만 사용한다. 저장은 이후 save-as 경로로 진행한다.
    await loadFile(file);
  });
}

export function setupZoomControls(canvasView: CanvasView | null, wasm: WasmBridge): void {
  if (!canvasView) return;
  const vm = canvasView.getViewportManager();

  document.getElementById('sb-zoom-in')!.addEventListener('click', () => {
    vm.smoothZoomBy(0.1);
  });
  document.getElementById('sb-zoom-out')!.addEventListener('click', () => {
    vm.smoothZoomBy(-0.1);
  });

  // 폭 맞춤: 용지 폭에 맞게 줌 조절
  document.getElementById('sb-zoom-fit-width')!.addEventListener('click', () => {
    if (wasm.pageCount === 0) return;
    const container = document.getElementById('scroll-container')!;
    const pageInfo = wasm.getPageInfo(0);
    // pageInfo.width는 이미 px 단위 (96dpi 기준)
    const zoom = calculateFitWidthZoom(container.clientWidth, pageInfo.width);
    console.log(`[zoom-fit-width] container=${container.clientWidth} page=${pageInfo.width} zoom=${zoom.toFixed(3)}`);
    vm.setZoom(zoom);
  });

  // 쪽 맞춤: 한 페이지 전체가 보이도록 줌 조절
  document.getElementById('sb-zoom-fit')!.addEventListener('click', () => {
    if (wasm.pageCount === 0) return;
    const container = document.getElementById('scroll-container')!;
    const pageInfo = wasm.getPageInfo(0);
    // pageInfo.width/height는 이미 px 단위 (96dpi 기준)
    const zoom = calculateFitPageZoom(
      container.clientWidth,
      container.clientHeight,
      pageInfo.width,
      pageInfo.height,
    );
    console.log(`[zoom-fit-page] containerW=${container.clientWidth} containerH=${container.clientHeight} pageW=${pageInfo.width} pageH=${pageInfo.height} zoom=${zoom.toFixed(3)}`);
    vm.setZoom(zoom);
  });

  // 모바일: 줌 값 클릭 → 100% 토글
  document.getElementById('sb-zoom-val')!.addEventListener('click', () => {
    const currentZoom = vm.getZoom();
    if (Math.abs(currentZoom - 1.0) < 0.05) {
      // 현재 100% → 쪽 맞춤으로 전환
      document.getElementById('sb-zoom-fit')!.click();
    } else {
      // 현재 쪽 맞춤/기타 → 100%로 전환
      vm.setZoom(1.0);
    }
  });

  document.addEventListener('keydown', (e) => {
    if (!e.ctrlKey && !e.metaKey) return;
    if (e.key === '=' || e.key === '+') {
      e.preventDefault();
      vm.smoothZoomBy(0.1);
    } else if (e.key === '-') {
      e.preventDefault();
      vm.smoothZoomBy(-0.1);
    } else if (e.key === '0') {
      e.preventDefault();
      vm.setZoom(1.0);
    }
  });
}

export function setupEventListeners(
  eventBus: EventBus,
  dispatcher: CommandDispatcher,
  wasm: WasmBridge,
  documentState: DocumentDirtyState,
  autosaveManager: AutosaveManager,
  sbPage: () => HTMLElement,
  sbSection: () => HTMLElement,
  sbZoomVal: () => HTMLElement,
  autosaveScheduleFromUserSettings: () => AutosaveScheduleSettings,
  setRenderBackendFallbackReason: (reason: RenderBackendFallbackReason | null) => void,
  getTotalSections: () => number,
): void {
  sbPage().addEventListener('click', () => {
    dispatcher.dispatch('edit:goto');
  });

  eventBus.on('current-page-changed', (page, _total) => {
    const pageIdx = page as number;
    sbPage().textContent = `${pageIdx + 1} / ${_total} 쪽`;

    // 구역 정보: 현재 페이지의 sectionIndex로 갱신
    if (wasm.pageCount > 0) {
      try {
        const pageInfo = wasm.getPageInfo(pageIdx);
        sbSection().textContent = `구역: ${pageInfo.sectionIndex + 1} / ${getTotalSections()}`;
      } catch { /* 무시 */ }
    }
  });

  eventBus.on('zoom-level-display', (zoom) => {
    sbZoomVal().textContent = `${Math.round((zoom as number) * 100)}%`;
  });

  // 삽입/수정 모드 토글
  eventBus.on('insert-mode-changed', (insertMode) => {
    document.getElementById('sb-mode')!.textContent = (insertMode as boolean) ? '삽입' : '수정';
  });

  eventBus.on('document-mutated', (reason) => {
    documentState.markDirty(typeof reason === 'string' ? reason : 'document-mutated');
  });

  eventBus.on('document-changed', (reason) => {
    documentState.markDirty(typeof reason === 'string' ? reason : 'document-changed');
  });

  eventBus.on('renderer-selection-changed', (payload) => {
    const diagnostics = payload as RendererSessionDiagnostics;
    setRenderBackendFallbackReason(diagnostics.fallbackReason);
    if (import.meta.env.DEV) {
      (window as any).__renderBackend = diagnostics.effectiveBackend;
      (window as any).__renderBackendFallbackReason = diagnostics.fallbackReason;
      (window as any).__rendererSelection = diagnostics;
    }
  });

  eventBus.on('document-dirty-changed', () => {
    eventBus.emit('command-state-changed');
    // 탭 제목의 '*' 마커 갱신 — 파일명은 이 시점 값 기준으로 함께 다시 쓴다.
    applyDocumentTitle(wasm.fileName, documentState.isDirty());
  });

  eventBus.on('autosave-settings-changed', () => {
    autosaveManager.updateSchedule(autosaveScheduleFromUserSettings());
  });

  // 필드 정보 표시
  const sbField = document.getElementById('sb-field');
  eventBus.on('field-info-changed', (info) => {
    if (!sbField) return;
    const fi = info as { fieldId: number; fieldType: string; guideName?: string } | null;
    if (fi) {
      const label = fi.guideName || `#${fi.fieldId}`;
      sbField.textContent = `[누름틀] ${label}`;
      sbField.style.display = '';
    } else {
      sbField.textContent = '';
      sbField.style.display = 'none';
    }
  });

  // 개체 선택 시 회전/대칭 버튼 그룹 표시/숨김
  const rotateGroup = document.querySelector('.tb-rotate-group') as HTMLElement | null;
  let noteToolbarActive = false;
  if (rotateGroup) {
    eventBus.on('picture-object-selection-changed', (selected) => {
      rotateGroup.style.display = (selected as boolean) && !noteToolbarActive ? '' : 'none';
    });
  }

  // 머리말/꼬리말 편집 모드 시 도구상자 전환 + 본문 dimming
  const hfGroup = document.querySelector('.tb-headerfooter-group') as HTMLElement | null;
  const hfLabel = hfGroup?.querySelector('.tb-hf-label') as HTMLElement | null;
  const noteGroup = document.querySelector('.tb-note-group') as HTMLElement | null;
  const defaultTbGroups = document.querySelectorAll('#icon-toolbar > .tb-group:not(.tb-headerfooter-group):not(.tb-note-group):not(.tb-rotate-group), #icon-toolbar > .tb-sep');
  const scrollContainer = document.getElementById('scroll-container');
  const styleBar = document.getElementById('style-bar');

  eventBus.on('headerFooterModeChanged', (mode) => {
    const isActive = (mode as string) !== 'none';
    // 도구상자 전환
    if (hfGroup) {
      hfGroup.style.display = isActive ? '' : 'none';
    }
    if (hfLabel) {
      hfLabel.textContent = (mode as string) === 'header' ? '머리말' : (mode as string) === 'footer' ? '꼬리말' : '';
    }
    defaultTbGroups.forEach((el) => {
      (el as HTMLElement).style.display = isActive ? 'none' : '';
    });
    // 서식 도구 모음은 머리말/꼬리말 편집 시에도 유지 (문단/글자 모양 설정 필요)
    // 본문 dimming
    if (scrollContainer) {
      if (isActive) {
        scrollContainer.classList.add('hf-editing');
      } else {
        scrollContainer.classList.remove('hf-editing');
      }
    }
  });

  eventBus.on('footnoteModeChanged', (active) => {
    const isActive = active as boolean;
    noteToolbarActive = isActive;
    if (noteGroup) {
      noteGroup.style.display = isActive ? '' : 'none';
    }
    if (rotateGroup && isActive) {
      rotateGroup.style.display = 'none';
    }
    defaultTbGroups.forEach((el) => {
      (el as HTMLElement).style.display = isActive ? 'none' : '';
    });
  });
}
