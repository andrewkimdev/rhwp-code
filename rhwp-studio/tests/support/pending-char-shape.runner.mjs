// [#3080] 글자 서식 행위 러너 — command.ts 의 실제 커맨드 클래스를 로드해 **실제 wasm 문서**에
// 대고 검증한다. cursor.ts/command.ts 의 TS 파라미터 프로퍼티 때문에 기본 test 러너(strip-only)는
// 엔진 클래스를 import 하지 못하므로, 부모 테스트가 --experimental-transform-types 로 spawn 한다
// (tests/undo-drag-command-behaviour.test.ts 와 동일한 방식).
//
// 검증 대상:
//  1) 선택 범위 굵게/색이 문서에 닿는가 + undo 로 되돌아가는가  (신고 ①②)
//  2) 캐럿 대기 서식(pending char shape)이 **다음 삽입 런에만** 적용되는가 (신고 ③④, "굵게 켜고 입력")
//  3) 예약 서식이 다른 삽입은 병합되지 않는가 (앞 글자에 뒤 서식이 덮이는 것 방지)
import { registerHooks } from 'node:module';
import { pathToFileURL, fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';
import { readFileSync } from 'node:fs';
import assert from 'node:assert/strict';

const studioDir = join(dirname(fileURLToPath(import.meta.url)), '..', '..');
const srcDir = join(studioDir, 'src');
const repoDir = join(studioDir, '..');

// @/ 별칭 + 확장자 없는 상대 import 를 .ts 로 해석(tsconfig paths 재현).
registerHooks({
  resolve(specifier, context, nextResolve) {
    if (specifier.startsWith('@/')) {
      const abs = join(srcDir, specifier.slice(2));
      const withTs = abs.endsWith('.ts') ? abs : abs + '.ts';
      return { url: pathToFileURL(withTs).href, shortCircuit: true };
    }
    if ((specifier.startsWith('./') || specifier.startsWith('../')) && !/\.[cm]?[tj]s$/.test(specifier)) {
      const parent = context.parentURL ? dirname(fileURLToPath(context.parentURL)) : srcDir;
      return { url: pathToFileURL(join(parent, specifier + '.ts')).href, shortCircuit: true };
    }
    return nextResolve(specifier, context);
  },
});

const { ApplyCharFormatCommand, InsertTextCommand, applyCharShapeModsToRange } =
  await import(pathToFileURL(join(srcDir, 'engine', 'command.ts')).href);

const { HwpDocument } = await import(pathToFileURL(join(repoDir, 'pkg-node', 'rhwp.js')).href);

const doc = new HwpDocument(new Uint8Array(readFileSync(join(repoDir, 'samples', '2010-01-06.hwp'))));

/** 커맨드가 실제로 쓰는 WasmBridge 표면만 구현한 얇은 어댑터. */
const wasm = {
  insertText: (sec, para, off, text) => doc.insertText(sec, para, off, text),
  deleteText: (sec, para, off, count) => doc.deleteText(sec, para, off, count),
  replaceBodyTextLocal: (sec, para, off, deleteCount, text) => {
    if (deleteCount > 0) doc.deleteText(sec, para, off, deleteCount);
    if (text) doc.insertText(sec, para, off, text);
    return { documentPaginationPending: false, flowChanged: false, paginationCompleted: true };
  },
  applyCharFormat: (sec, para, from, to, json) => doc.applyCharFormat(sec, para, from, to, json),
  setCharShapeId: (sec, para, from, to, id) => doc.setCharShapeId(sec, para, from, to, id),
  getCharPropertiesAt: (sec, para, off) => JSON.parse(doc.getCharPropertiesAt(sec, para, off)),
  getParagraphLength: (sec, para) => doc.getParagraphLength(sec, para),
  getTextRange: (sec, para, off, count) => doc.getTextRange(sec, para, off, count),
};

const pos = (para, charOffset) => ({ sectionIndex: 0, paragraphIndex: para, charOffset });
const props = (para, off) => JSON.parse(doc.getCharPropertiesAt(0, para, off));

// 10글자 이상인 첫 본문 문단을 대상으로 한다.
let para = -1;
for (let p = 0; p < 60; p++) {
  if (doc.getParagraphLength(0, p) >= 10) { para = p; break; }
}
assert.ok(para >= 0, '10글자 이상인 본문 문단이 있어야 한다');

// ── 1. 선택 범위 굵게/색 → 문서 반영 + undo ────────────────────────────────
{
  assert.equal(props(para, 0).bold, false, '전제: 문단 앞부분은 굵지 않다');

  const cmd = new ApplyCharFormatCommand(pos(para, 0), pos(para, 5), { bold: true, textColor: '#ff0000' });
  cmd.execute(wasm);

  for (const off of [0, 2, 4]) {
    assert.equal(props(para, off).bold, true, `offset ${off} 은 굵어야 한다`);
    assert.equal(props(para, off).textColor, '#ff0000', `offset ${off} 은 빨강이어야 한다`);
  }
  assert.equal(props(para, 6).bold, false, '선택 밖 글자는 그대로여야 한다');

  cmd.undo(wasm);
  assert.equal(props(para, 0).bold, false, 'undo 는 원래 글자 모양으로 되돌린다');
  assert.equal(props(para, 0).textColor, '#000000', 'undo 는 색도 되돌린다');
}

// ── 2. 캐럿 대기 서식은 다음 삽입 런에만 적용된다 ───────────────────────────
{
  const at = 3;
  const cmd = new InsertTextCommand(pos(para, at), 'ABC', undefined, { bold: true, textColor: '#0000ff' });
  const after = cmd.execute(wasm);

  assert.equal(doc.getTextRange(0, para, at, 3), 'ABC', '텍스트가 삽입돼야 한다');
  assert.equal(after.charOffset, at + 3, '캐럿은 삽입 길이만큼 전진한다');
  for (let i = 0; i < 3; i++) {
    assert.equal(props(para, at + i).bold, true, `삽입된 ${i}번째 글자에 예약 서식(굵게)이 적용돼야 한다`);
    assert.equal(props(para, at + i).textColor, '#0000ff', `삽입된 ${i}번째 글자에 예약 색이 적용돼야 한다`);
  }
  assert.equal(props(para, at + 3).bold, false, '삽입 범위 뒤 글자는 영향을 받지 않는다');
  assert.equal(props(para, 0).bold, false, '삽입 범위 앞 글자는 영향을 받지 않는다');

  cmd.undo(wasm);
  assert.notEqual(doc.getTextRange(0, para, at, 3), 'ABC', 'undo 는 삽입 텍스트를 지운다');
}

// ── 3. 예약 서식이 다른 삽입끼리 병합 금지 ─────────────────────────────────
{
  const plain = new InsertTextCommand(pos(0, 0), 'a', 1000);
  const bold = new InsertTextCommand(pos(0, 1), 'b', 1010, { bold: true });
  const bold2 = new InsertTextCommand(pos(0, 2), 'c', 1020, { bold: true });

  assert.equal(plain.mergeWith(bold), null, '서식 없는 입력과 굵은 입력은 병합 불가');
  const merged = bold.mergeWith(bold2);
  assert.ok(merged, '같은 예약 서식끼리는 기존대로 병합된다');
  assert.equal(merged.getCharFormat().bold, true, '병합 결과도 예약 서식을 유지한다');
}

// ── 4. 빈 범위(캐럿)는 문서에 쓰지 않는다 ──────────────────────────────────
{
  const calls = [];
  const spy = {
    applyCharFormat: (...args) => { calls.push(args); return '{}'; },
    applyCharFormatInCellByPath: (...args) => { calls.push(args); return '{}'; },
  };
  applyCharShapeModsToRange(spy, pos(0, 4), 4, 4, { bold: true });
  assert.equal(calls.length, 0, '빈 범위(캐럿)는 적용 대상이 없으므로 호출하지 않는다');
  applyCharShapeModsToRange(spy, pos(0, 4), 4, 7, { bold: true });
  assert.equal(calls.length, 1, '실제 범위는 본문 applyCharFormat 으로 간다');
  assert.match(JSON.stringify(calls[0]), /\\"bold\\":true/);
}

console.log('PENDING_CHAR_SHAPE_OK');
