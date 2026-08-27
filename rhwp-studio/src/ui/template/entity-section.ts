/**
 * 템플릿 패널의 "Java 엔티티 생성" 섹션(panel.ts에서 분리) — 서버 왕복 없이
 * template_entity.rs(TemplateEntityGenerator 포트)로 record 데이터 클래스 +
 * 모듈 클래스 초안을 만든다. 결과는 별도 오버레이 창(entity-window.ts)에 띄운다.
 */
import type { WasmBridge } from '@/core/wasm-bridge';
import { TemplateEntityWindow } from './entity-window';

/**
 * 미리 채워두는 패키지명 — 실제 조직 패키지(`com.ktnet.aspline...`)를 하드코딩하면 다른
 * 조직에서 그대로 컴파일되는 것처럼 보이는 깨지기 쉬운(brittle) 기본값이 되므로, 관례적인
 * `com.example` 로 시작해 사용자가 항상 자기 패키지로 바꿔 써야 함을 드러낸다.
 */
const DEFAULT_ENTITY_PACKAGE = 'com.example.hwpx.templates';

/**
 * 파일명에서 hwpx-template-engine의 code 제약([a-z0-9_-]+)을 만족하는 값을 만든다.
 * `template-validator.ts`의 `codeFromFileName`과 같은 정규화 규칙이지만, 서버에 코드를
 * 예약하는 게 아니라 클라이언트 전용 생성이라 "_preview" 접미사는 붙이지 않는다.
 */
function defaultEntityCodeFromFileName(fileName: string): string {
  const base = fileName.replace(/\.(hwpx?|hml)$/i, '');
  const sanitized = base.toLowerCase().replace(/[^a-z0-9_-]+/g, '_').replace(/^_+|_+$/g, '');
  return sanitized || 'template';
}

export class EntitySection {
  readonly fieldsetEl: HTMLFieldSetElement;
  private codeInput!: HTMLInputElement;
  private packageInput!: HTMLInputElement;
  private generateBtn!: HTMLButtonElement;
  private entityWindow: TemplateEntityWindow;
  /** 사용자가 코드 입력란을 직접 고쳤는지 — 파일명 기본값으로 덮어쓰지 않게 막는다. */
  private codeManuallyEdited = false;

  constructor(private wasm: WasmBridge) {
    this.entityWindow = new TemplateEntityWindow(wasm);

    this.fieldsetEl = document.createElement('fieldset');
    this.fieldsetEl.className = 'tp-role-group';
    const legend = document.createElement('legend');
    legend.className = 'tp-role-group-legend';
    legend.textContent = 'Java 엔티티 생성';
    this.fieldsetEl.appendChild(legend);

    const codeField = document.createElement('div');
    codeField.className = 'tp-field';
    const codeLabel = document.createElement('label');
    codeLabel.className = 'tp-label';
    codeLabel.textContent = '코드';
    this.codeInput = document.createElement('input');
    this.codeInput.type = 'text';
    this.codeInput.className = 'tp-input';
    this.codeInput.addEventListener('input', () => {
      this.codeManuallyEdited = true;
    });
    codeField.appendChild(codeLabel);
    codeField.appendChild(this.codeInput);
    this.fieldsetEl.appendChild(codeField);

    const packageField = document.createElement('div');
    packageField.className = 'tp-field';
    const packageLabel = document.createElement('label');
    packageLabel.className = 'tp-label';
    packageLabel.textContent = '패키지';
    this.packageInput = document.createElement('input');
    this.packageInput.type = 'text';
    this.packageInput.className = 'tp-input';
    this.packageInput.value = DEFAULT_ENTITY_PACKAGE;
    packageField.appendChild(packageLabel);
    packageField.appendChild(this.packageInput);
    this.fieldsetEl.appendChild(packageField);

    this.generateBtn = document.createElement('button');
    this.generateBtn.type = 'button';
    this.generateBtn.className = 'tp-btn tp-btn--primary';
    this.generateBtn.textContent = 'Java 엔티티 생성';
    this.generateBtn.title =
      '표 역할 마커(#REPEAT-*, #PAGENO)와 누름틀 이름에서 hwpx-template-engine의 '
      + 'TemplateEntityGenerator와 같은 record 데이터 클래스 + 모듈 클래스 초안을 만듭니다.';
    this.generateBtn.addEventListener('click', () => {
      const code = this.codeInput.value.trim() || defaultEntityCodeFromFileName(this.wasm.fileName);
      const packageName = this.packageInput.value.trim() || DEFAULT_ENTITY_PACKAGE;
      this.entityWindow.show(code, packageName);
    });
    this.fieldsetEl.appendChild(this.generateBtn);
  }

  /** panel.refresh()가 문서 상태를 반영해 부른다 — hwpx 게이트 + 코드 기본값. */
  refresh(isHwpx: boolean, fileName: string): void {
    this.generateBtn.disabled = !isHwpx;
    this.generateBtn.title = isHwpx
      ? ''
      : 'hwpx 문서에서만 사용할 수 있습니다(누름틀 스키마는 hwpx 마커를 기준으로 합니다).';
    if (!this.codeManuallyEdited) {
      this.codeInput.value = defaultEntityCodeFromFileName(fileName);
    }
  }
}
