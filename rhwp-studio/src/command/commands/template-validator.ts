/**
 * hwpx-template-engine 연동 커맨드 — 현재 문서를 hwpx-template-engine 서버의
 * `/templates/upload/preview`로 보내 표 역할 마커/그림-텍스트 겹침/반복블록 오버플로를 검증하고
 * (`tool:validate-template`), 다섯 마커를 모두 가진 참고용 견본 템플릿을 곧바로 불러온다
 * (`tool:load-sample-template`). 두 커맨드 모두 "도구" 메뉴에 노출된다.
 */
import type { CommandDef, CommandServices } from '../types';
import { exportDocumentForFormat } from '@/command/save-document-format';
import { userSettings } from '@/core/user-settings';
import { showToast } from '@/ui/toast';
import {
  ValidationResultsDialog,
  type TemplateValidationError,
  type TemplateValidationResponse,
} from '@/ui/validation-results-dialog';

/** 견본 템플릿 리소스명 — 서버(hwpx-template-engine)의 화이트리스트 상수와 짝을 이룬다.
 * scslic은 실제 고객명이 들어간 내부 검수용 파일을 임시로 쓰는 것 — 나중에 제네릭한
 * sample-template로 교체되면 이 값 하나만 바꾸면 된다. */
const SAMPLE_TEMPLATE_NAME = 'scslic';

/** 검증 요청에 실어 보내는 owner 식별자 — 이 확장에서 온 드라이런임을 서버 로그에서 구분하기 위함. */
const REQUEST_OWNER = 'rhwp-chrome-extension';

function backendUrl(): string {
  return userSettings.getTemplateValidatorSettings().backendUrl;
}

/**
 * 파일명에서 hwpx-template-engine의 code 제약([a-z0-9_-]+)을 만족하는 값을 만든다.
 *
 * 항상 "_preview" 접미사를 붙인다 — 접미사 없이 파일명을 그대로 code로 쓰면, 이미 컴파일되어
 * 서버에 등록된 실제 운영 템플릿(예: scslic)과 같은 이름의 파일을 열어 검증할 때
 * TemplateUploadHandler가 "예약된 템플릿 코드"로 거부한다(TemplateLookup.isCompiledCode). 이
 * 검증은 어차피 아무것도 게시(publish)하지 않는 드라이런이므로 실제 code를 점유할 필요가 없다.
 */
function codeFromFileName(fileName: string): string {
  const base = fileName.replace(/\.(hwpx?|hml)$/i, '');
  const sanitized = base.toLowerCase().replace(/[^a-z0-9_-]+/g, '_').replace(/^_+|_+$/g, '');
  return `${sanitized || 'template'}_preview`;
}

async function parseErrorResponse(response: Response): Promise<TemplateValidationError> {
  try {
    const data = await response.json() as { markerLintErrors?: string[]; error?: string };
    if (Array.isArray(data.markerLintErrors)) {
      return { markerLintErrors: data.markerLintErrors };
    }
    return { message: data.error ?? `서버 오류 (HTTP ${response.status})` };
  } catch {
    return { message: `서버 오류 (HTTP ${response.status})` };
  }
}

async function validateTemplate(services: CommandServices): Promise<void> {
  const code = codeFromFileName(services.wasm.fileName);
  let hwpxBytes: Uint8Array;
  try {
    hwpxBytes = exportDocumentForFormat(services.wasm, 'hwpx');
  } catch (err) {
    const msg = err instanceof Error ? err.message : String(err);
    showToast({ message: `문서를 hwpx로 내보내지 못했습니다: ${msg}`, durationMs: 3000 });
    return;
  }

  const form = new FormData();
  form.set('code', code);
  form.set('owner', REQUEST_OWNER);
  form.set('file', new Blob([hwpxBytes as unknown as BlobPart], {
    type: 'application/octet-stream',
  }), `${code}.hwpx`);

  // showToast()는 닫기 핸들을 반환하지 않아 fetch 완료 시 프로그램적으로 닫을 수 없다 — "로딩 중"
  // 상태 표시가 아니라 "요청을 보냈다"는 짧은 알림으로만 쓴다. 실제 완료 신호는 결과 다이얼로그다.
  showToast({ message: '템플릿 검증 요청을 보냈습니다...', durationMs: 4000 });
  let dialog: ValidationResultsDialog;
  try {
    const response = await fetch(`${backendUrl()}/templates/upload/preview`, {
      method: 'POST',
      body: form,
    });
    if (!response.ok) {
      dialog = new ValidationResultsDialog(code, null, await parseErrorResponse(response));
    } else {
      const result = await response.json() as TemplateValidationResponse;
      dialog = new ValidationResultsDialog(code, result, null);
    }
  } catch (err) {
    const msg = err instanceof Error ? err.message : String(err);
    dialog = new ValidationResultsDialog(code, null, {
      message: `${backendUrl()}에 연결하지 못했습니다: ${msg} (도구 > 환경 설정 > 템플릿 검증에서 서버 URL을 확인하세요)`,
    });
  }
  dialog.show();
}

async function loadSampleTemplate(services: CommandServices): Promise<void> {
  try {
    const response = await fetch(`${backendUrl()}/templates/samples/${SAMPLE_TEMPLATE_NAME}`);
    if (!response.ok) {
      showToast({ message: `견본 템플릿을 불러오지 못했습니다 (HTTP ${response.status})`, durationMs: 3000 });
      return;
    }
    const bytes = new Uint8Array(await response.arrayBuffer());
    // file.ts의 openFileViaPicker/file:open-recent와 같은 계약 — bytes만 있으면 이 이벤트로
    // 문서를 연다. skipUnsavedGuard를 지정하지 않아 저장되지 않은 변경사항이 있으면 그대로 확인
    // 대화상자가 뜬다.
    services.eventBus.emit('open-document-bytes', {
      bytes,
      fileName: `${SAMPLE_TEMPLATE_NAME}.hwpx`,
      fileHandle: null,
    });
  } catch (err) {
    const msg = err instanceof Error ? err.message : String(err);
    showToast({
      message: `${backendUrl()}에 연결하지 못했습니다: ${msg}`,
      durationMs: 3000,
    });
  }
}

export const templateValidatorCommands: CommandDef[] = [
  {
    id: 'tool:validate-template',
    label: '템플릿 검증',
    canExecute: (ctx) => ctx.hasDocument && ctx.sourceFormat === 'hwpx',
    execute(services) {
      void validateTemplate(services);
    },
  },
  {
    id: 'tool:load-sample-template',
    label: '샘플 템플릿 열기',
    execute(services) {
      void loadSampleTemplate(services);
    },
  },
];
