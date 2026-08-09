/**
 * hwpx-template-engine 서버의 `/templates/upload/preview` 응답을 사람이 읽을 수 있게 보여주는
 * 결과 다이얼로그 — 표 역할 마커 오타, 그림-텍스트 겹침, 반복블록 오버플로 경고, 생성된 Java DTO
 * 초안을 한 화면에서 확인한다. v1은 항목을 눌러 문서 내 위치로 이동하는 기능은 없다 — Java
 * 검증기의 식별자(표 인덱스/셀 좌표)를 rhwp 문서 IR의 스크롤 위치로 매핑하는 작업은 별도 후속
 * 과제다.
 */
import { ModalDialog } from './dialog';

export interface TemplateValidationSchema {
  code: string;
  name: string;
  fields: string[];
  totalPagesFields: string[];
  currentPageFields: string[];
  repeatBlocks: { name: string; fields: string[]; seqFields: string[] }[];
}

export interface TemplateValidationResponse {
  code: string;
  schema: TemplateValidationSchema;
  sampleDtoSource: string;
  pictureOverlapWarnings: string[];
  repeatOverflowWarnings: string[];
}

export interface TemplateValidationError {
  /** hwpx-template-engine이 마커 린트 실패로 보낸 구조화 오류 (400 + markerLintErrors). */
  markerLintErrors?: string[];
  /** 그 외 서버/네트워크 오류 메시지. */
  message?: string;
}

export class ValidationResultsDialog extends ModalDialog {
  constructor(
    private readonly templateCode: string,
    private readonly result: TemplateValidationResponse | null,
    private readonly error: TemplateValidationError | null,
  ) {
    super('템플릿 검증 결과', 640);
  }

  protected createBody(): HTMLElement {
    const body = document.createElement('div');
    body.className = 'validate-body';

    if (this.error) {
      body.appendChild(this.createErrorSection(this.error));
      return body;
    }
    if (!this.result) {
      body.appendChild(this.createEmptyState('결과를 표시할 수 없습니다.'));
      return body;
    }

    const { schema, pictureOverlapWarnings, repeatOverflowWarnings, sampleDtoSource } = this.result;
    const hasWarnings = pictureOverlapWarnings.length > 0 || repeatOverflowWarnings.length > 0;

    body.appendChild(this.createSummarySection(schema, hasWarnings));
    if (pictureOverlapWarnings.length > 0) {
      body.appendChild(this.createWarningSection('그림-텍스트 겹침', pictureOverlapWarnings));
    }
    if (repeatOverflowWarnings.length > 0) {
      body.appendChild(this.createWarningSection('반복블록 오버플로 시나리오에서의 겹침', repeatOverflowWarnings));
    }
    body.appendChild(this.createDtoSection(sampleDtoSource));

    return body;
  }

  private createErrorSection(error: TemplateValidationError): HTMLElement {
    const section = document.createElement('div');
    section.className = 'dialog-section';

    const title = document.createElement('div');
    title.className = 'dialog-section-title';
    title.textContent = error.markerLintErrors ? '표 역할 마커 오타' : '검증 실패';
    section.appendChild(title);

    if (error.markerLintErrors?.length) {
      section.appendChild(this.createList(error.markerLintErrors, 'validate-error-item'));
    } else {
      const msg = document.createElement('p');
      msg.className = 'opt-desc';
      msg.textContent = error.message ?? '알 수 없는 오류입니다.';
      section.appendChild(msg);
    }
    return section;
  }

  private createSummarySection(schema: TemplateValidationSchema, hasWarnings: boolean): HTMLElement {
    const section = document.createElement('div');
    section.className = 'dialog-section';

    const title = document.createElement('div');
    title.className = 'dialog-section-title';
    title.textContent = `${this.templateCode} — 스키마 요약`;
    section.appendChild(title);

    const summary = document.createElement('p');
    summary.className = 'opt-desc';
    const repeatNames = schema.repeatBlocks.map((b) => b.name).join(', ') || '없음';
    summary.textContent =
      `필드 ${schema.fields.length}개, 반복블록 ${schema.repeatBlocks.length}개(${repeatNames})`;
    section.appendChild(summary);

    if (!hasWarnings) {
      const ok = document.createElement('p');
      ok.className = 'validate-ok';
      ok.textContent = '마커 오타, 그림-텍스트 겹침, 반복블록 오버플로 문제를 찾지 못했습니다.';
      section.appendChild(ok);
    }
    return section;
  }

  private createWarningSection(title: string, items: string[]): HTMLElement {
    const section = document.createElement('div');
    section.className = 'dialog-section';

    const titleEl = document.createElement('div');
    titleEl.className = 'dialog-section-title';
    titleEl.textContent = `${title} (${items.length}건)`;
    section.appendChild(titleEl);
    section.appendChild(this.createList(items, 'validate-warning-item'));
    return section;
  }

  private createList(items: string[], itemClass: string): HTMLElement {
    const list = document.createElement('ul');
    list.className = 'validate-warning-list';
    for (const item of items) {
      const li = document.createElement('li');
      li.className = itemClass;
      li.textContent = item;
      list.appendChild(li);
    }
    return list;
  }

  private createDtoSection(sampleDtoSource: string): HTMLElement {
    const section = document.createElement('div');
    section.className = 'dialog-section';

    const header = document.createElement('div');
    header.className = 'validate-dto-header';

    const title = document.createElement('div');
    title.className = 'dialog-section-title';
    title.textContent = '생성된 Java DTO (초안)';
    header.appendChild(title);

    const copyBtn = document.createElement('button');
    copyBtn.className = 'dialog-btn validate-copy-btn';
    copyBtn.textContent = '복사';
    copyBtn.addEventListener('click', () => {
      void navigator.clipboard.writeText(sampleDtoSource).then(
        () => { copyBtn.textContent = '복사됨'; setTimeout(() => { copyBtn.textContent = '복사'; }, 1500); },
        () => { copyBtn.textContent = '복사 실패'; setTimeout(() => { copyBtn.textContent = '복사'; }, 1500); },
      );
    });
    header.appendChild(copyBtn);
    section.appendChild(header);

    const pre = document.createElement('pre');
    pre.className = 'validate-dto-code';
    pre.textContent = sampleDtoSource;
    section.appendChild(pre);

    return section;
  }

  private createEmptyState(message: string): HTMLElement {
    const p = document.createElement('p');
    p.className = 'opt-desc';
    p.textContent = message;
    return p;
  }

  protected onConfirm(): void {
    // 결과 표시 전용 — 확인 동작 없음
  }

  override show(): void {
    super.show();
    const footer = this.dialog.querySelector('.dialog-footer');
    if (footer) {
      footer.replaceChildren();
      const closeBtn = document.createElement('button');
      closeBtn.className = 'dialog-btn dialog-btn-primary';
      closeBtn.textContent = '닫기';
      closeBtn.addEventListener('click', () => this.hide());
      footer.appendChild(closeBtn);
    }
  }
}
