/**
 * E2E 특성화(캐릭터리제이션): compare/diff-engine 출력 동결 — god-file 분할의 안전망.
 *
 * diff-engine.ts 는 관심사별 모듈 분할 예정이다. 그런데 비교 엔진은 단위·e2e 어디에서도
 * 직접 검증되지 않았다(간접 경로: 비교·이력 다이얼로그 UI). 순수 이동임을 증명하려면
 * 이동 전 출력을 고정(golden)하고 이동 후에 재현해야 한다.
 *
 * 세 개의 축을 고정한다:
 *   [1] identity 경로 — 편집기에 올린 문서의 편집 전/후 스냅샷 비교(이력 다이얼로그와
 *       동일 흐름, shared stable_id → buildIdentityTextDiffs 분기)
 *   [2] alignment 경로 — 외부 파일 2개 비교(비교 다이얼로그 흐름, 서로 다른 문서의
 *       앵커+구간 DP/그리디 정렬 전체)
 *   [3] 무차 경로 — 동일 바이트 비교(diff 0건 + 세션 형태)
 *
 * 방식: Vite 개발 서버 페이지에서 `/src/compare/diff-engine.ts` 를 dynamic import 해
 * 공개 API(compareSnapshots/buildSnapshotFromWasm/compareDocuments)를 실행하고,
 * `generatedAt`(타임스탬프)만 제외한 세션을 정규화(canonical JSON, 키 정렬)해
 * blake3 로 해시한다.
 *
 * 옵션은 compare-dialog 기본값(src/ui/compare-dialog.ts DEFAULT_COMPARE_OPTS)과 동일하되
 * `maxComputeMs` 만 크게 잡는다 — 타임버짓 초과 시 greedy/fallback 이탈은 실행 속도에
 * 따라 결과가 달라지는 비결정 경로라서 특성화 대상이 아니다(결정적 풀품질 경로를 고정).
 *
 * 해시가 달라지면: (a) 엔진 동작이 정말 바뀐 것이므로 의도된 변경인지 확인하고
 * (b) 의도된 변경이면 GOLDEN 을 갱신하고 커밋 메시지에 근거를 남긴다.
 *
 * 실행:
 *   cd rhwp-studio && npx vite --port 7700 --strictPort &
 *   CHROME_PATH="/Applications/Google Chrome.app/Contents/MacOS/Google Chrome" \
 *     npm run e2e:compare-characterization
 */
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { blake3 } from '@noble/hashes/blake3.js';
import { bytesToHex } from '@noble/hashes/utils.js';

import { runTest, assert, loadHwpFile, clickEditArea, moveCursorToStart, moveCursorTo, typeText } from './helpers.mjs';

const studioRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const repoRoot = path.resolve(studioRoot, '..');

const SAMPLES = {
  sample5V2024: 'hwp3-sample5-hwp5-v2024.hwp',
  sample16: 'hwp3-sample16-hwp5-2022.hwp',
};

/** compare-dialog 기본값 + 특성화용 타임버짓 상향(비결정 이탈 경로 배제). */
const COMPARE_OPTIONS = {
  caseSensitive: true,
  ignoreWhitespace: true,
  kinds: ['text', 'table', 'shape', 'image', 'chart'],
  strategy: 'alignment',
  anchorTuning: {
    minTextLen: 22,
    minUniqueChars: 7,
    maxWhitespaceRatio: 0.58,
    minEntropy: 2.05,
  },
  performanceTuning: {
    maxComputeMs: 600000,
    hardSegmentCells: 160000,
  },
};

const GOLDEN = {
  identity_edit_before_after: '69c644381c157f06e88ce9d33c49490872e3fb810ccadcfbc3c3e0cd296e3886',
  sample16_vs_sample5V2024: '8e991aca08530a8d1657a4069fbda21fe20867aa735e146e307800a1856704c3',
  sample5V2024_identical: 'ad2c6bc9ef5352d3125735966fa9825a084f82233364e17db698c863fb5c066f',
};

function loadSampleBase64(fileName) {
  return fs.readFileSync(path.join(repoRoot, 'samples', fileName)).toString('base64');
}

/** 키 순서·삽입 순서에 무관한 정규화 직렬화 — 분할 전후 동일 판정의 기준. */
function stableStringify(value) {
  if (Array.isArray(value)) {
    return `[${value.map(stableStringify).join(',')}]`;
  }
  if (value !== null && typeof value === 'object') {
    const keys = Object.keys(value).sort();
    return `{${keys.map((k) => `${JSON.stringify(k)}:${stableStringify(value[k])}`).join(',')}}`;
  }
  return JSON.stringify(value);
}

/** compareDocuments(외부 파일 2개) — generatedAt 만 제외한 세션 반환. */
async function runCompareDocuments(page, leftB64, leftName, rightB64, rightName) {
  return page.evaluate(
    async (payload) => {
      const de = await import('/src/compare/diff-engine.ts');
      const bytes = (b64) => Uint8Array.from(atob(b64), (c) => c.charCodeAt(0));
      const session = await de.compareDocuments(
        bytes(payload.leftB64),
        payload.leftName,
        bytes(payload.rightB64),
        payload.rightName,
        payload.options,
      );
      const { generatedAt: _generatedAt, ...stable } = session;
      return JSON.parse(JSON.stringify(stable));
    },
    { leftB64, leftName, rightB64, rightName, options: COMPARE_OPTIONS },
  );
}

function sessionHash(session) {
  return bytesToHex(blake3(new TextEncoder().encode(stableStringify(session))));
}

function kindCounts(diffItems) {
  const counts = {};
  for (const item of diffItems) counts[item.kind] = (counts[item.kind] ?? 0) + 1;
  return counts;
}

runTest('비교 엔진 특성화 — diff-engine 출력 golden 고정(모듈 분할 안전망)', async ({ page }) => {
  // [1] identity 경로: 편집기 문서 편집 전/후 스냅샷 비교(이력 다이얼로그 흐름)
  await loadHwpFile(page, SAMPLES.sample5V2024);
  const identity = await page.evaluate(async (options) => {
    const de = await import('/src/compare/diff-engine.ts');
    const before = de.buildSnapshotFromWasm(window.__wasm, 'before.hwp', options);
    return before;
  }, COMPARE_OPTIONS);
  await clickEditArea(page);
  await moveCursorToStart(page);
  await typeText(page, '특성화삽입문장 ');
  await moveCursorTo(page, 0, 1, 0);
  await typeText(page, '두번째문단삽입 ');
  const identitySession = await page.evaluate(
    async (payload) => {
      const de = await import('/src/compare/diff-engine.ts');
      const after = de.buildSnapshotFromWasm(window.__wasm, 'after.hwp', payload.options);
      const session = de.compareSnapshots(payload.before, after, payload.options);
      const { generatedAt: _generatedAt, ...stable } = session;
      return JSON.parse(JSON.stringify(stable));
    },
    { before: identity, options: COMPARE_OPTIONS },
  );
  assert(identitySession.diffItems.length > 0, `identity 경로 diff ${identitySession.diffItems.length}건 검출`);
  assert(
    identitySession.textCompareStrategyUsed === 'identity',
    `identity 경로 전략 확인(실제 '${identitySession.textCompareStrategyUsed}')`,
  );
  console.log(`  [1] 편집 전/후(identity): ${identitySession.diffItems.length}건 ${JSON.stringify(kindCounts(identitySession.diffItems))}`);
  console.log(`      hash: ${sessionHash(identitySession)}`);

  // [2] alignment 경로: 이질 문서 쌍(공유 앵커 거의 없음 — 정렬 스트레스)
  const s16 = loadSampleBase64(SAMPLES.sample16);
  const v2024 = loadSampleBase64(SAMPLES.sample5V2024);
  const alignmentSession = await runCompareDocuments(page, s16, 'sample16.hwp', v2024, 'sample5-v2024.hwp');
  assert(alignmentSession.diffItems.length > 0, `alignment 경로 diff ${alignmentSession.diffItems.length}건 검출`);
  console.log(`  [2] sample16↔sample5-v2024(alignment): ${alignmentSession.diffItems.length}건`);
  console.log(`      hash: ${sessionHash(alignmentSession)}`);

  // [3] 무차 경로: 동일 바이트 비교(diff 0건 + 세션 형태 고정)
  const identicalSession = await runCompareDocuments(page, v2024, 'a.hwp', v2024, 'b.hwp');
  assert(identicalSession.diffItems.length === 0, `동일 문서 쌍 diff 0건(실제 ${identicalSession.diffItems.length}건)`);
  console.log(`  [3] sample5-v2024 동일쌍: 0건`);
  console.log(`      hash: ${sessionHash(identicalSession)}`);

  // [4] 동일 입력 재실행 — 세션 내 결정성 자체 점검(런타임 가드 상태 오염 여부)
  const alignmentRerun = await runCompareDocuments(page, s16, 'sample16.hwp', v2024, 'sample5-v2024.hwp');
  assert(
    sessionHash(alignmentRerun) === sessionHash(alignmentSession),
    '동일 입력 재실행 결정성 확인(1회차와 동일 해시)',
  );
  console.log('  [4] 재실행 결정성: PASS');

  // [5] golden 대조 — 미기록 키가 있으면 실측값을 안내하고 전체를 실패 처리한다.
  const checks = [
    ['identity_edit_before_after', identitySession],
    ['sample16_vs_sample5V2024', alignmentSession],
    ['sample5V2024_identical', identicalSession],
  ];
  let allRecorded = true;
  for (const [key, session] of checks) {
    const actual = sessionHash(session);
    const expected = GOLDEN[key];
    if (expected === 'TO_BE_RECORDED') {
      allRecorded = false;
      console.log(`  [5] GOLDEN['${key}'] 미기록 — 실측값: ${actual}`);
      continue;
    }
    assert(actual === expected, `${key} golden 일치`);
  }
  assert(allRecorded, 'golden 3건 전부 기록됨');
  if (allRecorded) console.log('  [5] golden 3건 전부 일치: PASS');
});
