/**
 * 비교 엔진 튜닝 상수·런타임 가드 — `diff-engine.ts` 헤더 주석의 튜닝 절을 이 모듈이 이어받았다.
 * (전체 파이프라인 설명은 `diff-engine.ts` 헤더와 `diff-engine-readme.md`)
 *
 * ── [튜닝 상수 — `// ─── 튜닝 상수` 블록]
 * - 앵커·구간: ANCHOR_*, INTRA_UNIQUE_SIG_*, SEGMENT_DP_MAX, NORM_HASH_PIN_MIN_TOTAL_PARAS, HARD_SEGMENT_CELL_LIMIT,
 *   ALIGNMENT_MAX_COMPUTE_MS, MAX_SEGMENT_RECURSION
 * - 구조 근접·비용: NEAR_STRUCTURE_*, MATCH_SOFT_SIM_MIN, MATCH_COST_WEAK, WINDOW_SIZE, GREEDY_AMBIGUOUS_GAP
 * - 컨트롤 슬롯: ALIGNMENT_CONTROL_SLOT_BONUS, ALIGNMENT_CONTROL_MIN_ADJUSTED_SCORE,
 *   TABLE_SUMMARY_MISMATCH_PENALTY, TABLE_PAIR_SIM_NO_PENALTY, TABLE_PAIR_SIM_FULL_PENALTY
 * - 후처리: PARA_SPLIT_JOIN_SIM_MIN, REMOVED_ADDED_*
 * - 문자 요약: CHAR_DIFF_FULL_DP_MAX, CHAR_DIFF_TOTAL_MAX, CHAR_DIFF_CELL_HARD
 */
import type { CompareAnchorTuning, CompareOptions, ComparePerformanceTuning } from './types';

// ═══ 튜닝·런타임 가드 ═══════════════════════════════════════════════════════
// alignment·컨트롤 슬롯 매칭의 비용/품질은 아래 상수에 강하게 묶여 있다.
// 튜닝 변경 후에는 통합 테스트용 HWP(앵커·표 삽입·밀림)으로 회귀 확인하는 것을 권장한다.
// 파일 맨 위 블록 주석(용어·파이프라인)과 이 표를 같이 읽으면 의도가 정리된다.

// ─── 튜닝 상수 (값 변경 시 대표 문서로 회귀 확인 권장) ─────────────────
// 상호 의존:
// - NEAR_STRUCTURE_* 는 `isNearStructure`가 true일 때만 DP/그리디에 반영된다.
// - 큰 밀림 직후에도 구간 베이스(`SegmentAnchorBoundary`)가 맞으면 isNear가 true가 되기 쉬워져
//   치환 비용 할인·유사도 완화가 켜진다.
// - 그래도 (L,null)(null,R) 패턴으로 남는 구간은 REMOVED_ADDED_* 로 “한 슬롯 수정”으로 승격할 수 있다.
/** alignment 경로: 오른쪽 문단마다 왼쪽에서 고를 때의 탐색 반경(문단 개수) */
export const WINDOW_SIZE = 32;
/** `buildAnchorPairs`: 시그니처가 같아도 너무 짧은 문단은 앵커 후보에서 제외(오탐 앵커 방지) */
const ANCHOR_MIN_TEXT_LEN = 20;
/** `[테스트 N: …]` 등 짧은 블록 제목을 앵커로 쓸 때 최소 글자 수(공백 제거 `trim` 기준) */
export const STRUCTURAL_ANCHOR_MIN_LEN = 5;
/** 최상위 구간 `buildUniqueSigPairsInSlices` 최소 문단 길이 */
export const INTRA_UNIQUE_SIG_MIN_TOP = 10;
/** 글로벌 앵커로 이미 자른 하위 구간에서 내부 유일 시그니처 핀의 최소 길이 */
export const INTRA_UNIQUE_SIG_MIN_NESTED = 4;
/** 동일 서명 문단 쌍의 globalIndex 차가 이 값보다 크면 "문단 이동" 메타 후보로 본다 */
export const MOVE_DISTANCE_THRESHOLD = 3;
/** 정렬이 (null,R앞)(L,R뒤)로 나온 쪼개기만: L≈R앞+R뒤일 때 R앞=변경·R뒤=추가로 재라벨 */
export const PARA_SPLIT_JOIN_SIM_MIN = 0.86;
/** 구간 내 DP 직접 적용 최대 한 변 길이 (n*m <= MAX^2) */
export const SEGMENT_DP_MAX = 150;
/** DP에서 약한 유사도끼리 붙는 것 방지 (치환 비용 하한으로 del+ins보다 불리하게) */
export const MATCH_SOFT_SIM_MIN = 0.7;
/** `matchCost`: 유사도가 너무 낮으면 치환 대신 삽입+삭제 쪽으로 유도하는 비용 상한 */
export const MATCH_COST_WEAK = 4;
/** `isNearStructure`이고 문단이 충분히 길 때, `textSimilarity` 결과를 DP/그리디 평가용으로만 끌어올리는 하한 */
export const NEAR_STRUCTURE_SIM_BOOST = 0.4;
/** 위 부스트를 받기 위한 최소 raw 유사도(무연관 짧은 문단·우연 일치 완화) */
export const NEAR_STRUCTURE_MIN_SIM = 0.12;
/** 구조 근접 시 치환 비용(`1-sim`)에서 깎는 할인 — `getEffectiveSimilarity`와 별개 튜닝 */
export const NEAR_STRUCTURE_COST_DISCOUNT = 0.4;
/** 그리디 `scorePairGreedy`: `isNearStructure`일 때 `minScore`(의역·서명 불일치) 통과용 가산 */
export const NEAR_STRUCTURE_GREEDY_BONUS = 1.5;
/** 그리디: 1·2위 점수 차가 이 값 미만이면 동률로 보고 매칭 포기(1위가 구조 근접이면 검사 생략) */
export const GREEDY_AMBIGUOUS_GAP = 0.35;
/** (L,null)(null,R) 등 삭제+추가로만 남았지만 동일 슬롯 수정으로 승격할 최소 textSimilarity */
export const REMOVED_ADDED_MERGE_SIM_MIN = 0.28;
/** 위 승격: 같은 구역에서 허용하는 문단 번호·globalIndex 최대 간격(위쪽 삽입으로 밀린 경우) */
export const REMOVED_ADDED_MAX_PARA_GAP = 12;
export const REMOVED_ADDED_MAX_GLOBAL_GAP = 24;
/** 문자 diff: 이 이하의 n×m만 전역 DP 역추적(Hirschberg 잎) */
export const CHAR_DIFF_FULL_DP_MAX = 280_000;
/** 좌+우 길이 합 상한 — 그 이상은 요약 생략(메인 스레드 보호) */
export const CHAR_DIFF_TOTAL_MAX = 96_000;
/** n×m 셀 상한 — 평균 케이스에서도 과도한 O(nm) 방지 */
export const CHAR_DIFF_CELL_HARD = 14_000_000;
/** 앵커 품질 기본 가드레일: 공백 비율 상한 */
const ANCHOR_MAX_WHITESPACE_RATIO = 0.62;
/** 앵커 품질 기본 가드레일: 최소 고유 문자 수 */
const ANCHOR_MIN_UNIQUE_CHARS = 6;
/** 앵커 품질 기본 가드레일: 최소 엔트로피 */
const ANCHOR_MIN_ENTROPY = 1.9;
/** 브라우저 프리징 방지: 구간 셀 수 하드캡(초과 시 DP 금지) */
const HARD_SEGMENT_CELL_LIMIT = 180_000;
/** 브라우저 프리징 방지: alignment 타임버짓 기본값(ms) */
const ALIGNMENT_MAX_COMPUTE_MS = 2600;
/** `matchSegment` 재귀(내부 앵커·half-match) 최대 깊이 */
export const MAX_SEGMENT_RECURSION = 96;
/** half-match 전수 탐색 시 n×m 상한(초과 시 half-match 생략) */
export const HALF_MATCH_MAX_PRODUCT = 450_000;
/**
 * `tryMatchByNormHashPins`: 서명 유일 핀이 없을 때 `normalizedText` trim을 해시 토큰으로 바꿔
 * 구간 내 유일 쌍을 찾는 **거시** 단계. 좌·우 문단 수 합이 이 값 미만이면 오버헤드만 커져 생략한다.
 */
export const NORM_HASH_PIN_MIN_TOTAL_PARAS = 48;
/**
 * `pairAlignmentSlotControls`: 문단 정렬로 이미 “같은 논리 문단”으로 묶인 표·도형에 대해
 * `scoreControlFallback` 원점수에 더하는 보너스. 위쪽 삽입으로 앵커 y가 크게 달라져도
 * 원점수가 2.75 근처에서 막히는 현상을 완화한다.
 */
export const ALIGNMENT_CONTROL_SLOT_BONUS = 2.35;
/**
 * 슬롯 단계에서 채택하는 최소 점수 = `scoreControlFallback(l,r) + ALIGNMENT_CONTROL_SLOT_BONUS`.
 * 너무 낮추면 다른 문단 개체와 오매칭, 너무 높이면 슬롯이 거의 동작하지 않는다.
 */
export const ALIGNMENT_CONTROL_MIN_ADJUSTED_SCORE = 4.38;
/**
 * `scoreControlFallback`: 표 요약(`summary`)이 다를 때 부과하는 **최대** 감점.
 * `tableControlPairContentSimilarity`가 높으면(같은 행·열 그리드에서 셀만 수정) 감점 비율을 0에 가깝게 줄여
 * `buildGranularControlDiffs`로 “표 수정”이 나가게 한다. 유사도가 낮으면(다른 표) 전액 감점한다.
 */
export const TABLE_SUMMARY_MISMATCH_PENALTY = 4.25;
/** 이 이상이면 표 요약 불일치 감점을 적용하지 않는다(한 셀 수정 등). */
export const TABLE_PAIR_SIM_NO_PENALTY = 0.74;
/** 이 이하이면 표 감점을 전액 적용한다(내용이 다른 표). */
export const TABLE_PAIR_SIM_FULL_PENALTY = 0.36;

/** `compareSnapshots` 한 번 호출 동안만 유효. `ALIGNMENT_MAX_COMPUTE_MS` 경과 시 greedy 쪽으로 이탈한다. */
export type CompareRuntimeGuard = {
  deadline: number;
  bailedOut: boolean;
};

/**
 * `isNearStructure`가 쓰는 좌·우 `globalIndex` 베이스(한 쌍의 절대 인덱스).
 * `matchSegment` 재귀 전체에서 동일 베이스를 쓰면, “구간 전체가 위에서 k칸 밀렸다”를
 * `dL = lp.globalIndex - leftBaseGi`, `dR = rp.globalIndex - rightBaseGi`로 보고 `|dL-dR|≤2`만 검사한다.
 */
export type SegmentStructBase = { leftBaseGi: number; rightBaseGi: number };
/**
 * 글로벌 앵커 한 쌍의 globalIndex. `buildTextDiffs`가 앵커 사이 슬라이스를 `matchSegment`에 넘길 때
 * 직전 경계 앵커 `(a.li,a.ri)`가 있으면 그 문단의 인덱스를 베이스로 넣고, 문서 맨 앞 구간은 null이라
 * 베이스가 구간의 **첫 문단** 쌍으로 대체된다(`matchSegment` 내부에서 산출).
 */
export type SegmentAnchorBoundary = { leftAnchorGi: number; rightAnchorGi: number };
/**
 * 한 번의 `compareSnapshots` 실행을 관통하는 명시적 컨텍스트.
 * 예전 모듈 전역(`activeRuntimeGuard`·`activeSegmentStructBase`·`activeCompareOptions`)을 대체하며,
 * 공개 진입점(`diff-engine.ts`의 `compareSnapshots`)이 생성해 내부 호출 사슬의 첫 인자로 전달한다.
 */
export interface CompareContext {
  /** compareSnapshots 실행 중 `matchCost`/`resolveAnchorTuning` 등이 읽는 옵션(스레드 안전은 요구하지 않음) */
  options: CompareOptions;
  /** `compareSnapshots` 한 번 호출 동안만 유효. */
  runtimeGuard: CompareRuntimeGuard | null;
  /** `matchSegment`/`matchSegmentDp`/`matchCost` 호출 동안만 세팅. 중첩 재귀 시 스택처럼 복구한다. */
  segmentStructBase: SegmentStructBase | null;
}

/** `fn` 실행 동안 `isNearStructure`가 `base` 기준 상대 오프셋을 보도록 한다. */
export function runWithSegmentStructBase<T>(
  ctx: CompareContext,
  base: SegmentStructBase,
  fn: () => T,
): T {
  const prev = ctx.segmentStructBase;
  ctx.segmentStructBase = base;
  try {
    return fn();
  } finally {
    ctx.segmentStructBase = prev;
  }
}

/** `CompareOptions.anchorTuning`이 있으면 그걸 쓰고, 없으면 파일 상단 기본 앵커 가드레일을 쓴다. */
export function resolveAnchorTuning(options: CompareOptions): Required<CompareAnchorTuning> {
  return {
    minTextLen: options.anchorTuning?.minTextLen ?? ANCHOR_MIN_TEXT_LEN,
    minUniqueChars: options.anchorTuning?.minUniqueChars ?? ANCHOR_MIN_UNIQUE_CHARS,
    maxWhitespaceRatio: options.anchorTuning?.maxWhitespaceRatio ?? ANCHOR_MAX_WHITESPACE_RATIO,
    minEntropy: options.anchorTuning?.minEntropy ?? ANCHOR_MIN_ENTROPY,
  };
}

/** `hardSegmentCells` / `maxComputeMs` — 큰 문서에서 DP 테이블·이중 루프가 메인 스레드를 막지 않게 상한을 둔다. */
export function resolvePerformanceTuning(options: CompareOptions): Required<ComparePerformanceTuning> {
  return {
    maxComputeMs: options.performanceTuning?.maxComputeMs ?? ALIGNMENT_MAX_COMPUTE_MS,
    hardSegmentCells: options.performanceTuning?.hardSegmentCells ?? HARD_SEGMENT_CELL_LIMIT,
  };
}

function shannonEntropy(text: string): number {
  if (!text) return 0;
  const freq = new Map<string, number>();
  for (const ch of text) freq.set(ch, (freq.get(ch) ?? 0) + 1);
  let entropy = 0;
  for (const c of freq.values()) {
    const p = c / text.length;
    entropy -= p * Math.log2(p);
  }
  return entropy;
}

/**
 * 글로벌 앵커 후보 문단의 "텍스트 품질" 검사(엔트로피·공백 비율·고유 문자 수).
 *
 * 너무 짧거나 반복적인 줄을 앵커로 쓰면 뒤따르는 모든 구간 정렬이 한 번에 틀어질 수 있어 기본은 배제한다.
 * 다만 `[테스트 N: …]`·`1. …` 같이 **구조적 한 줄 제목**으로 보이는 패턴(`isStructuralBlockTitleLine`)은
 * 길이가 `STRUCTURAL_ANCHOR_MIN_LEN` 이상·`minTextLen` 미만이면 엔트로피 검사 없이 후보로 통과시킨다.
 * 짧은 줄의 **전역 유일성**(양쪽 문서에서 trim 본문이 각각 한 번)은 `buildAnchorPairs`에서 별도로 걸러진다.
 */
export function isAnchorTextQualityOk(text: string, tuning: Required<CompareAnchorTuning>): boolean {
  const trimmed = text.trim();
  if (
    trimmed.length >= STRUCTURAL_ANCHOR_MIN_LEN &&
    trimmed.length < tuning.minTextLen &&
    isStructuralBlockTitleLine(trimmed)
  ) {
    return true;
  }
  if (text.length < tuning.minTextLen) return false;
  const whitespaceCount = (text.match(/\s/g) ?? []).length;
  if (whitespaceCount / Math.max(1, text.length) > tuning.maxWhitespaceRatio) return false;
  if (new Set(text).size < tuning.minUniqueChars) return false;
  return shannonEntropy(text) >= tuning.minEntropy;
}

/**
 * `[테스트 N: …]`·`[제목]`·`1. 소제목` 같이 짧아도 블록 경계로 쓸 만한 한 줄 제목.
 * 글로벌 앵커 후보는 `signature` 중복 여부로 별도 차단한다.
 */
export function isStructuralBlockTitleLine(trimmedOneLine: string): boolean {
  if (!trimmedOneLine) return false;
  if (/^\[[^\]]+\]$/.test(trimmedOneLine)) return true;
  if (/^\[[^\]]+:[^\]]+\]$/.test(trimmedOneLine)) return true;
  if (/^\d+\.\s+/.test(trimmedOneLine)) return true;
  return false;
}

/**
 * alignment 중 시간이 `maxComputeMs`를 넘기면 true로 고정되며, 이후 DP 진입을 막고 그리디로만 처리한다.
 * DP 이중 루프 안에서는 `i % 12` 등으로 가끔만 호출해 오버헤드를 제한한다.
 */
export function shouldBailToGreedy(ctx: CompareContext): boolean {
  if (!ctx.runtimeGuard) return false;
  if (ctx.runtimeGuard.bailedOut) return true;
  if (Date.now() <= ctx.runtimeGuard.deadline) return false;
  ctx.runtimeGuard.bailedOut = true;
  return true;
}
