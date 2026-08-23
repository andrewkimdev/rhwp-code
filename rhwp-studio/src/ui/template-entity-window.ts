/**
 * hwpx-template-engine `TemplateEntityGenerator`와 바이트 단위로 같은 출력을 서버 왕복 없이
 * 클라이언트에서 보여주는 인앱 오버레이 창 — `CompareResultWindow`(compare-result-window.ts)와
 * 같은 패턴이다(모달이 아니라 `document.body`에 직접 붙는 고정 오버레이). 새 브라우저 탭/창을
 * 열지 않는다 — studio는 PWA·Chrome 확장·hwpx-template-engine 서버에 내장된 `/rhwp`로도
 * 실행되며 그런 맥락에서는 팝업/새 탭이 막히거나 어색하다.
 *
 * `ValidationResultsDialog`(validation-results-dialog.ts)가 같은 소스를 서버 응답에서 보여주는
 * 기존 UX다 — 이 창은 그 자리를 대체하지 않고, 서버 없이도 초안을 볼 수 있는 별도 진입점이다.
 */
import type { WasmBridge } from '@/core/wasm-bridge';
import { highlightJava } from './java-highlight';

export interface TemplateEntityGenResult {
  code: string;
  packageName: string;
  dataClassName: string;
  moduleClassName: string;
  dataClassSource: string;
  moduleClassSource: string;
  errors: string[];
}

type TabKey = 'data' | 'module';

export class TemplateEntityWindow {
  private _open = false;
  private wrap!: HTMLDivElement;
  private codeInput!: HTMLInputElement;
  private packageInput!: HTMLInputElement;
  private tabsEl!: HTMLDivElement;
  private dataTabBtn!: HTMLButtonElement;
  private moduleTabBtn!: HTMLButtonElement;
  private preEl!: HTMLPreElement;
  private codeEl!: HTMLElement;
  private errorsEl!: HTMLDivElement;
  private copyBtn!: HTMLButtonElement;
  private downloadBtn!: HTMLButtonElement;
  private result: TemplateEntityGenResult | null = null;
  private activeTab: TabKey = 'data';

  constructor(private wasm: WasmBridge) {}

  isOpen(): boolean {
    return this._open;
  }

  /** 창을 열고(처음이면 DOM을 만들고) 주어진 code/package로 즉시 생성한다. */
  show(code: string, packageName: string): void {
    if (!this._open) {
      this._open = true;
      this.build();
      document.body.appendChild(this.wrap);
    }
    this.codeInput.value = code;
    this.packageInput.value = packageName;
    this.activeTab = 'data';
    this.generate();
  }

  hide(): void {
    this._open = false;
    this.wrap?.remove();
  }

  private generate(): void {
    const code = this.codeInput.value.trim() || 'template';
    const packageName = this.packageInput.value.trim() || 'com.example.hwpx.templates';
    try {
      this.result = this.wasm.generateTemplateEntity(code, packageName);
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      this.result = {
        code,
        packageName,
        dataClassName: '',
        moduleClassName: '',
        dataClassSource: '',
        moduleClassSource: '',
        errors: [`생성 실패: ${msg}`],
      };
    }
    this.render();
  }

  private render(): void {
    const r = this.result;
    if (!r) return;

    if (r.errors.length > 0) {
      this.errorsEl.style.display = '';
      this.errorsEl.textContent = '';
      const title = document.createElement('div');
      title.className = 'entity-errors-title';
      title.textContent = '표 역할 마커 검증 실패 — 소스를 생성할 수 없습니다.';
      this.errorsEl.appendChild(title);
      const list = document.createElement('ul');
      list.className = 'entity-errors-list';
      for (const e of r.errors) {
        const li = document.createElement('li');
        li.textContent = e;
        list.appendChild(li);
      }
      this.errorsEl.appendChild(list);
      this.tabsEl.style.display = 'none';
      this.preEl.style.display = 'none';
      this.copyBtn.disabled = true;
      this.downloadBtn.disabled = true;
      return;
    }

    this.errorsEl.style.display = 'none';
    this.tabsEl.style.display = '';
    this.preEl.style.display = '';
    this.copyBtn.disabled = false;
    this.downloadBtn.disabled = false;
    this.dataTabBtn.textContent = `${r.dataClassName}.java`;
    this.moduleTabBtn.textContent = `${r.moduleClassName}.java`;
    this.renderActiveTab();
  }

  private renderActiveTab(): void {
    const r = this.result;
    if (!r || r.errors.length > 0) return;
    this.dataTabBtn.classList.toggle('entity-tab--active', this.activeTab === 'data');
    this.moduleTabBtn.classList.toggle('entity-tab--active', this.activeTab === 'module');
    const source = this.activeTab === 'data' ? r.dataClassSource : r.moduleClassSource;
    this.codeEl.innerHTML = highlightJava(source);
  }

  private currentFile(): { name: string; source: string } | null {
    const r = this.result;
    if (!r || r.errors.length > 0) return null;
    return this.activeTab === 'data'
      ? { name: `${r.dataClassName}.java`, source: r.dataClassSource }
      : { name: `${r.moduleClassName}.java`, source: r.moduleClassSource };
  }

  private build(): void {
    this.wrap = document.createElement('div');
    this.wrap.className = 'entity-window';

    const head = document.createElement('div');
    head.className = 'entity-head';
    const titleEl = document.createElement('span');
    titleEl.textContent = 'Java 엔티티 초안';
    const close = document.createElement('button');
    close.className = 'dialog-close';
    close.textContent = '×';
    close.addEventListener('click', () => this.hide());
    head.append(titleEl, close);

    const body = document.createElement('div');
    body.className = 'entity-body';

    const configRow = document.createElement('div');
    configRow.className = 'entity-config-row';
    const codeLabel = document.createElement('label');
    codeLabel.className = 'tp-label';
    codeLabel.textContent = '코드';
    this.codeInput = document.createElement('input');
    this.codeInput.type = 'text';
    this.codeInput.className = 'tp-input entity-code-input';
    this.codeInput.addEventListener('keydown', (e) => {
      if (e.key === 'Enter') this.generate();
    });
    const packageLabel = document.createElement('label');
    packageLabel.className = 'tp-label';
    packageLabel.textContent = '패키지';
    this.packageInput = document.createElement('input');
    this.packageInput.type = 'text';
    this.packageInput.className = 'tp-input entity-package-input';
    this.packageInput.addEventListener('keydown', (e) => {
      if (e.key === 'Enter') this.generate();
    });
    const regenBtn = document.createElement('button');
    regenBtn.type = 'button';
    regenBtn.className = 'tp-btn tp-btn--primary';
    regenBtn.textContent = '다시 생성';
    regenBtn.addEventListener('click', () => this.generate());
    configRow.append(codeLabel, this.codeInput, packageLabel, this.packageInput, regenBtn);

    this.errorsEl = document.createElement('div');
    this.errorsEl.className = 'entity-errors';
    this.errorsEl.style.display = 'none';

    const tabsRow = document.createElement('div');
    tabsRow.className = 'entity-tabs-row';
    this.tabsEl = document.createElement('div');
    this.tabsEl.className = 'entity-tabs';
    this.dataTabBtn = document.createElement('button');
    this.dataTabBtn.type = 'button';
    this.dataTabBtn.className = 'entity-tab';
    this.dataTabBtn.addEventListener('click', () => {
      this.activeTab = 'data';
      this.renderActiveTab();
    });
    this.moduleTabBtn = document.createElement('button');
    this.moduleTabBtn.type = 'button';
    this.moduleTabBtn.className = 'entity-tab';
    this.moduleTabBtn.addEventListener('click', () => {
      this.activeTab = 'module';
      this.renderActiveTab();
    });
    this.tabsEl.append(this.dataTabBtn, this.moduleTabBtn);

    const tabActions = document.createElement('div');
    tabActions.className = 'entity-tab-actions';
    this.copyBtn = document.createElement('button');
    this.copyBtn.type = 'button';
    this.copyBtn.className = 'tp-btn entity-copy-btn';
    this.copyBtn.textContent = '복사';
    this.copyBtn.addEventListener('click', () => this.copyCurrent());
    this.downloadBtn = document.createElement('button');
    this.downloadBtn.type = 'button';
    this.downloadBtn.className = 'tp-btn entity-download-btn';
    this.downloadBtn.textContent = '다운로드';
    this.downloadBtn.addEventListener('click', () => this.downloadCurrent());
    tabActions.append(this.copyBtn, this.downloadBtn);

    tabsRow.append(this.tabsEl, tabActions);

    this.preEl = document.createElement('pre');
    this.preEl.className = 'entity-code';
    this.codeEl = document.createElement('code');
    this.preEl.appendChild(this.codeEl);

    body.append(configRow, this.errorsEl, tabsRow, this.preEl);
    this.wrap.append(head, body);
  }

  private copyCurrent(): void {
    const file = this.currentFile();
    if (!file) return;
    void navigator.clipboard.writeText(file.source).then(
      () => { this.copyBtn.textContent = '복사됨'; setTimeout(() => { this.copyBtn.textContent = '복사'; }, 1500); },
      () => { this.copyBtn.textContent = '복사 실패'; setTimeout(() => { this.copyBtn.textContent = '복사'; }, 1500); },
    );
  }

  private downloadCurrent(): void {
    const file = this.currentFile();
    if (!file) return;
    const blob = new Blob([file.source], { type: 'text/x-java;charset=utf-8' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = file.name;
    a.click();
    URL.revokeObjectURL(url);
  }
}
