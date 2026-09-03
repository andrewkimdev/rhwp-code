/**
 * 템플릿 패널의 "역할 선택" 폼(panel.ts에서 분리) — 라디오 그룹 3개("일반"/
 * "반복 블록"/"메모") + 블록명 입력 + 중첩·바닥고정 체크박스/부모 블록 select,
 * 그리고 패널 안 숫자 단축키(1~8, N, B).
 *
 * 역할 선택은 라디오 그룹 3개("일반"/"반복 블록"/"메모") + 중첩 체크박스 1개 +
 * 바닥고정 체크박스 1개로 구성한다(항목을 한 줄로 나열하던 select 대신) —
 * `-NESTED:` 4종은 실제로는 별도 역할이 아니라 "반복 블록 역할 + 이미 있는
 * `#REPEAT-BODY:<name>` 표 아래 중첩"이라는 수정자이고, `#REPEAT-BODY-BOTTOM:`도
 * 마찬가지로 "REPEAT_BODY 역할 + 페이지 바닥 고정"이라는 위치 수정자이므로,
 * 체크박스로 표현하는 쪽이 실제 마커 문법과 더 정확히 대응하고 전체 어휘도
 * 한눈에 보인다. 두 체크박스는 상호배타다 — 엔진 마커 어휘에 "중첩+바닥고정"을
 * 동시에 표현하는 토큰이 없다. 문서에 `#REPEAT-BODY:<name>` 표가 하나도
 * 없으면(`availableNestedParentBlockNames`, table-outline.ts) 중첩 체크박스
 * 자체를 감춘다(`setNestedAvailable`) — 골라도 만들 수 없는 옵션이라서다.
 * 바닥고정은 이런 의존이 없어(항상 만들 수 있음) REPEAT_BODY 선택 시 무조건 노출한다.
 * v1은 존재 여부만 확인한다 — 정확한 인접성 재검증은 `#6. "Validate now"`의 실제
 * lint가 최종 권위다.
 *
 * 각 역할에는 `hwpx-template-engine/docs/TEMPLATE_MARKER_SYNTAX.md` §3/§3e/§3g 요약을
 * 한 줄로 압축한 설명이 붙는다 — 라디오 `label`의 `title`(hover)과, 선택된 역할이
 * 바뀔 때마다 갱신되는 `roleDescEl`(항상 보이는 캡션) 두 군데에 쓴다.
 *
 * 역할 라디오 1~8과 중첩 체크박스(N)/바닥고정 체크박스(B)에는 숫자/문자 키보드
 * 단축키가 있다 — `#template-panel` 컨테이너의 `keydown`에서 처리하며(전역
 * 단축키가 아니다), 블록명 입력/부모 블록 select에 포커스가 있을 때는 숫자를
 * 그대로 입력해야 하므로 그 두 컨트롤에 포커스가 있으면 무시한다.
 */
import type { WasmBridge } from '@/core/wasm-bridge';
import type { InputHandler } from '@/engine/input-handler';
import type { TemplateTableRole } from '@/core/template-marker';
import { findCellIndexForRowCol } from '@/core/table-outline';

interface GeneralRoleOption {
  value: 'BLOCK' | 'PAGENO' | 'BLOCK_BOTTOM' | 'NOTE';
  label: string;
  description: string;
}

interface RepeatRoleOption {
  value: 'REPEAT_TITLE' | 'REPEAT_HEADER' | 'REPEAT_BODY' | 'REPEAT_FOOTER';
  nestedValue: 'REPEAT_TITLE_NESTED' | 'REPEAT_HEADER_NESTED' | 'REPEAT_BODY_NESTED' | 'REPEAT_FOOTER_NESTED';
  label: string;
  description: string;
}

const GENERAL_ROLES: readonly GeneralRoleOption[] = [
  { value: 'BLOCK', label: '#BLOCK', description: '표의 역할을 문서화만 합니다 — 렌더링에는 영향을 주지 않습니다.' },
  { value: 'PAGENO', label: '#PAGENO', description: '표 전체를 반복 꼬리말로 승격해 모든 페이지에 쪽 번호를 표시합니다. 문서당 최대 1개.' },
  { value: 'BLOCK_BOTTOM', label: '#BLOCK-BOTTOM', description: '#BLOCK과 역할은 같지만, 표가 자연스럽게 도달하는 페이지의 바닥에 붙입니다(예: 서명/직인 표). 남는 공간이 없으면 조용히 원래 위치로 되돌아갑니다.' },
];

const REPEAT_ROLES: readonly RepeatRoleOption[] = [
  { value: 'REPEAT_TITLE', nestedValue: 'REPEAT_TITLE_NESTED', label: '#REPEAT-TITLE:', description: '반복 블록 전체의 제목 표 — 문서에서 한 번만 렌더됩니다. (선택)' },
  { value: 'REPEAT_HEADER', nestedValue: 'REPEAT_HEADER_NESTED', label: '#REPEAT-HEADER:', description: '컬럼 라벨 표 — 블록 맨 앞에 한 번, 항목이 페이지를 넘기면 다음 페이지 첫머리에도 다시 찍힙니다. (선택)' },
  { value: 'REPEAT_BODY', nestedValue: 'REPEAT_BODY_NESTED', label: '#REPEAT-BODY:', description: '항목 1개 분량의 표 — 데이터 개수만큼 복제됩니다. (필수, 블록당 정확히 1개)' },
  { value: 'REPEAT_FOOTER', nestedValue: 'REPEAT_FOOTER_NESTED', label: '#REPEAT-FOOTER:', description: '항목들 뒤에 한 번만 렌더되는 합계 등 요약 표. (선택)' },
];

/** "메모" 그룹 — #NOTE 단독. GENERAL_ROLES와 별개 fieldset이지만 라디오는
 * `name='tp-role'`을 공유하므로(buildRoleGroup) 자동으로 상호배타다. */
const NOTE_ROLES: readonly GeneralRoleOption[] = [
  { value: 'NOTE', label: '#NOTE', description: '이 표 전체(마커 행 포함)를 최종 출력에서 삭제합니다 — authoring 중 한/글에서만 보이는 메모용입니다. 다른 역할과 함께 쓸 수 없습니다.' },
];

/** 역할 값 → 설명 한 줄. 라디오 label의 title(hover)과 상시 캡션(roleDescEl)이 공유한다. */
const ROLE_DESCRIPTIONS: Readonly<Record<string, string>> = Object.fromEntries(
  [...GENERAL_ROLES, ...REPEAT_ROLES, ...NOTE_ROLES].map(r => [r.value, r.description]),
);

const NESTED_TOGGLE_DESCRIPTION =
  '이미 있는 #REPEAT-BODY:<부모블록> 표 바로 뒤에, 그 항목마다 자기만의 하위 반복 목록을 붙입니다(최대 1단계 중첩).';

/** #REPEAT-BODY:만 대상 — TITLE/HEADER/FOOTER엔 위치 동의어가 없다(TEMPLATE_MARKER_SYNTAX.md §3g).
 * 중첩과는 상호배타다 — 엔진 마커 어휘에 "중첩+바닥고정" 조합 토큰이 없다. */
const BOTTOM_ANCHOR_TOGGLE_DESCRIPTION =
  '#REPEAT-BODY:와 역할은 같지만, 블록 전체(있는 TITLE/HEADER/BODY/FOOTER)를 그 블록이 자연스럽게 도달하는 페이지의 바닥에 붙입니다. 중첩 자식 블록과는 함께 쓸 수 없습니다.';

const DEFAULT_ROLE: TemplateTableRole = GENERAL_ROLES[0].value;

export interface RoleFormDeps {
  wasm: WasmBridge;
  getInputHandler: () => InputHandler | null;
  /** 역할·블록명·중첩 등 폼 값이 바뀔 수 있는 모든 지점에서 불린다 — panel이 미리보기/버튼 상태를 갱신한다. */
  onParamsChanged: () => void;
  /** 블록명 입력란에서 Enter — panel이 '태그 지정' 버튼 활성 상태를 검사한 뒤 적용한다. */
  onApplyRequested: () => void;
}

export class RoleForm {
  roleFieldEl!: HTMLDivElement;
  blockNameFieldEl!: HTMLDivElement;
  nestedToggleFieldEl!: HTMLDivElement;
  nestedParentFieldEl!: HTMLDivElement;
  bottomAnchorToggleFieldEl!: HTMLDivElement;

  private roleRadios: HTMLInputElement[] = [];
  private roleDescEl!: HTMLElement;
  private nestedRoleAvailable = false;
  private blockNameInput!: HTMLInputElement;
  /** 사용자가 블록명을 직접 입력했는지 — 자동 제안이 이를 덮어쓰지 않게 막는다. */
  private blockNameManuallyEdited = false;
  private nestedToggle!: HTMLInputElement;
  private nestedParentSelect!: HTMLSelectElement;
  private bottomAnchorToggle!: HTMLInputElement;

  constructor(private deps: RoleFormDeps) {
    this.build();
    this.onRoleChanged();
  }

  renderNestedParentOptions(names: readonly string[]): void {
    const current = this.nestedParentSelect.value;
    this.nestedParentSelect.textContent = '';
    for (const name of names) {
      const opt = document.createElement('option');
      opt.value = name;
      opt.textContent = name;
      this.nestedParentSelect.appendChild(opt);
    }
    if (names.includes(current)) this.nestedParentSelect.value = current;
  }

  /**
   * `#REPEAT-BODY:<name>` 표가 문서에 하나도 없으면 중첩 체크박스 자체를 감추고
   * 해제한다 — 나열 가능한 부모가 없으므로 체크해도 만들 수 없는 옵션이라서다.
   */
  setNestedAvailable(available: boolean): void {
    this.nestedRoleAvailable = available;
    if (!available && this.nestedToggle.checked) {
      this.nestedToggle.checked = false;
    }
    this.onRoleChanged();
  }

  buildParams(): { role: TemplateTableRole; blockName?: string; nestedParent?: string } {
    const repeat = this.selectedRepeatRole();
    const nested = Boolean(repeat) && this.nestedToggle.checked;
    return {
      role: this.resolveSelectedRole(),
      blockName: repeat ? (this.blockNameInput.value.trim() || undefined) : undefined,
      nestedParent: nested ? (this.nestedParentSelect.value || undefined) : undefined,
    };
  }

  private selectedRepeatRole(): RepeatRoleOption | null {
    const checked = this.roleRadios.find(r => r.checked)?.value;
    return REPEAT_ROLES.find(r => r.value === checked) ?? null;
  }

  private resolveSelectedRole(): TemplateTableRole {
    const repeat = this.selectedRepeatRole();
    if (repeat && this.nestedToggle.checked) return repeat.nestedValue;
    if (repeat?.value === 'REPEAT_BODY' && this.bottomAnchorToggle.checked) return 'REPEAT_BODY_BOTTOM';
    return (this.roleRadios.find(r => r.checked)?.value as TemplateTableRole | undefined) ?? DEFAULT_ROLE;
  }

  private onRoleChanged(): void {
    const repeat = this.selectedRepeatRole();
    this.blockNameFieldEl.style.display = repeat ? '' : 'none';
    const nestedTogglable = Boolean(repeat) && this.nestedRoleAvailable;
    this.nestedToggleFieldEl.style.display = nestedTogglable ? '' : 'none';
    if (!nestedTogglable) this.nestedToggle.checked = false;
    this.nestedParentFieldEl.style.display = (nestedTogglable && this.nestedToggle.checked) ? '' : 'none';

    const bottomAnchorTogglable = repeat?.value === 'REPEAT_BODY';
    this.bottomAnchorToggleFieldEl.style.display = bottomAnchorTogglable ? '' : 'none';
    if (!bottomAnchorTogglable) this.bottomAnchorToggle.checked = false;

    const checkedValue = this.roleRadios.find(r => r.checked)?.value;
    this.roleDescEl.textContent = (checkedValue && ROLE_DESCRIPTIONS[checkedValue]) || '';
    const ih = this.deps.getInputHandler();
    this.suggestBlockNameFromCurrentCell(ih, ih?.getCursorPosition());
    this.deps.onParamsChanged();
  }

  /**
   * REPEAT_TITLE 역할일 때, 현재 셀(셀 선택 모드면 선택 범위 좌상단 셀)의
   * 텍스트에서 공백을 모두 제거해 블록명 입력란에 제안한다. 사용자가 이미
   * 손댔거나(blockNameManuallyEdited) 입력란에 값이 있으면 절대 덮어쓰지 않는다.
   * REPEAT_TITLE에만 한정하는 이유: 이 역할만 "셀 하나의 텍스트 = 블록명"이라는
   * 전제가 성립한다(HEADER는 여러 라벨 셀, BODY/FOOTER는 반복되는 데이터 행).
   */
  private suggestBlockNameFromCurrentCell(
    ih: InputHandler | null,
    pos: ReturnType<InputHandler['getCursorPosition']> | undefined,
  ): void {
    if (this.selectedRepeatRole()?.value !== 'REPEAT_TITLE') return;
    if (this.blockNameManuallyEdited) return;
    if (this.blockNameInput.value.trim() !== '') return;
    if (!pos || pos.parentParaIndex === undefined || pos.controlIndex === undefined) return;
    if ((pos.cellPath?.length ?? 0) > 1) return; // 중첩 표는 지원하지 않음

    const sec = pos.sectionIndex;
    const ppi = pos.parentParaIndex;
    const ci = pos.controlIndex;
    try {
      const range = ih?.isInCellSelectionMode() ? ih.getSelectedCellRange() : null;
      const cellIdx = range
        ? findCellIndexForRowCol(this.deps.wasm, sec, ppi, ci, range.startRow, range.startCol)
        : (pos.cellIndex ?? null);
      if (cellIdx === null) return;

      const len = this.deps.wasm.getCellParagraphLength(sec, ppi, ci, cellIdx, 0);
      if (len <= 0) return;
      const text = this.deps.wasm.getTextInCell(sec, ppi, ci, cellIdx, 0, 0, len);
      const stripped = text.replace(/\s+/g, ''); // trim보다 강함 — 텍스트 내부 공백까지 전부 제거
      if (!stripped) return;

      this.blockNameInput.value = stripped;
      this.deps.onParamsChanged();
    } catch {
      // 조회 실패는 조용히 무시 — 제안은 optional UX이지 필수 경로가 아니다
    }
  }

  /**
   * 패널 안(블록명 입력/부모 블록 select 제외)에서 숫자 1~7 → 해당 역할 라디오,
   * `n` → 중첩 체크박스(보일 때만) 토글. 조합키가 있으면 무시해 브라우저/전역
   * 단축키와 겹치지 않게 한다. 전역 리스너가 아니라 `#template-panel`의
   * keydown이라 포커스가 패널 밖(에디터 캔버스 등)에 있을 때는 애초에 안 불린다.
   */
  handleShortcutKey(e: KeyboardEvent): void {
    if (e.ctrlKey || e.metaKey || e.altKey) return;
    if (e.target === this.blockNameInput || e.target === this.nestedParentSelect) return;

    const digit = Number(e.key);
    if (Number.isInteger(digit) && digit >= 1 && digit <= this.roleRadios.length) {
      e.preventDefault();
      const radio = this.roleRadios[digit - 1];
      radio.checked = true;
      radio.focus();
      this.onRoleChanged();
      return;
    }
    if (e.key.toLowerCase() === 'n' && this.nestedToggleFieldEl.style.display !== 'none') {
      e.preventDefault();
      this.nestedToggle.checked = !this.nestedToggle.checked;
      this.nestedToggle.focus();
      this.onRoleChanged();
      return;
    }
    if (e.key.toLowerCase() === 'b' && this.bottomAnchorToggleFieldEl.style.display !== 'none') {
      e.preventDefault();
      this.bottomAnchorToggle.checked = !this.bottomAnchorToggle.checked;
      this.bottomAnchorToggle.focus();
      this.onRoleChanged();
    }
  }

  private buildRoleGroup(
    legendText: string,
    roles: readonly { value: string; label: string; description: string }[],
  ): HTMLFieldSetElement {
    const fieldset = document.createElement('fieldset');
    fieldset.className = 'tp-role-group';
    const legend = document.createElement('legend');
    legend.className = 'tp-role-group-legend';
    legend.textContent = legendText;
    fieldset.appendChild(legend);
    for (const role of roles) {
      const label = document.createElement('label');
      label.className = 'tp-radio-label';
      label.title = role.description;
      const input = document.createElement('input');
      input.type = 'radio';
      input.name = 'tp-role';
      input.value = role.value;
      input.checked = role.value === DEFAULT_ROLE;
      input.addEventListener('change', () => {
        this.blockNameManuallyEdited = false;
        this.onRoleChanged();
      });
      // 단축키 배지(1~7)는 buildRoleGroup이 두 그룹에 걸쳐 순서대로 불리므로
      // roleRadios.length(=push 전 현재 길이)가 곧 0-based 순번이다 —
      // 실제 키 처리는 이 순번 그대로 this.roleRadios를 인덱싱한다(handleShortcutKey).
      const keyBadge = document.createElement('span');
      keyBadge.className = 'tp-role-key';
      keyBadge.textContent = String(this.roleRadios.length + 1);
      this.roleRadios.push(input);
      label.appendChild(input);
      label.appendChild(document.createTextNode(role.label));
      label.appendChild(keyBadge);
      fieldset.appendChild(label);
    }
    return fieldset;
  }

  private build(): void {
    // 역할 선택 — 라디오 그룹 2개("일반"/"반복 블록")
    this.roleFieldEl = document.createElement('div');
    this.roleFieldEl.className = 'tp-field';
    const roleLabel = document.createElement('span');
    roleLabel.className = 'tp-label';
    roleLabel.textContent = '역할';
    this.roleFieldEl.appendChild(roleLabel);
    this.roleFieldEl.appendChild(this.buildRoleGroup('일반', GENERAL_ROLES));
    this.roleFieldEl.appendChild(this.buildRoleGroup('반복 블록', REPEAT_ROLES));
    this.roleFieldEl.appendChild(this.buildRoleGroup('메모', NOTE_ROLES));
    this.roleDescEl = document.createElement('div');
    this.roleDescEl.className = 'tp-role-desc';
    this.roleFieldEl.appendChild(this.roleDescEl);

    // 블록명
    this.blockNameFieldEl = document.createElement('div');
    this.blockNameFieldEl.className = 'tp-field';
    const blockNameLabel = document.createElement('label');
    blockNameLabel.className = 'tp-label';
    blockNameLabel.textContent = '블록명';
    this.blockNameInput = document.createElement('input');
    this.blockNameInput.type = 'text';
    this.blockNameInput.className = 'tp-input';
    this.blockNameInput.placeholder = '예: 품목내역';
    this.blockNameInput.addEventListener('input', () => {
      this.blockNameManuallyEdited = true;
      this.deps.onParamsChanged();
    });
    this.blockNameInput.addEventListener('keydown', (e) => {
      if (e.key !== 'Enter' || e.isComposing) return;
      e.preventDefault();
      this.deps.onApplyRequested();
    });
    this.blockNameFieldEl.appendChild(blockNameLabel);
    this.blockNameFieldEl.appendChild(this.blockNameInput);

    // 중첩 여부 체크박스 — REPEAT_* 역할에서만, 유효한 부모가 있을 때만 노출
    this.nestedToggleFieldEl = document.createElement('div');
    this.nestedToggleFieldEl.className = 'tp-field';
    const nestedToggleLabel = document.createElement('label');
    nestedToggleLabel.className = 'tp-checkbox-label';
    nestedToggleLabel.title = NESTED_TOGGLE_DESCRIPTION;
    this.nestedToggle = document.createElement('input');
    this.nestedToggle.type = 'checkbox';
    this.nestedToggle.className = 'tp-checkbox';
    this.nestedToggle.addEventListener('change', () => {
      // 중첩과 바닥 고정은 상호배타다 — 엔진 마커 어휘에 두 위치를 동시에 표현하는
      // 토큰이 없다(REPEAT_BODY_NESTED_PREFIX와 REPEAT_BODY_BOTTOM_PREFIX는 별개 상수).
      if (this.nestedToggle.checked) this.bottomAnchorToggle.checked = false;
      this.onRoleChanged();
    });
    const nestedKeyBadge = document.createElement('span');
    nestedKeyBadge.className = 'tp-role-key';
    nestedKeyBadge.textContent = 'N';
    nestedToggleLabel.appendChild(this.nestedToggle);
    nestedToggleLabel.appendChild(document.createTextNode('중첩 자식 블록으로 지정'));
    nestedToggleLabel.appendChild(nestedKeyBadge);
    this.nestedToggleFieldEl.appendChild(nestedToggleLabel);

    // 바닥 고정 체크박스 — REPEAT_BODY 역할에서만 노출(onRoleChanged). 중첩과 달리
    // "기존 부모 표 존재" 의존이 없으므로 nestedRoleAvailable 게이트는 쓰지 않는다.
    this.bottomAnchorToggleFieldEl = document.createElement('div');
    this.bottomAnchorToggleFieldEl.className = 'tp-field';
    const bottomAnchorLabel = document.createElement('label');
    bottomAnchorLabel.className = 'tp-checkbox-label';
    bottomAnchorLabel.title = BOTTOM_ANCHOR_TOGGLE_DESCRIPTION;
    this.bottomAnchorToggle = document.createElement('input');
    this.bottomAnchorToggle.type = 'checkbox';
    this.bottomAnchorToggle.className = 'tp-checkbox';
    this.bottomAnchorToggle.addEventListener('change', () => {
      if (this.bottomAnchorToggle.checked) this.nestedToggle.checked = false;
      this.onRoleChanged();
    });
    const bottomAnchorKeyBadge = document.createElement('span');
    bottomAnchorKeyBadge.className = 'tp-role-key';
    bottomAnchorKeyBadge.textContent = 'B';
    bottomAnchorLabel.appendChild(this.bottomAnchorToggle);
    bottomAnchorLabel.appendChild(document.createTextNode('바닥에 고정(#REPEAT-BODY-BOTTOM:)'));
    bottomAnchorLabel.appendChild(bottomAnchorKeyBadge);
    this.bottomAnchorToggleFieldEl.appendChild(bottomAnchorLabel);

    // 중첩 부모 블록명
    this.nestedParentFieldEl = document.createElement('div');
    this.nestedParentFieldEl.className = 'tp-field';
    const nestedParentLabel = document.createElement('label');
    nestedParentLabel.className = 'tp-label';
    nestedParentLabel.textContent = '부모 블록 (중첩)';
    this.nestedParentSelect = document.createElement('select');
    this.nestedParentSelect.className = 'tp-select';
    this.nestedParentSelect.addEventListener('change', () => this.deps.onParamsChanged());
    this.nestedParentFieldEl.appendChild(nestedParentLabel);
    this.nestedParentFieldEl.appendChild(this.nestedParentSelect);
  }
}
