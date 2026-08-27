/**
 * HWP 문서 비교 엔진 — 문단 정렬(alignment), 문자 단위 diff, 표·도형·그림 등 컨트롤 diff를 한 모듈에서 처리한다.
 *
 * ── [용어] “밀림(shift)”
 * 한쪽에 문단·표가 삽입되면 이후 문단의 `section`/`paragraph`/`globalIndex`가 통째로 밀린다.
 * 텍스트는 내용 기반 정렬로 상쇄할 수 있으나, 컨트롤 키에 물리 위치가 남으면 표/도형이 “삭제+추가”로
 * 쪼개져 보이는 문제가 생긴다. 이 파일은 (A) 문단 쪽 밀림 완화 (B) 정렬 결과를 컨트롤에 전달 두 축으로 대응한다.
 *
 * ── [도메인 분리: identity vs alignment]
 * - **이력(같은 혈통)**: `stable_id`가 양쪽에 공유되면 `identity` 경로로 문단 1:1을 직접 잡는다(O(N) 수준).
 * - **외부 문서(다른 혈통)**: 공유 sid가 없으므로 `alignment`만 사용한다. 유일·긴 문단과 섹션 제목류를
 *   **글로벌 앵커**로 써 문서를 구간으로 쪼개고, 구간 안은 DP/그리디로 채운다.
 *
 * ── [문단 밀림 완화 — alignment 내부 계층]
 * 1. **글로벌 앵커** (`buildAnchorPairs`, `fillSnapshotFromWasm`의 `isAnchorCandidate`)
 *    - 시그니처가 좌·우 각각 한 번만 등장하는 문단만 앵커 후보.
 *    - 길이·엔트로피 등 `isAnchorTextQualityOk`로 짧은 반복 문장을 걸러 오염 앵커를 막는다.
 *    - `[테스트 N: …]` 같은 짧은 제목은 `isStructuralBlockTitleLine` 등으로 후보로 올리되,
 *      `buildBothSidesUniqueTrimNorm`으로 **trim 본문이 양쪽 문서에서 정확히 한 번씩**일 때만 글로벌 앵커로 승인.
 * 2. **구간 정렬** (`matchSegment` 재귀 트리)
 *    - `buildUniqueSigPairsInSlices`: 구간 안에서만 시그니처 빈도를 세 **내부 Patience 핀**으로 쪼갠다.
 *    - `tryMatchByNormHashPins`: 서명 핀이 없을 때 `normalizedText` 해시 토큰으로 거시 핀(DMP line-mode 유사).
 *    - `matchSegmentDp` / `findLongestEqualSignatureRun` / `matchWindowedGreedy`: 셀 수·시간 한도에 따라 선택.
 * 3. **상대 구조 근접** (`SegmentAnchorBoundary` + `runWithSegmentStructBase` + `isNearStructure`)
 *    - 구간마다 직전 글로벌 앵커 쌍의 `globalIndex`를 베이스로 잡고, `|Δleft - Δright| ≤ 2`로 본다.
 *    - 문서 맨 앞 구간은 베이스가 구간 **첫 문단**이다. 절대 인덱스만 보면 큰 밀림 직후 이웃 문단이
 *      구조적으로 가깝다고 인정받지 못하는 문제를 줄인다.
 * 4. **DP 비용** (`matchCost`, `getEffectiveSimilarity`, `softSimilarityThresholdForPair`)
 *    - 유사도는 “전역 라벨”이 아니라 치환 비용·약매칭 방지용 가중치로만 쓴다.
 *
 * ── [컨트롤 밀림 완화]
 * - 1차: `kind::key` 정확 일치(`sid:…` 우선).
 * - 2차: **`buildRightToLeftParaMapFromAligned`** — `buildTextDiffs`가 만든 문단 정렬 `AlignedPair[]`에서
 *   오른쪽 `section:paragraph` → 짝 왼쪽 문단 맵을 구축해 `buildControlDiffs`에 전달한다.
 * - 3차: **`extractTablePatiencePins`** — 요약 키가 양쪽에서 각각 유일한 표를 슬롯보다 먼저 고정한다.
 * - 4차: **`pairAlignmentSlotControls`** — 남은 컨트롤에 대해, 오른쪽 부모 문단의 맵상 짝 왼쪽 문단에 붙은
 *   미매칭 컨트롤만 후보로 `scoreControlFallback + ALIGNMENT_CONTROL_SLOT_BONUS`로 짝을 찾는다.
 * - 5차: `pairControlsFallback`(전역 탐욕, 임계 2.75).
 *
 * ── [단계적 비교(성능)]
 * - 전 문단×문단 유사도 행렬 같은 O(N²) 전역 최적화는 하지 않는다.
 * - 짝이 맞은 문단에만 `myersCharDiffSummary`(접두·접미 제거, Hirschberg, `CHAR_DIFF_*` 상한)를 적용한다.
 *
 * ── [튜닝 상수 — `// ─── 튜닝 상수` 블록]
 * - 앵커·구간: ANCHOR_*, INTRA_UNIQUE_SIG_*, SEGMENT_DP_MAX, NORM_HASH_PIN_MIN_TOTAL_PARAS, HARD_SEGMENT_CELL_LIMIT,
 *   ALIGNMENT_MAX_COMPUTE_MS, MAX_SEGMENT_RECURSION
 * - 구조 근접·비용: NEAR_STRUCTURE_*, MATCH_SOFT_SIM_MIN, MATCH_COST_WEAK, WINDOW_SIZE, GREEDY_AMBIGUOUS_GAP
 * - 컨트롤 슬롯: ALIGNMENT_CONTROL_SLOT_BONUS, ALIGNMENT_CONTROL_MIN_ADJUSTED_SCORE,
 *   TABLE_SUMMARY_MISMATCH_PENALTY, TABLE_PAIR_SIM_NO_PENALTY, TABLE_PAIR_SIM_FULL_PENALTY
 * - 후처리: PARA_SPLIT_JOIN_SIM_MIN, REMOVED_ADDED_*
 * - 문자 요약: CHAR_DIFF_FULL_DP_MAX, CHAR_DIFF_TOTAL_MAX, CHAR_DIFF_CELL_HARD
 *
 * ── [입력 경로]
 * - `buildSnapshotFromBytes`: 외부 파일 비교.
 * - `buildSnapshotFromWasm`: 편집기 IR — 같은 세션 이력에서 sid 보존에 유리.
 *
 * ── [공개 진입점]
 * - `compareSnapshots`: 전략 선택 → 본문 diff → 컨트롤 diff(맵 전달) → 필터·쪽번호·정렬.
 * - `compareDocuments`: bytes 두 개를 스냅샷으로 만든 뒤 위와 동일.
 *
 * ── [유지보수]
 * - 품질 이슈 시 compare-debug 로그 ① stable_id ② 전략 ③ 앵커를 함께 본다.
 * - 엔진 본체는 아래 모듈 맵처럼 관심사별 파일로 나뉘어 있고, 각 모듈이 `// ───` 섹션 구분으로 점프 검색을 지원한다.
 */
import { compareDbg, isCompareDebugEnabled } from './compare-debug';
import { resolvePerformanceTuning } from './tuning';
import type { CompareContext } from './tuning';
import { buildSnapshotFromBytes, buildSnapshotFromWasm, buildStableIdMap } from './snapshot';
import { buildIdentityTextDiffs, resolveTextCompareStrategy, suppressPureReflowMoves } from './identity';
import { buildTextDiffs } from './align-assembly';
import { annotateDiffSectionPages, buildControlDiffs } from './control-diff';
import type {
  CompareDocumentSnapshot,
  CompareOptions,
  CompareParaSnapshot,
  CompareSession,
  CompareStrategy,
} from './types';

// ═══ 모듈 맵 (관심사별 분할 — 각 모듈 안에 `// ───` 섹션 구분이 있다) ═════════
// tuning.ts          튜닝 상수·CompareContext·런타임 가드·앵커 품질 기준
// char-diff.ts       문자 단위 diff 요약(Levenshtein·Hirschberg)
// signature.ts       정규화·해시·DiffID·컨트롤 키·표 요약 파싱·정밀 컨트롤 diff
// snapshot.ts        WASM → CompareDocumentSnapshot 수집·빌더·노이즈 억제
// identity.ts        stable_id 1:1 경로·전략 선택·reflow 이동 제거
// align-core.ts      유사도·구조·비용 체인 + DP/그리디 문단 정렬기
// align-assembly.ts  앵커 → 구간 정렬 → 스텝 스트림 → cleanup → buildTextDiffs
// control-diff.ts    컨트롤 diff(키 매칭 → 표 patience 핀 → 정렬 슬롯 → 폴백)·쪽번호 주석
// 이 파일            공개 진입점(compareSnapshots·compareDocuments) + 재수출 인덱스

export { buildSnapshotFromBytes, buildSnapshotFromWasm };

// ─── 공개 진입점: 스냅샷 비교 세션 생성 ───────────────────────────────────────

/**
 * 이미 파싱된 스냅샷 두 개를 비교해 `CompareSession`을 만든다.
 *
 * 흐름:
 * - `resolveTextCompareStrategy`: 공유 stable_id가 신뢰되면 `identity`, 아니면 `alignment`.
 * - **본문**: `buildIdentityTextDiffs` 또는 `buildTextDiffs`(후자는 `{ diffs, rightToLeftPara }`).
 * - **컨트롤**: `buildControlDiffs(left, right, rightToLeftPara)` — 문단 밀림 보정을 위해 alignment 맵 전달.
 * - **후처리**: `options.kinds` 필터 → `suppressPureReflowMoves` → `annotateDiffSectionPages` →
 *   구역·문단 순 정렬.
 *
 * 성능: `ctx.runtimeGuard`로 wall-clock 상한. 초과 시 `matchSegment` 쪽이 그리디 위주로 이탈할 수 있다.
 */
export function compareSnapshots(
  left: CompareDocumentSnapshot,
  right: CompareDocumentSnapshot,
  options: CompareOptions,
): CompareSession {
  const strategy: CompareStrategy = options.strategy ?? 'alignment';
  const perf = resolvePerformanceTuning(options);
  // 예전 모듈 전역(activeCompareOptions·activeRuntimeGuard·activeSegmentStructBase)을
  // 명시적 컨텍스트로 치환했다. 세팅·해제 타이밍은 기존 try/finally와 같은 의미로,
  // 이 호출 한 번의 생애주기가 곧 상태의 생애주기다(호출 밖으로 새어나가는 잔상 없음).
  const ctx: CompareContext = {
    options,
    runtimeGuard: {
      deadline: Date.now() + perf.maxComputeMs,
      bailedOut: false,
    },
    segmentStructBase: null,
  };
  const textMode = resolveTextCompareStrategy(strategy, left, right);

  // compare-debug 켜진 빌드에서만: stable_id 품질(①) → 전략(②) → alignment는 ③ 앵커 로그 참고
  if (isCompareDebugEnabled()) {
    const lmap = buildStableIdMap(left);
    const rmap = buildStableIdMap(right);
    let sharedStable = 0;
    if (lmap && rmap) {
      for (const id of lmap.keys()) {
        if (rmap.has(id)) sharedStable += 1;
      }
    }
    compareDbg('[① stable_id] 스냅샷 요약', {
      left: left.meta.name,
      right: right.meta.name,
      leftParas: left.paragraphs.length,
      rightParas: right.paragraphs.length,
      leftHead: left.paragraphs.slice(0, 10).map((p) => ({
        sec: p.section,
        para: p.paragraph,
        id: p.stableId ? `${p.stableId.slice(0, 14)}…` : '(빈)',
        t: p.text.slice(0, 32),
      })),
      rightHead: right.paragraphs.slice(0, 10).map((p) => ({
        sec: p.section,
        para: p.paragraph,
        id: p.stableId ? `${p.stableId.slice(0, 14)}…` : '(빈)',
        t: p.text.slice(0, 32),
      })),
    });
    compareDbg('[② 전략·폴백]', {
      optionsStrategy: strategy,
      textMode,
      mapsBuildOk: Boolean(lmap && rmap),
      sharedStableIdCount: lmap && rmap ? sharedStable : null,
      path:
        textMode === 'identity'
          ? 'buildIdentityTextDiffs (Map<stableId>, 인덱스 1:1 아님)'
          : 'buildTextDiffs (앵커 + 구간 DP/그리디 — ③ 로그 참고)',
    });
  }

  // 1) 본문: identity면 sid 집합 비교, 아니면 앵커+구간 정렬
  const textBundle =
    textMode === 'identity'
      ? { diffs: buildIdentityTextDiffs(left, right, options.kinds), rightToLeftPara: new Map<string, CompareParaSnapshot>() }
      : buildTextDiffs(ctx, left, right);
  const textDiffs = textBundle.diffs;

  // 2) 개체(표/도형 등) 병합 — 문단 정렬 맵으로 밀린 문단 좌표의 표·그림을 같은 슬롯에서 재짝짓기
  const all = [...textDiffs, ...buildControlDiffs(left, right, textBundle.rightToLeftPara)];

  // 3) kinds 필터 + 순수 리플로우 이동 노이즈 제거 후, UI용 쪽번호 주석
  const filtered = suppressPureReflowMoves(
    all.filter((d) => options.kinds.includes(d.kind)),
    left,
    right,
  );
  annotateDiffSectionPages(filtered, left, right);
  if (isCompareDebugEnabled() && ctx.runtimeGuard?.bailedOut) {
    compareDbg('[성능 가드레일] 타임버짓 초과로 일부 구간을 greedy/fallback으로 처리했습니다.');
  }
  // 목록 정렬: 구역 → 문단 순으로 탐색하기 쉽게
  filtered.sort((a, b) => {
    const sa = a.path.section ?? 0;
    const sb = b.path.section ?? 0;
    if (sa !== sb) return sa - sb;
    const pa = a.path.paragraph ?? 0;
    const pb = b.path.paragraph ?? 0;
    return pa - pb;
  });

  return {
    left: left.meta,
    right: right.meta,
    options,
    diffItems: filtered,
    currentDiffIndex: filtered.length > 0 ? 0 : -1,
    generatedAt: Date.now(),
    textCompareStrategyUsed: textMode,
  };
}

/**
 * 외부 두 파일(bytes): 각각 별도 WASM으로 스냅샷을 만든 뒤 `compareSnapshots`와 동일 파이프라인.
 * 좌·우 stable_id 집합이 보통 겹치지 않아 alignment가 기본이 된다.
 */
export async function compareDocuments(
  leftBytes: Uint8Array,
  leftName: string,
  rightBytes: Uint8Array,
  rightName: string,
  options: CompareOptions,
): Promise<CompareSession> {
  const left = await buildSnapshotFromBytes(leftBytes, leftName, options);
  const right = await buildSnapshotFromBytes(rightBytes, rightName, options);
  return compareSnapshots(left, right, options);
}
