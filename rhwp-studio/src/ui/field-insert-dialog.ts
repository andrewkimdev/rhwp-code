/**
 * 누름틀 삽입 대화상자
 *
 * 현재 커서 위치에 ClickHere 필드를 만들기 위한 안내문, 메모, 필드 이름,
 * 양식 모드 편집 가능 여부를 입력한다.
 */
import { ModalDialog } from './dialog';
import {
  MAX_FIELD_GUIDE_LEN,
  MAX_FIELD_MEMO_LEN,
  MAX_FIELD_NAME_LEN,
  type ClickHereProps,
} from './field-edit-dialog';
import { checkNameCollision, wireTitleNameSync } from './field-name-sync';

export class FieldInsertDialog extends ModalDialog {
  private guideInput!: HTMLInputElement;
  private memoInput!: HTMLTextAreaElement;
  private nameInput!: HTMLInputElement;
  private editableCheckbox!: HTMLInputElement;
  private titleNameSyncCheckbox!: HTMLInputElement;
  private nameErrorLabel!: HTMLDivElement;
  private guideErrorLabel!: HTMLDivElement;
  private memoErrorLabel!: HTMLDivElement;
  private nameCollisionLabel!: HTMLDivElement;
  private confirmBtn!: HTMLButtonElement;
  private existingNames: ReadonlySet<string> = new Set();

  onApply: ((props: ClickHereProps) => void) | null = null;

  constructor() {
    super('필드 입력', 420, false);
  }

  protected createBody(): HTMLElement {
    const body = document.createElement('div');
    body.className = 'field-edit-body';

    const tabBar = document.createElement('div');
    tabBar.className = 'dialog-tabs';
    const tab = document.createElement('button');
    tab.className = 'dialog-tab active';
    tab.textContent = '누름틀';
    tab.type = 'button';
    tabBar.appendChild(tab);
    body.appendChild(tabBar);

    const panel = document.createElement('div');
    panel.className = 'field-edit-panel';

    const guideLabel = document.createElement('label');
    guideLabel.className = 'field-edit-label';
    guideLabel.textContent = '입력할 내용의 안내문(P):';
    panel.appendChild(guideLabel);

    this.guideInput = document.createElement('input');
    this.guideInput.type = 'text';
    this.guideInput.className = 'field-edit-input';
    this.guideInput.value = '입력하세요';
    this.guideInput.maxLength = MAX_FIELD_GUIDE_LEN;
    panel.appendChild(this.guideInput);

    this.guideErrorLabel = document.createElement('div');
    this.guideErrorLabel.className = 'field-edit-error';
    this.guideErrorLabel.style.color = '#c00';
    this.guideErrorLabel.style.fontSize = '11px';
    this.guideErrorLabel.style.display = 'none';
    this.guideErrorLabel.textContent = `안내문은 ${MAX_FIELD_GUIDE_LEN}자를 넘을 수 없습니다.`;
    panel.appendChild(this.guideErrorLabel);

    const memoLabel = document.createElement('label');
    memoLabel.className = 'field-edit-label';
    memoLabel.textContent = '메모 내용(M):';
    panel.appendChild(memoLabel);

    this.memoInput = document.createElement('textarea');
    this.memoInput.className = 'field-edit-textarea';
    this.memoInput.rows = 4;
    this.memoInput.maxLength = MAX_FIELD_MEMO_LEN;
    panel.appendChild(this.memoInput);

    this.memoErrorLabel = document.createElement('div');
    this.memoErrorLabel.className = 'field-edit-error';
    this.memoErrorLabel.style.color = '#c00';
    this.memoErrorLabel.style.fontSize = '11px';
    this.memoErrorLabel.style.display = 'none';
    this.memoErrorLabel.textContent = `메모 내용은 ${MAX_FIELD_MEMO_LEN}자를 넘을 수 없습니다.`;
    panel.appendChild(this.memoErrorLabel);

    const nameLabel = document.createElement('label');
    nameLabel.className = 'field-edit-label';
    nameLabel.textContent = '필드 이름(N):';
    panel.appendChild(nameLabel);

    this.nameInput = document.createElement('input');
    this.nameInput.type = 'text';
    this.nameInput.className = 'field-edit-input';
    this.nameInput.maxLength = MAX_FIELD_NAME_LEN;
    panel.appendChild(this.nameInput);

    this.nameErrorLabel = document.createElement('div');
    this.nameErrorLabel.className = 'field-edit-error';
    this.nameErrorLabel.style.color = '#c00';
    this.nameErrorLabel.style.fontSize = '11px';
    this.nameErrorLabel.style.display = 'none';
    this.nameErrorLabel.textContent = `필드 이름은 ${MAX_FIELD_NAME_LEN}자를 넘을 수 없습니다.`;
    panel.appendChild(this.nameErrorLabel);

    const editableRow = document.createElement('label');
    editableRow.className = 'field-edit-checkbox-row';
    this.editableCheckbox = document.createElement('input');
    this.editableCheckbox.type = 'checkbox';
    this.editableCheckbox.checked = true;
    editableRow.appendChild(this.editableCheckbox);
    editableRow.appendChild(document.createTextNode(' 양식 모드에서 편집 가능(F)'));
    panel.appendChild(editableRow);

    const syncRow = document.createElement('label');
    syncRow.className = 'field-edit-checkbox-row';
    this.titleNameSyncCheckbox = document.createElement('input');
    this.titleNameSyncCheckbox.type = 'checkbox';
    this.titleNameSyncCheckbox.checked = true;
    syncRow.appendChild(this.titleNameSyncCheckbox);
    syncRow.appendChild(document.createTextNode(' 제목과 이름 일치'));
    panel.appendChild(syncRow);

    this.nameCollisionLabel = document.createElement('div');
    this.nameCollisionLabel.className = 'field-edit-error';
    this.nameCollisionLabel.style.color = '#c00';
    this.nameCollisionLabel.style.fontSize = '11px';
    this.nameCollisionLabel.style.display = 'none';
    this.nameCollisionLabel.textContent = '동일한 이름의 필드가 있습니다.';
    panel.appendChild(this.nameCollisionLabel);

    wireTitleNameSync(this.guideInput, this.nameInput, this.titleNameSyncCheckbox, () => this.checkCollision());

    body.appendChild(panel);
    return body;
  }

  private checkCollision(): void {
    const collision = checkNameCollision(this.nameInput.value, this.existingNames);
    this.nameCollisionLabel.style.display = collision ? '' : 'none';
    this.confirmBtn.disabled = collision;
  }

  protected onConfirm(): void | boolean {
    if (this.nameInput.value.length > MAX_FIELD_NAME_LEN) {
      this.nameErrorLabel.style.display = '';
      this.nameInput.focus();
      return false;
    }
    this.nameErrorLabel.style.display = 'none';

    let valid = true;
    if (this.guideInput.value.length > MAX_FIELD_GUIDE_LEN) {
      this.guideErrorLabel.style.display = '';
      valid = false;
    } else {
      this.guideErrorLabel.style.display = 'none';
    }
    if (this.memoInput.value.length > MAX_FIELD_MEMO_LEN) {
      this.memoErrorLabel.style.display = '';
      valid = false;
    } else {
      this.memoErrorLabel.style.display = 'none';
    }
    if (!valid) {
      return false;
    }

    this.onApply?.({
      guide: this.guideInput.value,
      memo: this.memoInput.value,
      name: this.nameInput.value,
      editable: this.editableCheckbox.checked,
    });
  }

  /** 대화상자를 열고 문서 내 기존 필드 이름 목록(충돌 검사용)을 설정한다 */
  showWith(existingNames: ReadonlySet<string>): void {
    this.existingNames = existingNames;
    this.show();
    this.checkCollision();
  }

  override show(): void {
    super.show();

    const footer = this.dialog.querySelector('.dialog-footer');
    if (footer) {
      const buttons = footer.querySelectorAll('button');
      if (buttons[0]) {
        buttons[0].textContent = '넣기(D)';
        this.confirmBtn = buttons[0] as HTMLButtonElement;
      }
      if (buttons[1]) buttons[1].textContent = '취소';
    }

    // 싱글턴으로 재사용되므로 이전 삽입 값이 남지 않도록 초기 상태로 리셋한다.
    this.guideInput.value = '입력하세요';
    this.memoInput.value = '';
    this.nameInput.value = '';
    this.editableCheckbox.checked = true;
    this.titleNameSyncCheckbox.checked = true;

    this.guideInput.focus();
    this.guideInput.select();
  }
}
