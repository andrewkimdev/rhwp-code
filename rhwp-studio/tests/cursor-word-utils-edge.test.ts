import test from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

// [2차 god-file 분할 신뢰 보강] cursor-word-utils.ts(P8a 로 분리된 순수 함수 무리)의
// 엣지 케이스 행동 가드. 기존 커서 커버리지는 원문 가드·선택 모드 행동 테스트뿐이라
// 단어/문장 경계 탐색의 경계값(빈 입력·경계 오프셋·클래스 전환·UTF-16)은 직접 검증된
// 적이 없었다. 검증 방식은 selection-ordering-nested-cell.test.ts 의 확립 패턴을 따른다 —
// const enum 이 strip-only 로더에서 거부되므로 자식 프로세스를
// --experimental-transform-types 로 띄우고 registerHooks 로 확장자 없는 상대 import 를 매핑한다.

const studioRoot = path.dirname(path.dirname(fileURLToPath(import.meta.url)));
const workDir = mkdtempSync(path.join(tmpdir(), 'rhwp-word-utils-'));
const driverPath = path.join(workDir, 'driver.mjs');

writeFileSync(driverPath, `
import { registerHooks } from 'node:module';
import { pathToFileURL } from 'node:url';

const srcRoot = ${JSON.stringify(pathToFileURL(path.join(studioRoot, 'src') + path.sep).href)};
registerHooks({
  resolve(specifier, context, nextResolve) {
    if (specifier.startsWith('@/')) return nextResolve(srcRoot + specifier.slice(2) + '.ts', context);
    if (/^\\.{1,2}\\//.test(specifier) && !/\\.[a-z]+$/.test(specifier)) {
      return nextResolve(specifier + '.ts', context);
    }
    return nextResolve(specifier, context);
  },
});

const { findWordBoundaryForward, findWordBoundaryBackward, findWordAt, findSentenceAt } =
  await import(srcRoot + 'engine/cursor-word-utils.ts');

const F = [
  ['', '빈 문자열'],
  [' ', '공백 1개'],
  ['   ', '공백 3개'],
  [' \\t x', '탭 포함 선행 공백'],
  ['a', '단일 문자'],
  ['ab cd', '단어+공백'],
  ['ab   cd', '긴 공백'],
  ['ab ', '후행 공백'],
  ['한글 text', '한글→영문'],
  ['ab12', '영문→숫자'],
  ['12ab', '숫자→영문'],
  ['ㄱㄴㄷ', '자모 연속'],
  ['!!!', '문장부호 연속'],
  ['!!a', '문장부호→영문'],
  ['가나다', '완성형 연속'],
].map(([text]) => ({ text, out: findWordBoundaryForward(text) }));

const B = [
  ['', '빈 문자열'],
  [' ', '공백만'],
  ['abcd', '단일 단어'],
  ['ab cd', '두 단어'],
  ['ab  ', '후행 공백'],
  ['한글과 abc', '한글 뒤 영문'],
  ['12가', '숫자 뒤 한글'],
  ['abcd!', '문장부호 종결'],
].map(([text]) => ({ text, out: findWordBoundaryBackward(text) }));

const A = [
  ['', 0, '빈 텍스트'],
  ['abc', 5, '범위 밖 오프셋'],
  ['ab cd', 2, '단어 사이 공백'],
  ['ab cd', 1, '단어 내부'],
  ['한글과 text', 1, '한글 단어 내부'],
  ['a!!b', 2, '문장부호 군집'],
  ['가나다', 3, '끝 경계'],
  ['ab cd', 0, '시작 경계'],
].map(([text, offset]) => ({ text, offset, out: findWordAt(text, offset) }));

const S = [
  ['', 0, '빈 텍스트'],
  ['abc', 1, '종결부 없음'],
  ['ab. cd', 4, '종결부 직후(공백 스킵)'],
  ['ab. cd. ef', 8, '두번째 종결부 직후'],
  ['ab.', 1, '종결부가 끝'],
  ['ab. cd', 2, '종결부 위'],
  ['문장입니다. 다음', 8, '한글 종결부'],
  ['ab cd', 0, '오프셋 0'],
  ['ab', 10, '클램프 초과'],
  ['a.\\t b', 3, '종결부 뒤 탭+공백'],
  ['終了。次', 3, '한자 종결부(。) 위'],
].map(([text, offset]) => ({ text, offset, out: findSentenceAt(text, offset) }));

console.log(JSON.stringify({ F, B, A, S }));
`);

const run = spawnSync(process.execPath, ['--experimental-transform-types', driverPath], {
  encoding: 'utf8',
});
rmSync(workDir, { recursive: true, force: true });
if (run.status !== 0) throw new Error(`드라이버 실패: ${run.stderr}`);
const observed = JSON.parse(run.stdout);

test('findWordBoundaryForward — 빈 입력·공백 군집', () => {
  const by = (t: string) => observed.F.find((c: any) => c.text === t)!.out;
  assert.equal(by(''), 0, '빈 문자열은 0');
  assert.equal(by(' '), 1, '공백 1개는 전체 스킵');
  assert.equal(by('   '), 3, '공백 군집도 전체 스킵');
  assert.equal(by(' \t x'), 3, '탭도 공백 클래스 — 선행 공백 후 첫 비공백 위치');
});

test('findWordBoundaryForward — 단어 소비 후 후행 공백 포함', () => {
  const by = (t: string) => observed.F.find((c: any) => c.text === t)!.out;
  assert.equal(by('a'), 1);
  assert.equal(by('ab cd'), 3, '단어 + 후행 공백 1개');
  assert.equal(by('ab   cd'), 5, '단어 + 후행 공백 3개');
  assert.equal(by('ab '), 3);
});

test('findWordBoundaryForward — 문자 클래스 전환 경계(한/영/숫자/부호/자모)', () => {
  const by = (t: string) => observed.F.find((c: any) => c.text === t)!.out;
  assert.equal(by('한글 text'), 3, '한글 단어 + 후행 공백');
  assert.equal(by('ab12'), 2, '영문→숫자 전환에서 정지');
  assert.equal(by('12ab'), 2, '숫자→영문 전환에서 정지');
  assert.equal(by('ㄱㄴㄷ'), 3, '자모(0x3131–0x318E)도 한글 클래스');
  assert.equal(by('!!!'), 3, '같은 클래스 문장부호는 하나의 단어로 소비');
  assert.equal(by('!!a'), 2, '문장부호→영문 전환에서 정지');
  assert.equal(by('가나다'), 3, '완성형 연속');
});

test('findWordBoundaryBackward — 빈 입력·공백만·단일 단어', () => {
  const by = (t: string) => observed.B.find((c: any) => c.text === t)!.out;
  assert.equal(by(''), 0);
  assert.equal(by(' '), 0, '공백만이면 0');
  assert.equal(by('abcd'), 0, '단일 단어 시작');
});

test('findWordBoundaryBackward — 후행 공백 스킵 후 단어 시작', () => {
  const by = (t: string) => observed.B.find((c: any) => c.text === t)!.out;
  assert.equal(by('ab cd'), 3, "'cd' 시작");
  assert.equal(by('ab  '), 0, '후행 공백 제거 후 ab 시작');
});

test('findWordBoundaryBackward — 클래스 전환', () => {
  const by = (t: string) => observed.B.find((c: any) => c.text === t)!.out;
  assert.equal(by('한글과 abc'), 4, "영문 'abc' 시작(한글과는 다른 클래스)");
  assert.equal(by('12가'), 2, "한글 '가' 앞 숫자 경계");
  assert.equal(by('abcd!'), 4, "문장부호 '!' 시작(인덱스 4)");
});

test('findWordAt — 빈·범위 밖·경계 오프셋', () => {
  const at = (t: string, o: number) => observed.A.find((c: any) => c.text === t && c.offset === o)!.out;
  assert.deepEqual(at('', 0), { start: 0, end: 0 }, '빈 텍스트');
  assert.deepEqual(at('abc', 5), { start: 5, end: 5 }, '범위 밖 오프셋은 클램프 없이 그대로');
  assert.deepEqual(at('가나다', 3), { start: 3, end: 3 }, '끝 경계');
  assert.deepEqual(at('ab cd', 0), { start: 0, end: 2 }, '시작 경계 — 첫 단어');
});

test('findWordAt — 단어/비단어 군집 확장', () => {
  const at = (t: string, o: number) => observed.A.find((c: any) => c.text === t && c.offset === o)!.out;
  assert.deepEqual(at('ab cd', 2), { start: 2, end: 3 }, '공백 군집은 정확히 그 공백');
  assert.deepEqual(at('ab cd', 1), { start: 0, end: 2 }, '단어 내부는 단어 전체');
  assert.deepEqual(at('한글과 text', 1), { start: 0, end: 3 }, '한글 단어 전체(조사 포함)');
  assert.deepEqual(at('a!!b', 2), { start: 1, end: 3 }, '문장부호 군집');
});

test('findSentenceAt — 빈·종결부 없음·경계', () => {
  const at = (t: string, o: number) => observed.S.find((c: any) => c.text === t && c.offset === o)!.out;
  assert.deepEqual(at('', 0), { start: 0, end: 0 });
  assert.deepEqual(at('abc', 1), { start: 0, end: 3 }, '종결부 없으면 전체');
  assert.deepEqual(at('ab cd', 0), { start: 0, end: 5 }, '오프셋 0');
});

test('findSentenceAt — 종결부 직후 공백/탭 스킵은 clampedOffset 까지만', () => {
  const at = (t: string, o: number) => observed.S.find((c: any) => c.text === t && c.offset === o)!.out;
  assert.deepEqual(at('ab. cd', 4), { start: 4, end: 6 }, '종결부+공백 스킵 후 다음 문장');
  assert.deepEqual(at('ab. cd. ef', 8), { start: 8, end: 10 }, '두번째 문장');
  assert.deepEqual(at('a.\t b', 3), { start: 3, end: 5 }, '탭+공백 스킵이 offset 에서 멈춘다');
});

test('findSentenceAt — 종결부 소비·클램프·CJK 종결부', () => {
  const at = (t: string, o: number) => observed.S.find((c: any) => c.text === t && c.offset === o)!.out;
  assert.deepEqual(at('ab.', 1), { start: 0, end: 3 }, '끝의 종결부 포함(end++)');
  assert.deepEqual(at('ab. cd', 2), { start: 0, end: 3 }, '종결부 위에서 시작');
  assert.deepEqual(at('ab', 10), { start: 0, end: 2 }, '초과 오프셋 클램프');
  assert.deepEqual(at('문장입니다. 다음', 8), { start: 7, end: 9 }, '한글 문장');
  assert.deepEqual(at('終了。次', 3), { start: 3, end: 4 }, '종결부(。) 직후 오프셋 — 다음 문장 "次"만');
});
