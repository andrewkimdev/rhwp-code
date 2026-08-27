/** 문자 단위 diff 요약 — 접두·접미 제거, 전역 DP 잎, Hirschberg. 엔진 전체에서 `myersCharDiffSummary`만 노출한다. */
import { CHAR_DIFF_CELL_HARD, CHAR_DIFF_FULL_DP_MAX, CHAR_DIFF_TOTAL_MAX } from './tuning';

// ─── 문자 diff 요약 (`myersCharDiffSummary`): 접두·접미, 전역 DP 잎, Hirschberg ─

/** 공통 접두·접미를 제거해 Levenshtein/Hirschberg 입력을 줄인다. */
function stripCommonAffixChars(left: string, right: string): { a: string; b: string } {
  let lo = 0;
  const minLen = Math.min(left.length, right.length);
  while (lo < minLen && left.charCodeAt(lo) === right.charCodeAt(lo)) lo += 1;
  let suf = 0;
  while (
    suf < left.length - lo &&
    suf < right.length - lo &&
    left.charCodeAt(left.length - 1 - suf) === right.charCodeAt(right.length - 1 - suf)
  )
    suf += 1;
  return { a: left.slice(lo, left.length - suf), b: right.slice(lo, right.length - suf) };
}

function levenshteinDistanceTwoRow(a: string, b: string): number {
  const n = a.length;
  const m = b.length;
  if (n === 0) return m;
  if (m === 0) return n;
  let prev = new Array<number>(m + 1);
  let cur = new Array<number>(m + 1);
  for (let j = 0; j <= m; j += 1) prev[j] = j;
  for (let i = 1; i <= n; i += 1) {
    cur[0] = i;
    const cAi = a.charCodeAt(i - 1);
    for (let j = 1; j <= m; j += 1) {
      const eq = cAi === b.charCodeAt(j - 1) ? 0 : 1;
      cur[j] = Math.min(prev[j] + 1, cur[j - 1] + 1, prev[j - 1] + eq);
    }
    const t = prev;
    prev = cur;
    cur = t;
  }
  return prev[m];
}

/** `a[0..nrow)` vs `b` — 마지막 행 비용만 (Hirschberg 전반). `nrow===0`이면 삽입만. */
function levenshteinLastRowPrefix(a: string, nrow: number, b: string): number[] {
  const m = b.length;
  if (nrow <= 0) {
    const row = new Array<number>(m + 1);
    for (let j = 0; j <= m; j += 1) row[j] = j;
    return row;
  }
  let prev = new Array<number>(m + 1);
  let cur = new Array<number>(m + 1);
  for (let j = 0; j <= m; j += 1) prev[j] = j;
  for (let i = 1; i <= nrow; i += 1) {
    cur[0] = i;
    const cAi = a.charCodeAt(i - 1);
    for (let j = 1; j <= m; j += 1) {
      const eq = cAi === b.charCodeAt(j - 1) ? 0 : 1;
      cur[j] = Math.min(prev[j] + 1, cur[j - 1] + 1, prev[j - 1] + eq);
    }
    const t = prev;
    prev = cur;
    cur = t;
  }
  return prev;
}

/** `D[i][j]` = ed(`a[i..n)`, `b[j..m)`). `i === mid`인 행만 필요할 때 아래로 채운다. */
function levenshteinSuffixRowAt(a: string, mid: number, b: string): number[] {
  const n = a.length;
  const m = b.length;
  let cur = new Array<number>(m + 1);
  for (let j = 0; j <= m; j += 1) cur[j] = m - j;
  for (let i = n - 1; i >= mid; i -= 1) {
    const next = new Array<number>(m + 1);
    next[m] = n - i;
    const ca = a.charCodeAt(i);
    for (let j = m - 1; j >= 0; j -= 1) {
      const eq = ca === b.charCodeAt(j) ? 0 : 1;
      next[j] = Math.min(cur[j] + 1, next[j + 1] + 1, cur[j + 1] + eq);
    }
    cur = next;
  }
  return cur;
}

/** 전체 `dp` 역추적 — `n*m`이 작을 때만 호출. */
function charEditOpsFullDp(left: string, right: string): string {
  const n = left.length;
  const m = right.length;
  const dp: number[][] = Array.from({ length: n + 1 }, () => Array(m + 1).fill(0));
  for (let i = 0; i <= n; i += 1) dp[i][0] = i;
  for (let j = 0; j <= m; j += 1) dp[0][j] = j;
  for (let i = 1; i <= n; i += 1) {
    for (let j = 1; j <= m; j += 1) {
      const eq = left.charCodeAt(i - 1) === right.charCodeAt(j - 1) ? 0 : 1;
      dp[i][j] = Math.min(dp[i - 1][j] + 1, dp[i][j - 1] + 1, dp[i - 1][j - 1] + eq);
    }
  }
  let i = n;
  let j = m;
  const ops: string[] = [];
  while (i > 0 || j > 0) {
    if (i > 0 && j > 0 && left.charCodeAt(i - 1) === right.charCodeAt(j - 1)) {
      ops.push('=');
      i -= 1;
      j -= 1;
    } else if (i > 0 && dp[i][j] === dp[i - 1][j] + 1) {
      ops.push('-');
      i -= 1;
    } else if (j > 0 && dp[i][j] === dp[i][j - 1] + 1) {
      ops.push('+');
      j -= 1;
    } else if (i > 0 && j > 0) {
      ops.push('×');
      i -= 1;
      j -= 1;
    } else if (i > 0) {
      ops.push('-');
      i -= 1;
    } else {
      ops.push('+');
      j -= 1;
    }
  }
  ops.reverse();
  return ops.join('');
}

function charEditOpsHirschberg(a: string, b: string): string {
  const n = a.length;
  const m = b.length;
  if (n === 0) return '+'.repeat(m);
  if (m === 0) return '-'.repeat(n);
  if (n * m <= CHAR_DIFF_FULL_DP_MAX) {
    return charEditOpsFullDp(a, b);
  }
  const mid = Math.max(1, Math.floor(n / 2));
  const f = levenshteinLastRowPrefix(a, mid, b);
  const suf = levenshteinSuffixRowAt(a, mid, b);
  let bestJ = 0;
  let best = Infinity;
  for (let j = 0; j <= m; j += 1) {
    const s = f[j] + suf[j];
    if (s < best || (s === best && j < bestJ)) {
      best = s;
      bestJ = j;
    }
  }
  return (
    charEditOpsHirschberg(a.slice(0, mid), b.slice(0, bestJ)) +
    charEditOpsHirschberg(a.slice(mid), b.slice(bestJ))
  );
}

/**
 * 동일 stable_id 문단의 문자 단위 편집 거리·간단 패턴 요약 (2-depth diff).
 * 공통 접두·접미 제거 후, 작은 구간은 전역 DP, 큰 구간은 Hirschberg로 선형 메모리에 가깝게 처리한다.
 */
export function myersCharDiffSummary(left: string, right: string): string {
  if (left.length === 0 && right.length === 0) return '';
  if (left.length + right.length > CHAR_DIFF_TOTAL_MAX) {
    return `문자 diff 요약 생략(길이: ${left.length}+${right.length})`;
  }
  const { a, b } = stripCommonAffixChars(left, right);
  const n = a.length;
  const m = b.length;
  if (n === 0 && m === 0) return '';
  if (n * m > CHAR_DIFF_CELL_HARD) {
    return `문자 diff 요약 생략(과대: ${n}×${m})`;
  }
  const dist = levenshteinDistanceTwoRow(a, b);
  const opStr = charEditOpsHirschberg(a, b);
  const pat = opStr.replace(/=+/g, '·').slice(0, 100);
  return `편집거리 ${dist} · ${pat}`;
}
