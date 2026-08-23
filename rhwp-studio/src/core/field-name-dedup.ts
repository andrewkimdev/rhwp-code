/**
 * 누름틀 이름 제안 후보의 이름 충돌 해소 — `field-name-suggest.ts`(표 인접 셀
 * 기반)와 `selection-text.ts`(선택 텍스트 기반) 두 후보 소스가 공유하는 단일
 * 출처. 문서 내 기존 필드명(existingNames) 및 같은 배치에서 먼저 배정된 이름
 * (usedInBatch)과 겹치면 `_2`, `_3`, ... 접미어를 붙여 겹치지 않는 이름을 만든다.
 */
export function resolveUniqueName(
  baseName: string,
  existingNames: ReadonlySet<string>,
  usedInBatch: ReadonlySet<string>,
): string {
  let name = baseName;
  let suffix = 2;
  while (existingNames.has(name) || usedInBatch.has(name)) {
    name = `${baseName}_${suffix}`;
    suffix++;
  }
  return name;
}
