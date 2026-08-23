import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

// 꼬리말 쪽번호/총쪽수/파일이름 필드 삽입 자동 진입 + 끝 위치 보정 소스 가드.
//
// 세 커맨드는 머리말/꼬리말 편집 모드 밖에서는 insertHfField() 의
// `!cursor.isInHeaderFooter()` 가드에 막혀 조용히 no-op 이었다 — 숨겨진 꼬리말
// 도구모음으로 먼저 수동 진입해야만 동작했다. 이제 편집 모드가 아니면 자동으로
// 꼬리말 모드에 진입(항상 꼬리말 — 머리말은 기존 수동 흐름 유지)하고, 기존
// 내용 뒤에 이어 쓰도록 커서를 끝으로 옮긴다.
//
// enterHeaderFooterMode() 는 항상 (0,0) 으로 커서를 초기화하므로, positionAtEnd
// 옵션은 그 이후에 별도로 위치를 보정해야 한다 — 재진입/리셋이 아니라 커서
// 이동뿐이어야 한다(그렇지 않으면 두 번째 필드 삽입 시 첫 필드가 지워지고
// 처음부터 다시 쓰여진다).

const rootDir = dirname(dirname(fileURLToPath(import.meta.url)));
const pageSrc = readFileSync(join(rootDir, 'src/command/commands/page.ts'), 'utf8');
const keyboardSrc = readFileSync(join(rootDir, 'src/engine/input-handler-keyboard.ts'), 'utf8');

function slice(s: string, from: string, to: string): string {
  const a = s.indexOf(from);
  assert.notEqual(a, -1, `${from} not found`);
  const b = s.indexOf(to, a + from.length);
  return b === -1 ? s.slice(a) : s.slice(a, b);
}

for (const [id, fieldType] of [
  ['page:insert-field-pagenum', 1],
  ['page:insert-field-totalpage', 2],
  ['page:insert-field-filename', 3],
] as const) {
  test(`${id} 는 HF 모드 밖이면 자동으로 꼬리말 모드에 끝 위치로 진입한 뒤 필드를 삽입한다`, () => {
    const block = slice(pageSrc, `id: '${id}',`, "\n  {");
    const guardIdx = block.search(/!cursor\.isInHeaderFooter\(\)/);
    assert.notEqual(guardIdx, -1, `${id}: !cursor.isInHeaderFooter() 가드 없음`);

    const enterIdx = block.indexOf('enterHeaderFooterEditing(services, false, { positionAtEnd: true });');
    assert.notEqual(enterIdx, -1, `${id}: 자동 진입(끝 위치 보정 포함) 호출 없음`);
    assert.ok(enterIdx > guardIdx, `${id}: 자동 진입은 !isInHeaderFooter() 가드 안쪽이어야 한다`);

    const insertIdx = block.indexOf(`insertHfField(services, ${fieldType});`);
    assert.notEqual(insertIdx, -1, `${id}: insertHfField(services, ${fieldType}) 호출 없음`);
    assert.ok(insertIdx > enterIdx, `${id}: 자동 진입 뒤에 insertHfField 를 호출해야 한다`);

    // 항상 꼬리말로 진입한다 — 머리말 변형은 만들지 않는다(스코프 결정).
    assert.match(block, /enterHeaderFooterEditing\(services, false, \{ positionAtEnd: true \}\)/, '항상 꼬리말(isHeader=false)');
  });
}

test('insertHfField 자체는 건드리지 않는다 — isInHeaderFooter() 가드를 그대로 유지', () => {
  const block = slice(pageSrc, 'function insertHfField(', 'function navigateHeaderFooter(');
  assert.match(block, /if \(!cursor \|\| !cursor\.isInHeaderFooter\(\)\) return;/, 'insertHfField 는 여전히 no-op 가드를 유지한다');
});

test('positionHfCursorAtEnd 는 마지막 문단 끝으로 커서를 옮길 뿐, 모드를 재설정하지 않는다', () => {
  const block = slice(pageSrc, 'function positionHfCursorAtEnd(', 'function applyHfTemplate(');
  assert.match(block, /wasm\.getHeaderFooterParaInfo\(/, '문단 정보 조회');
  assert.match(block, /cursor\.setHfCursorPosition\(/, '커서 위치만 갱신');
  assert.doesNotMatch(block, /cursor\.enterHeaderFooterMode\(/, '재진입/리셋 금지 — 위치 보정만 해야 한다');
});

test('enterHeaderFooterEditing 은 positionAtEnd 옵션이 있을 때만 끝 위치 보정을 호출한다', () => {
  const block = slice(pageSrc, 'function enterHeaderFooterEditing(', 'function insertHfField(');
  assert.match(block, /options\?:\s*\{\s*positionAtEnd\?:\s*boolean\s*\}/, 'positionAtEnd 옵션 파라미터');
  const modeIdx = block.indexOf('cursor.enterHeaderFooterMode(isHeader, target.sectionIndex, target.applyTo, currentPage);');
  const posIdx = block.indexOf('positionHfCursorAtEnd(services, cursor, isHeader, target.sectionIndex, target.applyTo);');
  const emitIdx = block.indexOf("services.eventBus.emit('headerFooterModeChanged'");
  assert.notEqual(modeIdx, -1);
  assert.notEqual(posIdx, -1);
  assert.notEqual(emitIdx, -1);
  assert.ok(modeIdx < posIdx, 'enterHeaderFooterMode 다음에 위치 보정을 호출해야 한다(그래야 (0,0) 리셋 뒤에 보정됨)');
  assert.ok(posIdx < emitIdx, '위치 보정은 headerFooterModeChanged emit 이전에 끝나야 한다');
});

test('HF 모드 키보드 처리 블록의 Home/End 는 setHfCursorPosition 을 호출하고, 블록의 종결 return 이전에 위치한다', () => {
  const block = slice(keyboardSrc, 'if (this.cursor.isInHeaderFooter()) {', '// ─── 각주 편집 모드 키보드 처리');

  const homeEndIdx = block.search(/e\.key === 'Home' \|\| e\.key === 'End'/);
  assert.notEqual(homeEndIdx, -1, "HF 블록에 Home/End 분기 없음");

  const setPosIdx = block.indexOf('this.cursor.setHfCursorPosition(', homeEndIdx);
  assert.notEqual(setPosIdx, -1, 'Home/End 분기는 setHfCursorPosition 을 호출해야 한다');

  // 블록의 마지막 무조건 종결 return — "기타 키(문자 입력)" 주석 뒤에 오는 return.
  const terminalReturnIdx = block.indexOf('return;', block.indexOf('기타 키'));
  assert.notEqual(terminalReturnIdx, -1, '블록 종결 return 을 찾지 못함');

  assert.ok(homeEndIdx < terminalReturnIdx, 'Home/End 분기는 블록의 무조건 종결 return 이전에 있어야 도달 가능하다');
  assert.ok(setPosIdx < terminalReturnIdx, 'setHfCursorPosition 호출도 종결 return 이전에 있어야 한다');

  // 방향키(ArrowLeft/Right) 처리 다음에 와야 한다는 스펙 순서도 함께 확인.
  const arrowIdx = block.search(/e\.key === 'ArrowLeft' \|\| e\.key === 'ArrowRight'/);
  assert.notEqual(arrowIdx, -1);
  assert.ok(arrowIdx < homeEndIdx, 'Home/End 분기는 ArrowLeft/Right 처리 다음에 온다');
});

test('HF 모드 Home/End 는 문단 내 이동만 한다 — Ctrl/Shift 확장이나 문서 전체 이동을 추가하지 않는다', () => {
  const block = slice(keyboardSrc, 'if (this.cursor.isInHeaderFooter()) {', '// ─── 각주 편집 모드 키보드 처리');
  const start = block.search(/e\.key === 'Home' \|\| e\.key === 'End'/);
  const end = block.indexOf('return;', start);
  const homeEndBlock = block.slice(start, end);
  assert.doesNotMatch(homeEndBlock, /ctrlKey/, 'Ctrl+Home/End 문서 전체 이동은 스코프 밖');
  assert.doesNotMatch(homeEndBlock, /shiftKey/, 'Shift+Home/End 선택 확장은 스코프 밖(HF 모드는 선택 미지원)');
});
