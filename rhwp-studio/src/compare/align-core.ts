/** 문단 정렬 core — 유사도·구조 근접·비용 체인과 구간 DP/그리디 매처(`matchSegment` 계열). */
import {
  GREEDY_AMBIGUOUS_GAP,
  HALF_MATCH_MAX_PRODUCT,
  INTRA_UNIQUE_SIG_MIN_NESTED,
  INTRA_UNIQUE_SIG_MIN_TOP,
  MATCH_COST_WEAK,
  MATCH_SOFT_SIM_MIN,
  MAX_SEGMENT_RECURSION,
  NEAR_STRUCTURE_COST_DISCOUNT,
  NEAR_STRUCTURE_GREEDY_BONUS,
  NEAR_STRUCTURE_MIN_SIM,
  NEAR_STRUCTURE_SIM_BOOST,
  NORM_HASH_PIN_MIN_TOTAL_PARAS,
  PARA_SPLIT_JOIN_SIM_MIN,
  REMOVED_ADDED_MAX_GLOBAL_GAP,
  REMOVED_ADDED_MAX_PARA_GAP,
  REMOVED_ADDED_MERGE_SIM_MIN,
  SEGMENT_DP_MAX,
  WINDOW_SIZE,
  resolvePerformanceTuning,
  runWithSegmentStructBase,
  shouldBailToGreedy,
} from './tuning';
import type { CompareContext, SegmentAnchorBoundary, SegmentStructBase } from './tuning';
import { simpleHash } from './signature';
import type { CompareAnchorTuning, CompareControlSnapshot, CompareParaSnapshot } from './types';

// ─── alignment 중간 표현: 정렬 결과 한 줄(좌만·우만·양쪽) ─────────────────────
// `AlignedPair[]`는 `buildTextDiffs`가 앵커 경계마다 `matchSegment`를 호출해 쌓은 뒤,
// `buildParagraphAlignStepsFromAligned` → `cleanupParagraphAlignStepsToDiffItems`로 소비된다.
// 컨트롤 슬롯 매칭은 cleanup **이전**의 동일 배열에서 `buildRightToLeftParaMapFromAligned`로 맵만 뽑는다.

/** `matchSegment*`의 출력 원소. null 쪽은 해당 문서에 문단이 없음(삽입/삭제 축). */
export type AlignedPair = {
  left: CompareParaSnapshot | null;
  right: CompareParaSnapshot | null;
};

/** 컨트롤 폴백 매칭 단계에서 확정된 (좌, 우) 쌍 */
export type ControlPair = {
  left: CompareControlSnapshot;
  right: CompareControlSnapshot;
};

/** diff의 path는 보통 "변경 후" 좌표를 우선하지만, 우측이 없으면 좌측을 쓴다. */
export function preferRightPath(
  left: { section: number; paragraph: number } | null,
  right: { section: number; paragraph: number } | null,
): { section: number; paragraph: number } {
  if (right) return { section: right.section, paragraph: right.paragraph };
  if (left) return { section: left.section, paragraph: left.paragraph };
  return { section: 0, paragraph: 0 };
}

/**
 * **구간 한정** 유일 시그니처 앵커(Patience / LCS 스타일).
 *
 * `leftSeg`·`rightSeg` **안에서만** 시그니처 빈도를 세므로, 전역적으로는 중복이어도 구간 안에서만 유일하면 핀이 된다.
 * 양쪽에서 정확히 1번씩만 나오는 시그니처끼리 `ri`를 잡되, `ri`가 단조 증가하도록만 쌍을 채택해 교차 매칭을 막는다.
 * `minTextLen`: 상위 구간은 `INTRA_UNIQUE_SIG_MIN_TOP`, 깊은 재귀는 `INTRA_UNIQUE_SIG_MIN_NESTED`로
 * 짧은 줄의 우연 유일 핀을 줄인다. `tryMatchByNormHashPins`는 `minTextLen=0`으로 같은 로직을 재사용한다.
 */
function buildUniqueSigPairsInSlices(
  leftSeg: CompareParaSnapshot[],
  rightSeg: CompareParaSnapshot[],
  minTextLen: number,
): Array<{ li: number; ri: number }> {
  const countL = new Map<string, number>();
  const countR = new Map<string, number>();
  for (const p of leftSeg) countL.set(p.signature, (countL.get(p.signature) ?? 0) + 1);
  for (const p of rightSeg) countR.set(p.signature, (countR.get(p.signature) ?? 0) + 1);

  const rightBySig = new Map<string, number[]>();
  for (let ri = 0; ri < rightSeg.length; ri += 1) {
    const p = rightSeg[ri];
    if ((countR.get(p.signature) ?? 0) !== 1) continue;
    if (p.normalizedText.length < minTextLen) continue;
    if (!rightBySig.has(p.signature)) rightBySig.set(p.signature, []);
    rightBySig.get(p.signature)!.push(ri);
  }

  const pairs: Array<{ li: number; ri: number }> = [];
  let lastRi = -1;
  for (let li = 0; li < leftSeg.length; li += 1) {
    const lp = leftSeg[li];
    if ((countL.get(lp.signature) ?? 0) !== 1) continue;
    if (lp.normalizedText.length < minTextLen) continue;
    const candidates = rightBySig.get(lp.signature);
    if (!candidates || candidates.length !== 1) continue;
    const ri = candidates[0];
    if (ri <= lastRi) continue;
    pairs.push({ li, ri });
    lastRi = ri;
  }
  return pairs;
}

/** 문서 전체에서 `normalizedText.trim()` 문자열이 몇 번 나오는지 센다(빈 문자열 제외). */
function countTrimNormText(paras: CompareParaSnapshot[]): Map<string, number> {
  const m = new Map<string, number>();
  for (const p of paras) {
    const k = p.normalizedText.trim();
    if (!k) continue;
    m.set(k, (m.get(k) ?? 0) + 1);
  }
  return m;
}

/**
 * trim `normalizedText` 가 왼쪽·오른쪽 문서에서 각각 정확히 한 번만 등장하는 문자열(교집합).
 * 짧은 구조 앵커 후보가 전역적으로 우연 중복이 아닌지 확인할 때 사용한다.
 */
function buildBothSidesUniqueTrimNorm(left: CompareParaSnapshot[], right: CompareParaSnapshot[]): Set<string> {
  const cl = countTrimNormText(left);
  const cr = countTrimNormText(right);
  const out = new Set<string>();
  for (const [k, v] of cl) {
    if (v !== 1) continue;
    if (cr.get(k) === 1) out.add(k);
  }
  return out;
}

/**
 * 전역 문단 배열에서 유일한 동일 시그니처 쌍을 단조 증가하는 `ri`로만 잡아 alignment 구간 경계를 만든다.
 * 후보는 `fillSnapshotFromWasm`에서 `isAnchorCandidate`로 거른다(기본은 길이·엔트로피, 예외는 짧은 블록 제목·Outline/Number).
 * `minTextLen` 미만인 후보는 trim 본문이 양쪽 문서에서 각각 한 번만 나올 때만 앵커로 승인한다.
 */
export function buildAnchorPairs(
  left: CompareParaSnapshot[],
  right: CompareParaSnapshot[],
  anchorTuning: Required<CompareAnchorTuning>,
): Array<{ li: number; ri: number }> {
  const shortOkTrim = buildBothSidesUniqueTrimNorm(left, right);
  const rightBySig = new Map<string, number[]>();
  for (let i = 0; i < right.length; i += 1) {
    const p = right[i];
    if (!p.isAnchorCandidate) continue;
    if (!rightBySig.has(p.signature)) rightBySig.set(p.signature, []);
    rightBySig.get(p.signature)!.push(i);
  }

  const pairs: Array<{ li: number; ri: number }> = [];
  let lastRi = -1;
  for (let li = 0; li < left.length; li += 1) {
    const lp = left[li];
    if (!lp.isAnchorCandidate) continue;
    const t = lp.normalizedText.trim();
    if (t.length < anchorTuning.minTextLen && !shortOkTrim.has(t)) continue;
    const candidates = rightBySig.get(lp.signature);
    if (!candidates || candidates.length !== 1) continue; // 앵커 오염 방지: 단일 매치만 앵커로 인정
    const ri = candidates[0];
    if (ri <= lastRi) continue;
    pairs.push({ li, ri });
    lastRi = ri;
  }
  return pairs;
}

// ─── 문단 정렬: 유사도·구조·비용 (DP / 그리디 공통) ─────────────────────────────
// `textSimilarity`: 스냅샷의 normalizedText 기준. 정렬 외 일부 휴리스틱에서도 호출된다.
// `isNearStructure` / `getEffectiveSimilarity` / `matchCost` / `scorePairGreedy` 는 문단 쌍 정렬 전용.

/** 문자 2-gram Dice 계수. 공백 제거 문자열에 사용해 형태소 단위 토큰이 없을 때도 신호를 남긴다. */
function charBigramSimilarity(a: string, b: string): number {
  const aa = Array.from(a);
  const bb = Array.from(b);
  if (aa.length < 2 || bb.length < 2) return 0;
  const gramsA = new Set<string>();
  const gramsB = new Set<string>();
  for (let i = 0; i < aa.length - 1; i += 1) gramsA.add(`${aa[i]}${aa[i + 1]}`);
  for (let i = 0; i < bb.length - 1; i += 1) gramsB.add(`${bb[i]}${bb[i + 1]}`);
  if (gramsA.size === 0 || gramsB.size === 0) return 0;
  let inter = 0;
  for (const g of gramsA) if (gramsB.has(g)) inter += 1;
  return (2 * inter) / (gramsA.size + gramsB.size);
}

/**
 * 토큰(공백 분리) + 문자 바이그램 혼합 유사도.
 * 한국어는 조사/어미만 바뀌어도 토큰 집합이 크게 달라질 수 있어 `charSim`에 0.8 가중.
 * `containsBoost`는 짧은 문자열이 긴 쪽에 거의 그대로 포함되는 경우(번호 접미 등)를 보정.
 */
export function textSimilarity(a: string, b: string): number {
  if (!a && !b) return 1;
  if (!a || !b) return 0;
  if (a === b) return 1;

  const sa = new Set(a.split(/\s+/).filter(Boolean));
  const sb = new Set(b.split(/\s+/).filter(Boolean));
  const tokenSim =
    sa.size === 0 || sb.size === 0
      ? 0
      : (() => {
          let inter = 0;
          for (const t of sa) if (sb.has(t)) inter += 1;
          return (2 * inter) / (sa.size + sb.size);
        })();

  // 문자 유사도는 공백 변형 영향을 줄이기 위해 공백 제거본으로 계산한다.
  const aNoWs = a.replace(/\s+/g, '');
  const bNoWs = b.replace(/\s+/g, '');
  const charSim = charBigramSimilarity(aNoWs, bNoWs);

  // 한쪽이 다른 쪽을 거의 포함하면(예: "맛있다" -> "맛있다2") 동일 문단 가능성이 높다.
  const shorter = a.length <= b.length ? a : b;
  const longer = a.length <= b.length ? b : a;
  const containsBoost = shorter.length >= 4 && longer.includes(shorter) ? 0.96 : 0;

  // 문단 정렬: 한국어는 조사·어미만 바뀌어도 공백 토큰이 크게 달라지므로 바이그램 쪽 비중을 둔다.
  const mixed = tokenSim * 0.2 + charSim * 0.8;
  return Math.max(mixed, containsBoost);
}

/**
 * 구역·문단 번호·(상대)globalIndex·컨트롤 수가 "같은 슬롯의 이웃"으로 볼 만한 쌍.
 * - `matchSegment` 안에서는 `ctx.segmentStructBase` 기준으로 베이스 대비 오프셋 차만 본다(베이스는 직전
 *   글로벌 앵커 쌍이 있으면 그 `globalIndex`, 없으면 구간 첫 문단).
 * - 그 밖(후처리 등)에서는 전역 `globalIndex` 차 ≤2를 그대로 사용한다.
 */
function isNearStructure(ctx: CompareContext, lp: CompareParaSnapshot, rp: CompareParaSnapshot): boolean {
  if (lp.section !== rp.section) return false;
  if (Math.abs(lp.paragraph - rp.paragraph) > 1) return false;
  if (lp.controlCount !== rp.controlCount) return false;
  if (ctx.segmentStructBase) {
    const dL = lp.globalIndex - ctx.segmentStructBase.leftBaseGi;
    const dR = rp.globalIndex - ctx.segmentStructBase.rightBaseGi;
    return Math.abs(dL - dR) <= 2;
  }
  return Math.abs(lp.globalIndex - rp.globalIndex) <= 2;
}

/** 공백 제거 기준 양쪽 문단이 충분히 길 때만 임계/부스트 완화(짧은 문단 노이즈 억제) */
function isNearStructureLongPair(lp: CompareParaSnapshot, rp: CompareParaSnapshot): boolean {
  const lenL = lp.normalizedText.replace(/\s+/g, '').length;
  const lenR = rp.normalizedText.replace(/\s+/g, '').length;
  return lenL > 15 && lenR > 15;
}

/**
 * textSimilarity 통과 하한: 전역은 엄격, 구조 근접 시만 완화(짧은 문단은 우연 일치 억제).
 * `textSimilarity` 자체는 수정하지 않고, DP/그리디의 `matchCost`·`scorePairGreedy`에서만 사용한다.
 */
function softSimilarityThresholdForPair(ctx: CompareContext, lp: CompareParaSnapshot, rp: CompareParaSnapshot): number {
  if (!isNearStructure(ctx, lp, rp)) return MATCH_SOFT_SIM_MIN;
  return isNearStructureLongPair(lp, rp) ? 0.25 : 0.45;
}

/**
 * DP/그리디 평가 전용 유사도. `textSimilarity`는 순수 값으로 두고, 구조 근접·긴 문단일 때만 보정한다.
 */
function getEffectiveSimilarity(ctx: CompareContext, lp: CompareParaSnapshot, rp: CompareParaSnapshot): number {
  const rawSim = textSimilarity(lp.normalizedText, rp.normalizedText);
  if (!isNearStructure(ctx, lp, rp)) return rawSim;
  if (!isNearStructureLongPair(lp, rp)) return rawSim;
  if (rawSim > NEAR_STRUCTURE_MIN_SIM) {
    return Math.max(rawSim, NEAR_STRUCTURE_SIM_BOOST);
  }
  return rawSim;
}

/** 매칭 비용: 낮을수록 좋음 (0 = 완전 일치). 서명 불일치 + 낮은 유사도는 치환 경로를 막아 이웃 문단 오매칭을 줄인다. */
function matchCost(ctx: CompareContext, lp: CompareParaSnapshot, rp: CompareParaSnapshot): number {
  if (lp.signature === rp.signature) return 0;
  const sim = getEffectiveSimilarity(ctx, lp, rp);
  const threshold = softSimilarityThresholdForPair(ctx, lp, rp);
  if (sim < threshold) return MATCH_COST_WEAK;
  let c = 1 - sim;
  if (lp.controlCount !== rp.controlCount) c += 0.35;
  if (isNearStructure(ctx, lp, rp)) c = Math.max(0, c - NEAR_STRUCTURE_COST_DISCOUNT);
  return c;
}

/** 원본 문단을 복제하되 `signature`만 `nh:<hash(trim)>`로 바꿔, 유일 시그니처 핀 로직을 재사용한다. */
function paraWithNormHashSig(p: CompareParaSnapshot): CompareParaSnapshot {
  const key = p.normalizedText.trim();
  const h = key ? simpleHash(key) : 'empty';
  return { ...p, signature: `nh:${h}` };
}

/**
 * DMP `diff_lineMode_`와 비슷한 **2단계 중 거시**: 구간 문단을 줄 단위 해시 토큰으로 본 뒤
 * `buildUniqueSigPairsInSlices(..., minTextLen=0)`로 구간 내 양쪽 유일 쌍을 핀으로 세운다.
 * 원문 시그니처만으로는 유일 핀이 없을 때(대량 삽입으로 서명이 겹칠 때) 큰 덩어리를 가른다.
 * `NORM_HASH_PIN_MIN_TOTAL_PARAS` 미만이면 호출하지 않는다.
 */
function tryMatchByNormHashPins(
  ctx: CompareContext,
  leftSeg: CompareParaSnapshot[],
  rightSeg: CompareParaSnapshot[],
  depth: number,
  anchorBoundary: SegmentAnchorBoundary | null,
): AlignedPair[] | null {
  const n = leftSeg.length;
  const m = rightSeg.length;
  if (n + m < NORM_HASH_PIN_MIN_TOTAL_PARAS) return null;
  const lh = leftSeg.map(paraWithNormHashSig);
  const rh = rightSeg.map(paraWithNormHashSig);
  const intra0 = buildUniqueSigPairsInSlices(lh, rh, 0);
  if (intra0.length === 0) return null;
  return runInternalAnchorBoundaries(ctx, leftSeg, rightSeg, intra0, depth, anchorBoundary);
}

/**
 * 내부 핀(`intra`)을 경계로 삼아 슬라이스를 나누고, 각 조각에 `matchSegment`를 재귀한다.
 * `intra`는 시그니처 기반(`matchSegmentWithInternalAnchors`)이든 해시 기반(`tryMatchByNormHashPins`)이든 동일.
 */
function runInternalAnchorBoundaries(
  ctx: CompareContext,
  leftSeg: CompareParaSnapshot[],
  rightSeg: CompareParaSnapshot[],
  intra: Array<{ li: number; ri: number }>,
  depth: number,
  anchorBoundary: SegmentAnchorBoundary | null,
): AlignedPair[] {
  const boundaries = [{ li: -1, ri: -1 }, ...intra, { li: leftSeg.length, ri: rightSeg.length }];
  const out: AlignedPair[] = [];
  for (let i = 0; i < boundaries.length - 1; i += 1) {
    const a = boundaries[i];
    const b = boundaries[i + 1];
    if (a.li >= 0 && a.ri >= 0) {
      out.push({ left: leftSeg[a.li], right: rightSeg[a.ri] });
    }
    const ls = leftSeg.slice(a.li + 1, b.li);
    const rs = rightSeg.slice(a.ri + 1, b.ri);
    if (ls.length === 0 && rs.length === 0) continue;
    out.push(...matchSegment(ctx, ls, rs, depth + 1, anchorBoundary));
  }
  return out;
}

/**
 * 구간 내부 유일 시그니처 앵커로 쪼개 각 조각에 `matchSegment` 재귀.
 * Git patience와 유사: DP에 넘기기 전 구간을 최대한 잘게 나눈다.
 */
function matchSegmentWithInternalAnchors(
  ctx: CompareContext,
  leftSeg: CompareParaSnapshot[],
  rightSeg: CompareParaSnapshot[],
  depth: number,
  anchorBoundary: SegmentAnchorBoundary | null,
): AlignedPair[] {
  const intraMin = depth >= 1 ? INTRA_UNIQUE_SIG_MIN_NESTED : INTRA_UNIQUE_SIG_MIN_TOP;
  const intra = buildUniqueSigPairsInSlices(leftSeg, rightSeg, intraMin);
  if (intra.length === 0) return [];
  return runInternalAnchorBoundaries(ctx, leftSeg, rightSeg, intra, depth, anchorBoundary);
}

/**
 * 문단 시그니처가 **연속으로** 동일한 최장 구간(시작 인덱스 포함).
 * DMP half-match 유사: 그리디 직전 큰 구간을 한 번 가른다.
 */
function findLongestEqualSignatureRun(
  leftSeg: CompareParaSnapshot[],
  rightSeg: CompareParaSnapshot[],
): { li0: number; ri0: number; len: number } | null {
  const n = leftSeg.length;
  const m = rightSeg.length;
  if (n === 0 || m === 0) return null;
  if (n * m > HALF_MATCH_MAX_PRODUCT) return null;
  let bestLen = 0;
  let bestI = 0;
  let bestJ = 0;
  for (let i = 0; i < n; i += 1) {
    for (let j = 0; j < m; j += 1) {
      if (leftSeg[i].signature !== rightSeg[j].signature) continue;
      let k = 0;
      while (i + k < n && j + k < m && leftSeg[i + k].signature === rightSeg[j + k].signature) k += 1;
      if (k > bestLen) {
        bestLen = k;
        bestI = i;
        bestJ = j;
      }
    }
  }
  if (bestLen < 1) return null;
  const touchesAll = bestI === 0 && bestJ === 0 && bestLen === n && bestLen === m;
  if (touchesAll) return null;
  const hasLeftover = bestI > 0 || bestJ > 0 || bestI + bestLen < n || bestJ + bestLen < m;
  if (!hasLeftover) return null;
  if (bestLen < 2 && n + m < 12) return null;
  return { li0: bestI, ri0: bestJ, len: bestLen };
}

/**
 * 앵커 사이(또는 재귀 하위) 한 구간의 좌·우 문단 슬라이스를 `AlignedPair[]`로 정렬한다.
 *
 * 순서(대략):
 * 1. `runWithSegmentStructBase`: `anchorBoundary`가 있으면 그 앵커의 globalIndex를 구조 베이스로,
 *    없으면 슬라이스 첫 문단 쌍을 베이스로 `isNearStructure`가 동작하게 한다.
 * 2. 시간 가드(`shouldBailToGreedy`) → 그리디 조기 종료.
 * 3. `buildUniqueSigPairsInSlices`로 내부 유일 시그니처 핀이 있으면 `matchSegmentWithInternalAnchors`.
 * 4. 없으면 `tryMatchByNormHashPins`(큰 구간만).
 * 5. 셀 수 한도 내면 `matchSegmentDp`, 아니면 `findLongestEqualSignatureRun`으로 한 번 가르고 재귀,
 *    그것도 어렵면 `matchWindowedGreedy`.
 */
export function matchSegment(
  ctx: CompareContext,
  leftSeg: CompareParaSnapshot[],
  rightSeg: CompareParaSnapshot[],
  depth = 0,
  anchorBoundary: SegmentAnchorBoundary | null = null,
): AlignedPair[] {
  const n = leftSeg.length;
  const m = rightSeg.length;
  const perf = resolvePerformanceTuning(ctx.options);
  if (n === 0 && m === 0) return [];
  if (n === 0) return rightSeg.map((rp) => ({ left: null, right: rp }));
  if (m === 0) return leftSeg.map((lp) => ({ left: lp, right: null }));

  const structBase: SegmentStructBase = anchorBoundary
    ? { leftBaseGi: anchorBoundary.leftAnchorGi, rightBaseGi: anchorBoundary.rightAnchorGi }
    : { leftBaseGi: leftSeg[0].globalIndex, rightBaseGi: rightSeg[0].globalIndex };

  return runWithSegmentStructBase(ctx, structBase, () => {
    if (shouldBailToGreedy(ctx)) return matchWindowedGreedy(ctx, leftSeg, rightSeg);

    if (depth >= MAX_SEGMENT_RECURSION) {
      if (n * m <= SEGMENT_DP_MAX * SEGMENT_DP_MAX && n * m <= perf.hardSegmentCells) {
        return matchSegmentDp(ctx, leftSeg, rightSeg);
      }
      return matchWindowedGreedy(ctx, leftSeg, rightSeg);
    }

    const intraMin = depth >= 1 ? INTRA_UNIQUE_SIG_MIN_NESTED : INTRA_UNIQUE_SIG_MIN_TOP;
    const intra = buildUniqueSigPairsInSlices(leftSeg, rightSeg, intraMin);
    if (intra.length > 0) {
      return matchSegmentWithInternalAnchors(ctx, leftSeg, rightSeg, depth, anchorBoundary);
    }

    const hashPinned = tryMatchByNormHashPins(ctx, leftSeg, rightSeg, depth, anchorBoundary);
    if (hashPinned) return hashPinned;

    if (n * m <= SEGMENT_DP_MAX * SEGMENT_DP_MAX && n * m <= perf.hardSegmentCells) {
      return matchSegmentDp(ctx, leftSeg, rightSeg);
    }

    const run = findLongestEqualSignatureRun(leftSeg, rightSeg);
    if (run && depth + 1 < MAX_SEGMENT_RECURSION) {
      const { li0, ri0, len } = run;
      const out: AlignedPair[] = [];
      out.push(...matchSegment(ctx, leftSeg.slice(0, li0), rightSeg.slice(0, ri0), depth + 1, anchorBoundary));
      for (let k = 0; k < len; k += 1) {
        out.push({ left: leftSeg[li0 + k], right: rightSeg[ri0 + k] });
      }
      out.push(...matchSegment(ctx, leftSeg.slice(li0 + len), rightSeg.slice(ri0 + len), depth + 1, anchorBoundary));
      return out;
    }

    return matchWindowedGreedy(ctx, leftSeg, rightSeg);
  });
}

/**
 * 표준 2문자열 편집 DP를 "문단 시퀀스"에 적용한 것. `dp[i][j]` = 왼쪽 i개·오른쪽 j개까지 최소 비용.
 * - 치환 비용은 `matchCost`(0~수렴)이고 삽입/삭제는 고정 `del`/`ins`(1.05)로 스무딩한다.
 * - 백트래킹 동률 시 `match > delete > insert` 순으로 분기해, 삽입으로만 밀린 것처럼 보이는 경향을 줄인다.
 * - 연속한 `matchCost===0`(시그니처 일치 등) 대각선은 한 번에 소비(snake)해 백트래킹 루프 비용을 줄인다.
 */
function matchSegmentDp(ctx: CompareContext, leftSeg: CompareParaSnapshot[], rightSeg: CompareParaSnapshot[]): AlignedPair[] {
  if (shouldBailToGreedy(ctx)) return matchWindowedGreedy(ctx, leftSeg, rightSeg);
  const n = leftSeg.length;
  const m = rightSeg.length;
  const inf = 1e9;
  /** 문단 한 줄 삽입/삭제 비용. 1.0보다 약간 크게 두어 무의미한 치환 남발을 억제 */
  const del = 1.05;
  const ins = 1.05;
  const dp: number[][] = Array.from({ length: n + 1 }, () => Array(m + 1).fill(inf));
  dp[0][0] = 0;
  for (let i = 1; i <= n; i += 1) dp[i][0] = dp[i - 1][0] + del;
  for (let j = 1; j <= m; j += 1) dp[0][j] = dp[0][j - 1] + ins;

  for (let i = 1; i <= n; i += 1) {
    if (i % 12 === 0 && shouldBailToGreedy(ctx)) return matchWindowedGreedy(ctx, leftSeg, rightSeg);
    for (let j = 1; j <= m; j += 1) {
      const mc = matchCost(ctx, leftSeg[i - 1], rightSeg[j - 1]);
      dp[i][j] = Math.min(dp[i - 1][j - 1] + mc, dp[i - 1][j] + del, dp[i][j - 1] + ins);
    }
  }

  const eps = 1e-5;
  const out: AlignedPair[] = [];
  let i = n;
  let j = m;
  while (i > 0 || j > 0) {
    const candMatch =
      i > 0 && j > 0
        ? dp[i - 1][j - 1] + matchCost(ctx, leftSeg[i - 1], rightSeg[j - 1])
        : inf;
    const candDel = i > 0 ? dp[i - 1][j] + del : inf;
    const candIns = j > 0 ? dp[i][j - 1] + ins : inf;
    const target = dp[i][j];
    if (i > 0 && j > 0 && Math.abs(candMatch - target) < eps) {
      out.push({ left: leftSeg[i - 1], right: rightSeg[j - 1] });
      i -= 1;
      j -= 1;
      // Snake: 이후 `matchCost===0`이고 대각선이 최적을 유지하는 동안 연속 소비.
      while (i > 0 && j > 0) {
        const lp = leftSeg[i - 1];
        const rp = rightSeg[j - 1];
        if (matchCost(ctx, lp, rp) !== 0) break;
        if (Math.abs(dp[i][j] - dp[i - 1][j - 1]) >= eps) break;
        const delC = dp[i - 1][j] + del;
        const insC = dp[i][j - 1] + ins;
        if (delC < dp[i][j] - eps || insC < dp[i][j] - eps) break;
        out.push({ left: lp, right: rp });
        i -= 1;
        j -= 1;
      }
      continue;
    } else if (i > 0 && Math.abs(candDel - target) < eps) {
      out.push({ left: leftSeg[i - 1], right: null });
      i -= 1;
    } else if (j > 0 && Math.abs(candIns - target) < eps) {
      out.push({ left: null, right: rightSeg[j - 1] });
      j -= 1;
    } else {
      if (i > 0 && j > 0) {
        out.push({ left: leftSeg[i - 1], right: rightSeg[j - 1] });
        i -= 1;
        j -= 1;
      } else if (i > 0) {
        out.push({ left: leftSeg[i - 1], right: null });
        i -= 1;
      } else {
        out.push({ left: null, right: rightSeg[j - 1] });
        j -= 1;
      }
    }
  }
  out.reverse();
  return out;
}

/**
 * 윈도 그리디 전용 점수. 음수면 후보에서 제외(`sim`이 쌍별 임계 미만 등).
 * 서명 일치(+5)·컨트롤 수 일치(+1)·구조 근접(`NEAR_STRUCTURE_GREEDY_BONUS`)에 `sim*3`을 더해 스케일을 맞춘다.
 */
function scorePairGreedy(ctx: CompareContext, lp: CompareParaSnapshot, rp: CompareParaSnapshot): number {
  const sim = getEffectiveSimilarity(ctx, lp, rp);
  if (lp.signature !== rp.signature && sim < softSimilarityThresholdForPair(ctx, lp, rp)) return -1;
  let score = 0;
  if (lp.signature === rp.signature) score += 5;
  if (lp.controlCount === rp.controlCount) score += 1;
  if (isNearStructure(ctx, lp, rp)) score += NEAR_STRUCTURE_GREEDY_BONUS;
  return score + sim * 3;
}

/**
 * 대구간 폴백: 오른쪽 문단 순서대로 왼쪽 후보를 윈도 안에서 고른 뒤, 점수로 매칭.
 * - `minScore`: 서명이 다른 의역 문단은 sim*3+보너스로도 낮게 나올 수 있어 NEAR_STRUCTURE_GREEDY_BONUS 로 보완.
 * - `ambiguous`: 1·2위 점수 차가 GREEDY_AMBIGUOUS_GAP 미만이면 매칭 포기. 단 1위가 `isNearStructure`이면
 *   동률 검사 생략(의역 쌍이 엉뚱한 2위와 0.2점 차로 동반 탈락하는 것 방지).
 */
function matchWindowedGreedy(ctx: CompareContext, leftSeg: CompareParaSnapshot[], rightSeg: CompareParaSnapshot[]): AlignedPair[] {
  const aligned: AlignedPair[] = [];
  const usedLeft = new Set<number>();
  let leftCursor = 0;
  /** 후보 거절 임계. 튜닝 시 `scorePairGreedy` 최솟값(서명 불일치·의역)과 함께 맞출 것 */
  const minScore = 3.45;

  const pickBestInRange = (rp: CompareParaSnapshot, start: number, end: number) => {
    let bestLi = -1;
    let bestScore = -1;
    let secondScore = -1;
    const lo = Math.max(0, start);
    const hi = Math.min(leftSeg.length - 1, end);
    for (let li = lo; li <= hi; li += 1) {
      if (usedLeft.has(li)) continue;
      const s = scorePairGreedy(ctx, leftSeg[li], rp);
      if (s < 0) continue;
      if (s > bestScore) {
        secondScore = bestScore;
        bestScore = s;
        bestLi = li;
      } else if (s > secondScore) {
        secondScore = s;
      }
    }
    return { bestLi, bestScore, secondScore };
  };

  for (let ri = 0; ri < rightSeg.length; ri += 1) {
    if (ri % 32 === 0 && shouldBailToGreedy(ctx)) {
      for (let rj = ri; rj < rightSeg.length; rj += 1) aligned.push({ left: null, right: rightSeg[rj] });
      for (let li = 0; li < leftSeg.length; li += 1) {
        if (!usedLeft.has(li)) aligned.push({ left: leftSeg[li], right: null });
      }
      return aligned;
    }
    const rp = rightSeg[ri];
    // 이전 매칭 위치 근처를 먼저 본 뒤, 실패 시 전 구간 재탐색(의역으로 인덱스가 크게 어긋난 경우).
    const start = Math.max(leftCursor, 0);
    const end = Math.min(leftSeg.length - 1, leftCursor + WINDOW_SIZE + WINDOW_SIZE);
    let { bestLi, bestScore, secondScore } = pickBestInRange(rp, start, end);

    if (bestLi < 0 || bestScore < minScore) {
      const full = pickBestInRange(rp, 0, leftSeg.length - 1);
      if (full.bestScore > bestScore) {
        bestLi = full.bestLi;
        bestScore = full.bestScore;
        secondScore = full.secondScore;
      }
    }

    const isBestNear = bestLi >= 0 && isNearStructure(ctx, leftSeg[bestLi], rp);
    const ambiguous =
      !isBestNear && bestLi >= 0 && secondScore >= 0 && bestScore - secondScore < GREEDY_AMBIGUOUS_GAP;
    if (bestLi >= 0 && bestScore >= minScore && !ambiguous) {
      usedLeft.add(bestLi);
      aligned.push({ left: leftSeg[bestLi], right: rp });
      leftCursor = bestLi;
    } else {
      aligned.push({ left: null, right: rp });
    }
  }

  for (let li = 0; li < leftSeg.length; li += 1) {
    if (!usedLeft.has(li)) aligned.push({ left: leftSeg[li], right: null });
  }
  return aligned;
}

/** 왼쪽 한 문단이 오른쪽 연속 두 문단으로만 쪼개진 경우(순서: 앞·뒤) */
export function isLeftParagraphSplitIntoTwoRightParas(
  left: CompareParaSnapshot,
  rightHead: CompareParaSnapshot,
  rightTail: CompareParaSnapshot,
): boolean {
  const joined = `${rightHead.normalizedText} ${rightTail.normalizedText}`.trim();
  if (!joined) return false;
  const leftN = left.normalizedText.replace(/\s+/g, ' ').trim();
  const joinedN = joined.replace(/\s+/g, ' ').trim();
  if (!leftN) return false;
  // 중간 삽입으로 밀린 경우: R앞에 원문에 없던 긴 텍스트가 끼면 이어붙인 길이가 원문보다 커진다.
  // textSimilarity의 contains 부스트만으로는 이 경우도 '분할'로 오인할 수 있어 길이로 한 번 걸러낸다.
  const maxExtra = Math.max(2, Math.ceil(leftN.length * 0.06));
  if (joinedN.length > leftN.length + maxExtra) return false;
  return textSimilarity(left.normalizedText, joined) >= PARA_SPLIT_JOIN_SIM_MIN;
}

/**
 * 유사도만으로는 잡기 어려운 케이스 보정:
 * - 기존 빈 문단에 텍스트를 입력한 경우(sim=0에 가까움)도
 *   구조 신호(구역/문단 위치/컨트롤 수)가 일치하면 "텍스트 변경"으로 본다.
 */
export function shouldPromoteEmptyTextEdit(
  left: CompareParaSnapshot,
  right: CompareParaSnapshot,
  leftParas: CompareParaSnapshot[],
  rightParas: CompareParaSnapshot[],
): boolean {
  if (left.section !== right.section) return false;
  // 문서 비교(alignment)에서는 앞쪽 삽입/삭제로 인덱스가 쉽게 밀리므로
  // 빈문단 편집 승격은 근접 허용폭을 약간 넓힌다.
  if (Math.abs(left.paragraph - right.paragraph) > 2) return false;
  if (Math.abs(left.globalIndex - right.globalIndex) > 4) return false;
  if (left.controlCount !== right.controlCount) return false;
  const lEmpty = left.normalizedText.length === 0;
  const rEmpty = right.normalizedText.length === 0;
  if (lEmpty === rEmpty) return false;
  const leftByGlobal = new Map<number, CompareParaSnapshot>(leftParas.map((p) => [p.globalIndex, p] as const));
  const rightByGlobal = new Map<number, CompareParaSnapshot>(rightParas.map((p) => [p.globalIndex, p] as const));

  const leftPrev = leftByGlobal.get(left.globalIndex - 1) ?? null;
  const leftNext = leftByGlobal.get(left.globalIndex + 1) ?? null;
  const rightPrev = rightByGlobal.get(right.globalIndex - 1) ?? null;
  const rightNext = rightByGlobal.get(right.globalIndex + 1) ?? null;

  const isEmptyPara = (p: CompareParaSnapshot | null) => !!p && p.normalizedText.length === 0;

  // 연속 빈 문단 구간은 원래 보수적으로 차단했지만,
  // 실제 문서에서는 "빈 문단 -> 텍스트 입력" 케이스가 여기에 걸려 added/removed로 남는 경우가 잦다.
  // 슬롯 근접(문단/전역 인덱스)일 때는 빈 이웃이 있어도 승격을 허용한다.
  const hasAdjacentEmpty =
    isEmptyPara(leftPrev) || isEmptyPara(leftNext) || isEmptyPara(rightPrev) || isEmptyPara(rightNext);
  if (hasAdjacentEmpty) {
    const nearSlot =
      Math.abs(left.paragraph - right.paragraph) <= 2 &&
      Math.abs(left.globalIndex - right.globalIndex) <= 4;
    if (!nearSlot) return false;
  }

  // 양옆 문맥 정합성: 바로 위/아래 문단 중 하나 이상은 시그니처가 유지되어야 한다.
  const prevStable =
    !!leftPrev &&
    !!rightPrev &&
    leftPrev.section === rightPrev.section &&
    leftPrev.signature === rightPrev.signature;
  const nextStable =
    !!leftNext &&
    !!rightNext &&
    leftNext.section === rightNext.section &&
    leftNext.signature === rightNext.signature;
  if (!prevStable && !nextStable) {
    // 양옆 시그니처가 모두 흔들린 케이스에서도,
    // 같은 슬롯 근처(문단/전역 인덱스가 충분히 가까운 경우)면 빈문단 편집으로 본다.
    const nearSlot =
      Math.abs(left.paragraph - right.paragraph) <= 2 &&
      Math.abs(left.globalIndex - right.globalIndex) <= 4;
    if (!nearSlot) return false;
  }

  return true;
}

/**
 * 정렬이 (삭제, 추가)로 쪼개졌지만 텍스트는 같은 문단의 수정에 가깝다고 볼 때(밀림·globalIndex 한계 보정).
 */
export function shouldMergeRemovedAddedAsModify(lp: CompareParaSnapshot, rp: CompareParaSnapshot): boolean {
  if (lp.section !== rp.section) return false;
  if (lp.controlCount !== rp.controlCount) return false;
  if (Math.abs(lp.paragraph - rp.paragraph) > REMOVED_ADDED_MAX_PARA_GAP) return false;
  if (Math.abs(lp.globalIndex - rp.globalIndex) > REMOVED_ADDED_MAX_GLOBAL_GAP) return false;
  return textSimilarity(lp.normalizedText, rp.normalizedText) >= REMOVED_ADDED_MERGE_SIM_MIN;
}
