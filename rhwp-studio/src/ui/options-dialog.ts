/**
 * 환경 설정 대화상자 (도구 > 환경 설정)
 *
 * 탭 구조: [글꼴] (향후 [편집], [보기] 등 탭 추가 가능)
 */
import { ModalDialog } from './dialog';
import { userSettings } from '@/core/user-settings';
import { FontSetDialog } from './font-set-dialog';
import type { EventBus } from '@/core/event-bus';

export class OptionsDialog extends ModalDialog {
  private showRecentCheck!: HTMLInputElement;
  private recentCountInput!: HTMLInputElement;
  private recoveryEnabledCheck!: HTMLInputElement;
  private recoveryIntervalInput!: HTMLInputElement;
  private idleSaveEnabledCheck!: HTMLInputElement;
  private idleDelayInput!: HTMLInputElement;
  private pdfPrintGuidanceCheck!: HTMLInputElement;
  private backendUrlInput!: HTMLInputElement;

  constructor(private readonly eventBus?: EventBus) {
    super('환경 설정', 480);
  }

  protected createBody(): HTMLElement {
    const body = document.createElement('div');
    body.className = 'opt-body';

    // 탭 헤더
    const tabs = document.createElement('div');
    tabs.className = 'dialog-tabs';

    const fontTab = document.createElement('button');
    fontTab.className = 'dialog-tab active';
    fontTab.textContent = '글꼴';
    fontTab.dataset.tab = 'font';
    tabs.appendChild(fontTab);

    const fileTab = document.createElement('button');
    fileTab.className = 'dialog-tab';
    fileTab.textContent = '파일';
    fileTab.dataset.tab = 'file';
    tabs.appendChild(fileTab);

    const templateTab = document.createElement('button');
    templateTab.className = 'dialog-tab';
    templateTab.textContent = '템플릿 검증';
    templateTab.dataset.tab = 'template';
    tabs.appendChild(templateTab);

    body.appendChild(tabs);

    // 글꼴 탭 패널
    const fontPanel = this.createFontPanel();
    fontPanel.className = 'dialog-tab-panel opt-tab-panel active';
    fontPanel.dataset.tab = 'font';
    body.appendChild(fontPanel);

    const filePanel = this.createFilePanel();
    filePanel.className = 'dialog-tab-panel opt-tab-panel';
    filePanel.dataset.tab = 'file';
    body.appendChild(filePanel);

    const templatePanel = this.createTemplateValidatorPanel();
    templatePanel.className = 'dialog-tab-panel opt-tab-panel';
    templatePanel.dataset.tab = 'template';
    body.appendChild(templatePanel);

    // 탭 클릭 이벤트 (향후 탭 추가 대비)
    tabs.addEventListener('click', (e) => {
      const btn = (e.target as HTMLElement).closest('.dialog-tab') as HTMLElement | null;
      if (!btn) return;
      const tabId = btn.dataset.tab;
      tabs.querySelectorAll('.dialog-tab').forEach(t => t.classList.remove('active'));
      body.querySelectorAll('.dialog-tab-panel').forEach(p => p.classList.remove('active'));
      btn.classList.add('active');
      const panel = body.querySelector(`.dialog-tab-panel[data-tab="${tabId}"]`);
      panel?.classList.add('active');
    });

    return body;
  }

  private createFontPanel(): HTMLElement {
    const panel = document.createElement('div');
    const fs = userSettings.getFontSettings();

    // ── 글꼴 보기 섹션 ──
    const viewSection = document.createElement('div');
    viewSection.className = 'dialog-section';

    const viewTitle = document.createElement('div');
    viewTitle.className = 'dialog-section-title';
    viewTitle.textContent = '글꼴 보기';
    viewSection.appendChild(viewTitle);

    // 최근 사용 글꼴 보이기
    const recentRow = document.createElement('div');
    recentRow.className = 'dialog-row opt-row';

    this.showRecentCheck = document.createElement('input');
    this.showRecentCheck.type = 'checkbox';
    this.showRecentCheck.id = 'opt-show-recent';
    this.showRecentCheck.checked = fs.showRecentFonts;

    const recentLabel = document.createElement('label');
    recentLabel.htmlFor = 'opt-show-recent';
    recentLabel.textContent = '최근에 사용한 글꼴 보이기';

    this.recentCountInput = document.createElement('input');
    this.recentCountInput.type = 'number';
    this.recentCountInput.className = 'dialog-input opt-count-input';
    this.recentCountInput.min = '1';
    this.recentCountInput.max = '5';
    this.recentCountInput.value = String(fs.recentFontCount);

    const countLabel = document.createElement('span');
    countLabel.className = 'opt-count-label';
    countLabel.textContent = '개';

    recentRow.appendChild(this.showRecentCheck);
    recentRow.appendChild(recentLabel);
    recentRow.appendChild(this.recentCountInput);
    recentRow.appendChild(countLabel);
    viewSection.appendChild(recentRow);

    panel.appendChild(viewSection);

    // ── 대표 글꼴 등록 섹션 ──
    const fontSetSection = document.createElement('div');
    fontSetSection.className = 'dialog-section';

    const fontSetTitle = document.createElement('div');
    fontSetTitle.className = 'dialog-section-title';
    fontSetTitle.textContent = '대표 글꼴 등록';
    fontSetSection.appendChild(fontSetTitle);

    const fontSetDesc = document.createElement('p');
    fontSetDesc.className = 'opt-desc';
    fontSetDesc.textContent = '대표 글꼴은 각 언어별 글꼴을 짝지어 한 번에 적용하는 글꼴 세트입니다.';
    fontSetSection.appendChild(fontSetDesc);

    const fontSetBtn = document.createElement('button');
    fontSetBtn.className = 'dialog-btn opt-fontset-btn';
    fontSetBtn.textContent = '대표 글꼴 등록하기';
    fontSetBtn.addEventListener('click', () => {
      const dlg = new FontSetDialog();
      dlg.show();
    });
    fontSetSection.appendChild(fontSetBtn);

    panel.appendChild(fontSetSection);

    return panel;
  }

  private createFilePanel(): HTMLElement {
    const panel = document.createElement('div');
    const autosave = userSettings.getAutosaveSettings();
    const dialogSettings = userSettings.getDialogSettings();

    const saveSection = document.createElement('div');
    saveSection.className = 'dialog-section';

    const saveTitle = document.createElement('div');
    saveTitle.className = 'dialog-section-title';
    saveTitle.textContent = '복구용 임시 파일 자동 저장';
    saveSection.appendChild(saveTitle);

    const desc = document.createElement('p');
    desc.className = 'opt-desc';
    desc.textContent = '대형 문서는 자동저장 시 전체 HWP 복구본을 만들기 때문에 간격을 길게 두면 편집 중 멈춤을 줄일 수 있습니다.';
    saveSection.appendChild(desc);

    this.recoveryEnabledCheck = document.createElement('input');
    this.recoveryEnabledCheck.type = 'checkbox';
    this.recoveryEnabledCheck.id = 'opt-recovery-enabled';
    this.recoveryEnabledCheck.checked = autosave.recoveryEnabled;

    this.recoveryIntervalInput = document.createElement('input');
    this.recoveryIntervalInput.type = 'number';
    this.recoveryIntervalInput.className = 'dialog-input opt-interval-input';
    this.recoveryIntervalInput.min = '1';
    this.recoveryIntervalInput.max = '120';
    this.recoveryIntervalInput.value = String(autosave.recoveryIntervalMinutes);

    saveSection.appendChild(createAutosaveNumberRow({
      checkbox: this.recoveryEnabledCheck,
      labelText: '복구용 자동 저장',
      numberInput: this.recoveryIntervalInput,
      unitText: '분',
    }));

    this.idleSaveEnabledCheck = document.createElement('input');
    this.idleSaveEnabledCheck.type = 'checkbox';
    this.idleSaveEnabledCheck.id = 'opt-idle-save-enabled';
    this.idleSaveEnabledCheck.checked = autosave.idleSaveEnabled;

    this.idleDelayInput = document.createElement('input');
    this.idleDelayInput.type = 'number';
    this.idleDelayInput.className = 'dialog-input opt-interval-input';
    this.idleDelayInput.min = '5';
    this.idleDelayInput.max = '600';
    this.idleDelayInput.value = String(autosave.idleDelaySeconds);

    saveSection.appendChild(createAutosaveNumberRow({
      checkbox: this.idleSaveEnabledCheck,
      labelText: '쉴 때 자동 저장',
      numberInput: this.idleDelayInput,
      unitText: '초',
    }));

    const syncDisabled = (): void => {
      this.recoveryIntervalInput.disabled = !this.recoveryEnabledCheck.checked;
      this.idleDelayInput.disabled = !this.idleSaveEnabledCheck.checked;
    };
    this.recoveryEnabledCheck.addEventListener('change', syncDisabled);
    this.idleSaveEnabledCheck.addEventListener('change', syncDisabled);
    syncDisabled();

    panel.appendChild(saveSection);

    const pdfSection = document.createElement('div');
    pdfSection.className = 'dialog-section';

    const pdfTitle = document.createElement('div');
    pdfTitle.className = 'dialog-section-title';
    pdfTitle.textContent = 'PDF 저장';
    pdfSection.appendChild(pdfTitle);

    const pdfDesc = document.createElement('p');
    pdfDesc.className = 'opt-desc';
    pdfDesc.textContent =
      '안내를 끄면 PDF로 저장을 선택하는 즉시 문서 준비가 시작됩니다. 준비 진행률과 오류는 계속 표시됩니다.';
    pdfSection.appendChild(pdfDesc);

    const pdfRow = document.createElement('div');
    pdfRow.className = 'dialog-row opt-row';

    this.pdfPrintGuidanceCheck = document.createElement('input');
    this.pdfPrintGuidanceCheck.type = 'checkbox';
    this.pdfPrintGuidanceCheck.id = 'opt-pdf-print-guidance';
    this.pdfPrintGuidanceCheck.checked = dialogSettings.showPdfPrintGuidance;

    const pdfLabel = document.createElement('label');
    pdfLabel.htmlFor = 'opt-pdf-print-guidance';
    pdfLabel.textContent = 'PDF로 저장할 때 저장 방법 안내 표시';

    pdfRow.append(this.pdfPrintGuidanceCheck, pdfLabel);
    pdfSection.appendChild(pdfRow);
    panel.appendChild(pdfSection);

    return panel;
  }

  private createTemplateValidatorPanel(): HTMLElement {
    const panel = document.createElement('div');
    const settings = userSettings.getTemplateValidatorSettings();

    const section = document.createElement('div');
    section.className = 'dialog-section';

    const title = document.createElement('div');
    title.className = 'dialog-section-title';
    title.textContent = 'hwpx-template-engine 서버';
    section.appendChild(title);

    const desc = document.createElement('p');
    desc.className = 'opt-desc';
    desc.textContent = '도구 > 템플릿 검증 / 샘플 템플릿 열기가 호출할 hwpx-template-engine 서버 주소입니다.';
    section.appendChild(desc);

    const row = document.createElement('div');
    row.className = 'dialog-row opt-row';

    const label = document.createElement('label');
    label.htmlFor = 'opt-template-backend-url';
    label.textContent = '서버 URL';

    this.backendUrlInput = document.createElement('input');
    this.backendUrlInput.type = 'text';
    this.backendUrlInput.id = 'opt-template-backend-url';
    this.backendUrlInput.className = 'dialog-input opt-backend-url-input';
    this.backendUrlInput.placeholder = 'http://localhost:8080';
    this.backendUrlInput.value = settings.backendUrl;

    row.appendChild(label);
    row.appendChild(this.backendUrlInput);
    section.appendChild(row);
    panel.appendChild(section);

    return panel;
  }

  protected onConfirm(): void {
    const count = Math.min(5, Math.max(1, parseInt(this.recentCountInput.value) || 3));
    userSettings.updateFontSettings({
      showRecentFonts: this.showRecentCheck.checked,
      recentFontCount: count,
    });
    userSettings.updateAutosaveSettings({
      recoveryEnabled: this.recoveryEnabledCheck.checked,
      recoveryIntervalMinutes: clampInteger(this.recoveryIntervalInput.value, 10, 1, 120),
      idleSaveEnabled: this.idleSaveEnabledCheck.checked,
      idleDelaySeconds: clampInteger(this.idleDelayInput.value, 10, 5, 600),
    });
    userSettings.setShowPdfPrintGuidance(this.pdfPrintGuidanceCheck.checked);
    userSettings.setTemplateValidatorBackendUrl(this.backendUrlInput.value);
    this.eventBus?.emit('autosave-settings-changed', { source: 'options-dialog' });
  }
}

function createAutosaveNumberRow(options: {
  checkbox: HTMLInputElement;
  labelText: string;
  numberInput: HTMLInputElement;
  unitText: string;
}): HTMLElement {
  const row = document.createElement('div');
  row.className = 'dialog-row opt-row opt-autosave-row';

  const label = document.createElement('label');
  label.htmlFor = options.checkbox.id;
  label.textContent = options.labelText;

  const unit = document.createElement('span');
  unit.className = 'opt-count-label';
  unit.textContent = options.unitText;

  row.appendChild(options.checkbox);
  row.appendChild(label);
  row.appendChild(options.numberInput);
  row.appendChild(unit);
  return row;
}

function clampInteger(value: string, fallback: number, min: number, max: number): number {
  const parsed = Number.parseInt(value, 10);
  if (!Number.isFinite(parsed)) return fallback;
  return Math.min(max, Math.max(min, parsed));
}

