/**
 * 누름틀 다이얼로그(field-edit-dialog.ts, field-insert-dialog.ts) 공용 —
 * "제목과 이름 일치" 양방향 미러링과 필드 이름 충돌 판정.
 */

/**
 * 안내문 입력과 필드 이름 입력을 `syncCheckbox`가 체크된 동안 양방향으로 미러링한다.
 *
 * `syncing` 가드: `.value =` 대입은 네이티브 `input` 이벤트를 재발생시키지 않으므로
 * 실제로는 무한루프가 될 수 없지만, 이후 누군가 `dispatchEvent`로 바꾸거나 테스트에서
 * 수동으로 `input`을 dispatch할 경우를 대비한 저비용 방어다.
 */
export function wireTitleNameSync(
  guideInput: HTMLInputElement,
  nameInput: HTMLInputElement,
  syncCheckbox: HTMLInputElement,
  onEffectiveNameChanged: () => void,
): void {
  let syncing = false;

  guideInput.addEventListener('input', () => {
    if (syncCheckbox.checked && !syncing) {
      syncing = true;
      nameInput.value = guideInput.value;
      syncing = false;
    }
    onEffectiveNameChanged();
  });

  nameInput.addEventListener('input', () => {
    if (syncCheckbox.checked && !syncing) {
      syncing = true;
      guideInput.value = nameInput.value;
      syncing = false;
    }
    onEffectiveNameChanged();
  });
}

/**
 * 같은 이름의 누름틀이 문서에 여러 번 나오는 것은 이 저장소의 서식 채우기 도구가
 * 정상 취급하는 반복 필드 패턴이다(`이름[0]`/`이름[1]` 지목, form_filling_guide.md §1-1).
 * 그럼에도 이 다이얼로그는 제품 결정으로 신규 충돌을 하드 블록한다 — 버그로 오인해
 * 이 검사를 제거하지 말 것.
 */
export function checkNameCollision(name: string, existingNames: ReadonlySet<string>): boolean {
  return name !== '' && existingNames.has(name);
}
