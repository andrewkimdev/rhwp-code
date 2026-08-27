/** identity 경로 — 같은 혈통(공유 stable_id) 문서의 O(N) 텍스트 diff, 전략 선택, 순수 reflow 이동 노이즈 제거. */
import { myersCharDiffSummary } from './char-diff';
import { preferRightPath } from './align-core';
import { mkDiffId } from './signature';
import { buildStableIdMap, shouldSuppressNoiseParagraphOnly } from './snapshot';
import { MOVE_DISTANCE_THRESHOLD } from './tuning';
import type {
  CompareDocumentSnapshot,
  CompareParaSnapshot,
  CompareStrategy,
  DiffItem,
  DiffKind,
} from './types';

// ─── identity 경로: 이력(동일 혈통)에서 stable_id 기준 O(N) 근사 텍스트 diff ───

/**
 * identity 텍스트 비교: stable_id 정규화 키로 좌/우 문단을 1:1 매칭.
 * - 양쪽 중 한쪽에만 있으면 added/removed
 * - 둘 다 있으면 normalizedText 불일치 시 modified + 문자 요약
 * - kinds에 paragraphMeta가 있으면 이동/컨트롤 수 변화도 별도 항목으로 낸다
 */
export function buildIdentityTextDiffs(left: CompareDocumentSnapshot, right: CompareDocumentSnapshot, kinds: DiffKind[]): DiffItem[] {
  const lmap = buildStableIdMap(left);
  const rmap = buildStableIdMap(right);
  if (!lmap || !rmap) return [];

  // 양쪽 Map의 key(stableId)를 합집합으로 만든다.
  // -> 어떤 id든 최소 1회는 순회되므로 "한쪽만 존재(추가/삭제)"도 놓치지 않는다.
  const keys = [...new Set([...lmap.keys(), ...rmap.keys()])];
  keys.sort((a, b) => {
    const la = lmap.get(a);
    const lb = lmap.get(b);
    const ra = rmap.get(a);
    const rb = rmap.get(b);
    const sa = la?.section ?? ra?.section ?? 0;
    const sb = lb?.section ?? rb?.section ?? 0;
    if (sa !== sb) return sa - sb;
    const pa = la?.paragraph ?? ra?.paragraph ?? 0;
    const pb = lb?.paragraph ?? rb?.paragraph ?? 0;
    return pa - pb;
  });

  const diffs: DiffItem[] = [];
  for (const id of keys) {
    // 동일 id를 좌/우 Map에서 각각 조회:
    // - l만 있으면: 기준 문서에는 있었는데 비교 문서에는 없음 => removed
    // - r만 있으면: 비교 문서에 새로 생김 => added
    // - 둘 다 있으면: 내용(normalizedText) 비교로 modified 여부 판단
    // 조회 자체는 Map#get 이라 평균 O(1), 전체는 key 개수에 비례해 O(N)으로 동작한다.
    const l = lmap.get(id);
    const r = rmap.get(id);
    if (l && !r) {
      if (!shouldSuppressNoiseParagraphOnly(l)) {
        diffs.push({
          id: mkDiffId('text', `id-removed:${id}`),
          kind: 'text',
          severity: 'removed',
          path: { section: l.section, paragraph: l.paragraph },
          title: '문단 삭제',
          leftPreview: l.text,
          rightPreview: '',
          leftAnchor: l.anchor,
        });
      }
      continue;
    }
    if (!l && r) {
      if (!shouldSuppressNoiseParagraphOnly(r)) {
        diffs.push({
          id: mkDiffId('text', `id-added:${id}`),
          kind: 'text',
          severity: 'added',
          path: { section: r.section, paragraph: r.paragraph },
          title: '문단 추가',
          leftPreview: '',
          rightPreview: r.text,
          rightAnchor: r.anchor,
        });
      }
      continue;
    }
    if (!l || !r) continue;

    if (l.normalizedText !== r.normalizedText) {
      diffs.push({
        id: mkDiffId('text', `id-modified:${id}`),
        kind: 'text',
        severity: 'modified',
        path: preferRightPath(l, r),
        title: '텍스트 변경',
        leftPreview: l.text,
        rightPreview: r.text,
        leftAnchor: l.anchor,
        rightAnchor: r.anchor,
        inlineTextDiff: myersCharDiffSummary(l.text, r.text),
      });
    }

    if (
      kinds.includes('paragraphMeta') &&
      l.signature === r.signature &&
      Math.abs(l.globalIndex - r.globalIndex) > MOVE_DISTANCE_THRESHOLD
    ) {
      diffs.push({
        id: mkDiffId('paragraphMeta', `id-moved:${id}`),
        kind: 'paragraphMeta',
        severity: 'modified',
        path: preferRightPath(l, r),
        title: '문단 순서 이동',
        leftPreview: `A idx=${l.globalIndex}`,
        rightPreview: `B idx=${r.globalIndex}`,
        leftAnchor: l.anchor,
        rightAnchor: r.anchor,
      });
    }

    if (kinds.includes('paragraphMeta') && l.controlCount !== r.controlCount) {
      diffs.push({
        id: mkDiffId('paragraphMeta', `id-ctrlcount:${id}`),
        kind: 'paragraphMeta',
        severity: 'modified',
        path: preferRightPath(l, r),
        title: '문단 개체 수 변경',
        leftPreview: `controls=${l.controlCount}`,
        rightPreview: `controls=${r.controlCount}`,
        leftAnchor: l.anchor,
        rightAnchor: r.anchor,
      });
    }
  }
  return diffs;
}

// ─── 전략 선택·순수 reflow 에 대한 moved 메타 제거 ───────────────────────────

/**
 * 호출부 `options.strategy`와 실제 가능 여부를 교차 검사한다.
 * `identity`를 요청해도 양쪽 `buildStableIdMap`이 null이면 alignment로 내려가 변동성이 커질 수 있다.
 */
export function resolveTextCompareStrategy(
  strategy: CompareStrategy,
  left: CompareDocumentSnapshot,
  right: CompareDocumentSnapshot,
): 'identity' | 'alignment' {
  const canId = Boolean(buildStableIdMap(left) && buildStableIdMap(right));
  if (strategy === 'identity') return canId ? 'identity' : 'alignment';
  return 'alignment';
}

/** `suppressPureReflowMoves`가 제거 대상으로 삼는 paragraphMeta 이동 항목만 true */
function isParagraphMoveMeta(diff: DiffItem): boolean {
  return diff.kind === 'paragraphMeta' && (diff.id.includes('moved:') || diff.id.includes('id-moved:'));
}

/**
 * 삽입/삭제로 globalIndex만 밀린 것과 "진짜 순서 바뀜"을 구분해 moved 노이즈를 제거한다.
 * (공유 sid의 상대 순서가 유지되고, 밀림량이 앞쪽 순수 추가/삭제 개수와 일치하면 제외)
 */
export function suppressPureReflowMoves(
  diffs: DiffItem[],
  left: CompareDocumentSnapshot,
  right: CompareDocumentSnapshot,
): DiffItem[] {
  const lmap = buildStableIdMap(left);
  const rmap = buildStableIdMap(right);
  if (!lmap || !rmap) return diffs;

  const sharedLeftIds = left.paragraphs.map((p) => p.stableId).filter((id) => id && rmap.has(id));
  const sharedRightIds = right.paragraphs.map((p) => p.stableId).filter((id) => id && lmap.has(id));
  const rankLeft = new Map<string, number>();
  const rankRight = new Map<string, number>();
  sharedLeftIds.forEach((id, i) => rankLeft.set(id, i));
  sharedRightIds.forEach((id, i) => rankRight.set(id, i));

  const rightOnlyPrefixCount: number[] = Array(right.paragraphs.length + 1).fill(0);
  for (let i = 0; i < right.paragraphs.length; i += 1) {
    rightOnlyPrefixCount[i + 1] = rightOnlyPrefixCount[i] + (lmap.has(right.paragraphs[i].stableId) ? 0 : 1);
  }
  const leftOnlyPrefixCount: number[] = Array(left.paragraphs.length + 1).fill(0);
  for (let i = 0; i < left.paragraphs.length; i += 1) {
    leftOnlyPrefixCount[i + 1] = leftOnlyPrefixCount[i] + (rmap.has(left.paragraphs[i].stableId) ? 0 : 1);
  }

  return diffs.filter((d) => {
    if (!isParagraphMoveMeta(d)) return true;
    let l: CompareParaSnapshot | undefined;
    let r: CompareParaSnapshot | undefined;

    // identity 경로: id는 "paragraphMeta:id-moved:<stableId>"
    const sid = d.id.includes('id-moved:') ? d.id.split('id-moved:')[1] : '';
    if (sid) {
      l = lmap.get(sid);
      r = rmap.get(sid);
    } else {
      // alignment 경로: path가 좌측 기준으로 내려오는 moved:<sec>:<para>
      l = left.paragraphs.find((p) => p.section === d.path.section && p.paragraph === d.path.paragraph);
      r = l ? rmap.get(l.stableId) : undefined;
    }
    if (!l || !r) return true;

    const delta = r.globalIndex - l.globalIndex;
    if (delta === 0) return false;

    const sameRelativeOrder = rankLeft.get(l.stableId) === rankRight.get(l.stableId);
    if (!sameRelativeOrder) return true;

    if (delta > 0) {
      const addedBefore = rightOnlyPrefixCount[r.globalIndex];
      if (delta === addedBefore) return false;
    } else {
      const removedBefore = leftOnlyPrefixCount[l.globalIndex];
      if (-delta === removedBefore) return false;
    }
    return true;
  });
}
