import test from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

// [2차 god-file 분할 신뢰 보강] compare/char-diff.ts(P5b 분리)의 myersCharDiffSummary 는
// 지금까지 e2e compare-characterization 의 3경로 golden 으로만 간접 검증됐다. 여기서는
// 문자 단위 diff 의 경계값(빈 입력·동일·삽입/삭제만·대체 연쇄·UTF-16 서로게이트)과
// 생략 가드(총길이·셀 하드리밋)를 직접 확인하고, full-DP 경로(짧은 쌍)와 Hirschberg
// 경로(n×m > CHAR_DIFF_FULL_DP_MAX) 모두에서 독립 구현 Levenshtein 오라클과 교차 검증한다.
// 드라이버 패턴은 selection-ordering-nested-cell.test.ts 를 따른다(registerHooks + transform-types).

const studioRoot = path.dirname(path.dirname(fileURLToPath(import.meta.url)));
const workDir = mkdtempSync(path.join(tmpdir(), 'rhwp-char-diff-'));
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

const { myersCharDiffSummary } = await import(srcRoot + 'compare/char-diff.ts');

// 독립 구현 오라클 — 모듈의 levenshteinDistanceTwoRow 와 같은 점화식을 새로 쓴다.
function oracleDist(a, b) {
  const prev = new Array(b.length + 1);
  for (let j = 0; j <= b.length; j += 1) prev[j] = j;
  for (let i = 1; i <= a.length; i += 1) {
    let diagonal = prev[0];
    prev[0] = i;
    for (let j = 1; j <= b.length; j += 1) {
      const temp = prev[j];
      prev[j] = Math.min(
        prev[j] + 1,
        prev[j - 1] + 1,
        diagonal + (a.charCodeAt(i - 1) === b.charCodeAt(j - 1) ? 0 : 1),
      );
      diagonal = temp;
    }
  }
  return prev[b.length];
}

const parseDist = (summary) => {
  const m = /^편집거리 (\\d+) /.exec(summary);
  return m ? Number(m[1]) : null;
};

const KNOWN = [
  ['', ''],
  ['abc', 'abc'],
  ['', 'ab'],
  ['ab', ''],
  ['kitten', 'sitting'],
  ['ab', 'ba'],
  ['가나다라', '가나타라'],
  ['flat', 'flatter'],
  ['flatter', 'flat'],
].map(([a, b]) => ({ a, b, summary: myersCharDiffSummary(a, b) }));

const SURROGATE = [
  ['😀x', 'x'],
  ['x😀y', 'xy'],
  ['😀', '😃'],
].map(([a, b]) => ({ a, b, summary: myersCharDiffSummary(a, b), oracle: oracleDist(a, b) }));

// 결정적 유사난수(LCG) 쌍 — 짧은 쌍은 full-DP, 긴 쌍(600×600=360k > 280k)은 Hirschberg 경로.
let seed = 0x2f6e2b1;
const rnd = () => { seed = (Math.imul(seed, 1664525) + 1013904223) >>> 0; return seed; };
const gen = (len, alphabet) => Array.from({ length: len }, () => alphabet[(rnd() >>> 16) % alphabet.length]).join('');

const ALPHABETS = ['abcd', 'abc가한1!', 'ab😀c'];
const CORPUS = [];
for (const alphabet of ALPHABETS) {
  for (const [la, lb] of [[20, 20], [40, 37], [80, 65], [600, 600], [600, 540]]) {
    const a = gen(la, alphabet);
    const b = gen(lb, alphabet);
    const summary = myersCharDiffSummary(a, b);
    CORPUS.push({
      alphabet: alphabet.length,
      la,
      lb,
      oracle: oracleDist(a, b),
      dist: parseDist(summary),
      truncated: summary.includes('생략'),
    });
  }
}

const OMIT_TOTAL = myersCharDiffSummary('a'.repeat(50_000), 'a'.repeat(50_000));
const OMIT_CELL = myersCharDiffSummary('x'.repeat(4_000), 'y'.repeat(4_000));
const PREFIX_HEAVY = myersCharDiffSummary('공통'.repeat(200) + '차이A', '공통'.repeat(200) + '차이B');

console.log(JSON.stringify({ KNOWN, SURROGATE, CORPUS, OMIT_TOTAL, OMIT_CELL, PREFIX_HEAVY }));
`);

const run = spawnSync(process.execPath, ['--experimental-transform-types', driverPath], {
  encoding: 'utf8',
});
rmSync(workDir, { recursive: true, force: true });
if (run.status !== 0) throw new Error(`드라이버 실패: ${run.stderr}`);
const o: any = JSON.parse(run.stdout);

const known = (a: string, b: string) => o.KNOWN.find((c: any) => c.a === a && c.b === b)!.summary;
const editOps = (summary: string): number => (summary.match(/[×+\-]/g) ?? []).length;

test('빈·동일 입력은 요약 없음', () => {
  assert.equal(known('', ''), '');
  assert.equal(known('abc', 'abc'), '', '공통 접두/접미 제거 후 빈 구간');
});

test('순수 삽입/삭제', () => {
  assert.equal(known('', 'ab'), '편집거리 2 · ++');
  assert.equal(known('ab', ''), '편집거리 2 · --');
});

test('고전 케이스 — 거리와 패턴의 편집 연산 합 일치', () => {
  const s = known('kitten', 'sitting');
  assert.match(s, /^편집거리 3 · /);
  assert.equal(editOps(s), 3, '×/+/− 연산 수 === 편집거리');
});

test('순서 교환·한글 대체·접두/접미 비대칭', () => {
  assert.match(known('ab', 'ba'), /^편집거리 2 · /);
  assert.equal(editOps(known('ab', 'ba')), 2);
  assert.match(known('가나다라', '가나타라'), /^편집거리 1 · /);
  const grown = known('flat', 'flatter');
  assert.match(grown, /^편집거리 3 · /);
  assert.equal(editOps(grown), 3);
  assert.match(known('flatter', 'flat'), /^편집거리 3 · /);
});

test('UTF-16 서로게이트 — 코드 유닛 단위 거리로 측정된다(현행 규약 고정)', () => {
  for (const c of o.SURROGATE) {
    assert.ok(c.oracle !== null, '오라클 계산됨');
    const dist = /^편집거리 (\d+) /.exec(c.summary);
    assert.ok(dist, `요약 생략 아님: ${c.summary}`);
    assert.equal(Number(dist[1]), c.oracle, `UTF-16 유닛 기반 거리 일치(${c.a} vs ${c.b})`);
  }
  assert.equal(o.SURROGATE[0].oracle, 2, '😀(유닛 2) 삭제는 거리 2');
});

test('생략 가드 — 총길이 상한', () => {
  assert.match(o.OMIT_TOTAL, /^문자 diff 요약 생략\(길이: 50000\+50000\)$/);
});

test('생략 가드 — 셀 하드리밋(접두/접미 제거 후에도 n×m 과대)', () => {
  assert.match(o.OMIT_CELL, /^문자 diff 요약 생략\(과대: 4000×4000\)$/);
});

test('공통 접두가 길면 접두/접미 제거 후 최소 구간만 계산한다', () => {
  assert.match(o.PREFIX_HEAVY, /^편집거리 1 · /);
});

test('full-DP·Hirschberg 양 경로 — 독립 오라클과 거리 일치(결정적 코퍼스 15쌍)', () => {
  assert.equal(o.CORPUS.length, 15);
  for (const c of o.CORPUS) {
    assert.equal(c.truncated, false, `생략 없음(alphabet ${c.alphabet}, ${c.la}×${c.lb})`);
    assert.equal(c.dist, c.oracle, `거리 일치(alphabet ${c.alphabet}, ${c.la}×${c.lb})`);
  }
});

test('코퍼스가 Hirschberg 임계(280k)를 실제로 넘는다', () => {
  const hirschbergPairs = o.CORPUS.filter((c: any) => c.la * c.lb > 280_000);
  assert.equal(hirschbergPairs.length, 6, '600×600·600×540 × 3 알파벳 = 6쌍이 재귀 경로 사용');
});
