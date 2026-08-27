/** 컨트롤 diff — 키 정확 매칭 → 표 patience 핀 → 정렬 슬롯 → 전역 폴백 → added/removed, 그리고 쪽번호 주석. */
import { textSimilarity } from './align-core';
import type { ControlPair } from './align-core';
import {
  buildGranularControlDiffs,
  extractControlIndexFromKey,
  kindLabel,
  mkDiffId,
  paraPosKey,
  parseSummaryKV,
  simpleHash,
} from './signature';
import {
  ALIGNMENT_CONTROL_MIN_ADJUSTED_SCORE,
  ALIGNMENT_CONTROL_SLOT_BONUS,
  TABLE_PAIR_SIM_FULL_PENALTY,
  TABLE_PAIR_SIM_NO_PENALTY,
  TABLE_SUMMARY_MISMATCH_PENALTY,
} from './tuning';
import type {
  CompareControlSnapshot,
  CompareDocumentSnapshot,
  CompareOptions,
  CompareParaSnapshot,
  DiffItem,
} from './types';

/**
 * 키가 달라진 뒤에도 **표(table)** 만 모아, 요약 문자열에서 뽑은 키(`txt` / `sig` / 해시) 기준으로
 * 양쪽 문서에서 각각 **정확히 한 개**뿐인 표끼리 1:1 고정한다(Git Patience 유사).
 *
 * `buildControlDiffs`에서는 **정렬 슬롯보다 먼저** 호출한다. 삽입으로 키가 어긋져도 요약이 유일하게 대응되면
 * 같은 내용의 표가 먼저 짝지어져, 슬롯이 위치만 보고 엉뚱한 표를 가져가는 일을 줄인다.
 * 이 단계는 표 요약이 충분히 구별될 때만 효과가 있다.
 */
function extractTablePatiencePins(
  left: CompareControlSnapshot[],
  right: CompareControlSnapshot[],
): { pins: ControlPair[]; restLeft: CompareControlSnapshot[]; restRight: CompareControlSnapshot[] } {
  const pins: ControlPair[] = [];
  const lTables = left.filter((c) => c.type === 'table');
  const rTables = right.filter((c) => c.type === 'table');
  if (lTables.length === 0 || rTables.length === 0) {
    return { pins: [], restLeft: left, restRight: right };
  }
  const keyOf = (c: CompareControlSnapshot): string => {
    const kv = parseSummaryKV(c.summary);
    const t = (kv.txt ?? '').trim();
    if (t) return `t:${t}`;
    const sg = (kv.sig ?? '').trim();
    if (sg) return `s:${sg}`;
    return `h:${simpleHash(c.summary)}`;
  };
  const lByKey = new Map<string, CompareControlSnapshot[]>();
  const rByKey = new Map<string, CompareControlSnapshot[]>();
  for (const c of lTables) {
    const k = keyOf(c);
    const arr = lByKey.get(k);
    if (arr) arr.push(c);
    else lByKey.set(k, [c]);
  }
  for (const c of rTables) {
    const k = keyOf(c);
    const arr = rByKey.get(k);
    if (arr) arr.push(c);
    else rByKey.set(k, [c]);
  }
  const usedL = new Set<CompareControlSnapshot>();
  const usedR = new Set<CompareControlSnapshot>();
  for (const [k, ls] of lByKey) {
    const rs = rByKey.get(k);
    if (!rs || ls.length !== 1 || rs.length !== 1) continue;
    pins.push({ left: ls[0], right: rs[0] });
    usedL.add(ls[0]);
    usedR.add(rs[0]);
  }
  return {
    pins,
    restLeft: left.filter((c) => !usedL.has(c)),
    restRight: right.filter((c) => !usedR.has(c)),
  };
}

/**
 * 문단 정렬 맵(`rightToLeftPara`)을 이용한 컨트롤 **슬롯** 매칭.
 *
 * 전제: `kind::key` 1차 매칭에서 빠진 표·도형·그림이 있다. 오른쪽 컨트롤 `r`의 부모 문단이
 * 맵에서 왼쪽 문단 `L`로 짝지어져 있으면, 후보는 **같은 문단 좌표(`paraPosKey(L)`)에 남아 있는
 * 왼쪽 미매칭 컨트롤**으로 한정한다. 이렇게 하면 위쪽에 표가 삽입되어 y·para 인덱스만 밀린 경우에도
 * “테스트 7 표 vs 테스트 7 표”처럼 짝을 다시 찾을 수 있다.
 *
 * 알고리즘: 오른쪽을 (section, paragraph, controlIdx)순으로 정렬한 뒤, 각 `r`에 대해 버킷 내에서
 * `scoreControlFallback` 최대 + `ALIGNMENT_CONTROL_SLOT_BONUS`가 `ALIGNMENT_CONTROL_MIN_ADJUSTED_SCORE`
 * 이상인 쌍만 채택(탐욕, 한 왼쪽 개체는 한 번만 사용).
 */
function pairAlignmentSlotControls(
  unmatchedLeft: CompareControlSnapshot[],
  unmatchedRight: CompareControlSnapshot[],
  rightToLeftPara: Map<string, CompareParaSnapshot>,
): { pairs: ControlPair[]; restLeft: CompareControlSnapshot[]; restRight: CompareControlSnapshot[] } {
  if (rightToLeftPara.size === 0) {
    return { pairs: [], restLeft: unmatchedLeft, restRight: unmatchedRight };
  }
  const leftByPara = new Map<string, CompareControlSnapshot[]>();
  for (const c of unmatchedLeft) {
    const k = paraPosKey(c);
    if (!leftByPara.has(k)) leftByPara.set(k, []);
    leftByPara.get(k)!.push(c);
  }
  for (const arr of leftByPara.values()) {
    arr.sort((a, b) => (extractControlIndexFromKey(a.key) ?? 0) - (extractControlIndexFromKey(b.key) ?? 0));
  }

  const sortedR = [...unmatchedRight].sort((a, b) => {
    if (a.section !== b.section) return a.section - b.section;
    if (a.paragraph !== b.paragraph) return a.paragraph - b.paragraph;
    return (extractControlIndexFromKey(a.key) ?? 0) - (extractControlIndexFromKey(b.key) ?? 0);
  });

  const usedL = new Set<CompareControlSnapshot>();
  const usedR = new Set<CompareControlSnapshot>();
  const pairs: ControlPair[] = [];

  for (const r of sortedR) {
    const lp = rightToLeftPara.get(paraPosKey(r));
    if (!lp) continue;
    const bucket = leftByPara.get(paraPosKey(lp));
    if (!bucket?.length) continue;
    let best: CompareControlSnapshot | null = null;
    let bestAdj = -1;
    for (const l of bucket) {
      if (usedL.has(l)) continue;
      const raw = scoreControlFallback(l, r);
      if (raw < 0) continue;
      const adj = raw + ALIGNMENT_CONTROL_SLOT_BONUS;
      if (adj > bestAdj) {
        bestAdj = adj;
        best = l;
      }
    }
    if (best && bestAdj >= ALIGNMENT_CONTROL_MIN_ADJUSTED_SCORE) {
      usedL.add(best);
      usedR.add(r);
      pairs.push({ left: best, right: r });
    }
  }

  return {
    pairs,
    restLeft: unmatchedLeft.filter((c) => !usedL.has(c)),
    restRight: unmatchedRight.filter((c) => !usedR.has(c)),
  };
}

/** compareSnapshots 등에서 options 일부가 빠졌을 때의 기본값. kinds는 UI 노이즈를 줄이기 위해 화이트리스트 형태 */
export const DEFAULT_COMPARE_OPTIONS: CompareOptions = {
  caseSensitive: true,
  ignoreWhitespace: true,
  kinds: ['text', 'table', 'shape', 'image', 'chart', 'paragraphMeta'],
};


// ─── 컨트롤 diff: 키 매칭 → 표 patience 핀 → 정렬 슬롯 → 폴백 → added/removed ───

/**
 * 표·도형·그림 등 컨트롤 diff. 단계:
 *
 * 1. **키 정확 매칭** — `kind::key`(우선 `sid:paraStableId:ci:type`). 좌우 동시 존재하면 내용 비교 후
 *    `buildGranularControlDiffs` 또는 동일 시 생략. 한쪽만 있으면 unmatched 버퍼.
 * 2. **표 Patience(선행)** — `extractTablePatiencePins`(요약 키가 양쪽에서 각각 유일할 때).
 * 3. **정렬 슬롯**(`rightToLeftPara`가 비어 있지 않을 때) — `pairAlignmentSlotControls`.
 *    짝이 맞고 요약·종류까지 같으면 diff 생략(순수 밀림), 다르면 `align-slot:` 스템으로 상세 diff.
 * 4. **전역 폴백** — `pairControlsFallback`(임계 2.75). 동일 요약·타입이면 생략.
 * 5. 남은 항목은 added / removed.
 *
 * `identity` 전략에서는 `rightToLeftPara`가 비어 3단계 슬롯만 no-op이며, 2단계 Patience는 그대로 적용된다.
 */
export function buildControlDiffs(
  left: CompareDocumentSnapshot,
  right: CompareDocumentSnapshot,
  rightToLeftPara?: Map<string, CompareParaSnapshot>,
): DiffItem[] {
  const diffs: DiffItem[] = [];
  const lmap = new Map(left.controls.map((c) => [`${c.kind}::${c.key}`, c] as const));
  const rmap = new Map(right.controls.map((c) => [`${c.kind}::${c.key}`, c] as const));
  const unmatchedLeft: CompareControlSnapshot[] = [];
  const unmatchedRight: CompareControlSnapshot[] = [];

  // 1차: key 기반 정확 매칭
  const keys = new Set([...lmap.keys(), ...rmap.keys()]);
  for (const key of keys) {
    const l = lmap.get(key);
    const r = rmap.get(key);
    if (l && r) {
      if (l.summary !== r.summary || l.kind !== r.kind) {
        diffs.push(...buildGranularControlDiffs(l, r, `modified:${key}`));
      }
      continue;
    }
    if (l) unmatchedLeft.push(l);
    if (r) unmatchedRight.push(r);
  }

  const {
    pins: tablePins,
    restLeft: afterPatienceL,
    restRight: afterPatienceR,
  } = extractTablePatiencePins(unmatchedLeft, unmatchedRight);
  for (const { left: l, right: r } of tablePins) {
    if (l.summary !== r.summary || l.kind !== r.kind) {
      diffs.push(...buildGranularControlDiffs(l, r, `table-pin:${l.key}=>${r.key}`));
    }
  }

  let slotRestL = afterPatienceL;
  let slotRestR = afterPatienceR;
  if (rightToLeftPara && rightToLeftPara.size > 0) {
    const slot = pairAlignmentSlotControls(afterPatienceL, afterPatienceR, rightToLeftPara);
    for (const { left: l, right: r } of slot.pairs) {
      if (l.summary !== r.summary || l.kind !== r.kind) {
        diffs.push(...buildGranularControlDiffs(l, r, `align-slot:${l.key}=>${r.key}`));
      }
    }
    slotRestL = slot.restLeft;
    slotRestR = slot.restRight;
  }

  // 2차: key가 달라진 컨트롤(특히 표/이미지)의 폴백 매칭
  const fallbackPairs = pairControlsFallback(slotRestL, slotRestR);
  const pairedL = new Set(fallbackPairs.map((p) => p.left));
  const pairedR = new Set(fallbackPairs.map((p) => p.right));
  for (const { left: l, right: r } of fallbackPairs) {
    // 매칭 키만 달라지고 내용/속성이 동일하면 "밀림 보정" 성격의 재매칭이므로 결과에서 제외한다.
    if (l.summary === r.summary && l.type === r.type && l.kind === r.kind) continue;
    diffs.push(...buildGranularControlDiffs(l, r, `fallback-modified:${l.key}=>${r.key}`));
  }

  for (const r of slotRestR) {
    if (pairedR.has(r)) continue;
    const key = `${r.kind}::${r.key}`;
    diffs.push({
      id: mkDiffId(r.kind, `added:${key}`),
      kind: r.kind,
      severity: 'added',
      path: { section: r.section, paragraph: r.paragraph, controlKey: key },
      title: `${kindLabel(r.kind)} 추가`,
      leftPreview: '',
      rightPreview: r.summary,
      rightAnchor: r.anchor,
    });
  }
  for (const l of slotRestL) {
    if (pairedL.has(l)) continue;
    const key = `${l.kind}::${l.key}`;
    diffs.push({
      id: mkDiffId(l.kind, `removed:${key}`),
      kind: l.kind,
      severity: 'removed',
      path: { section: l.section, paragraph: l.paragraph, controlKey: key },
      title: `${kindLabel(l.kind)} 삭제`,
      leftPreview: l.summary,
      rightPreview: '',
      leftAnchor: l.anchor,
    });
  }
  return diffs;
}

/**
 * 키·표 Patience·슬롯 이후에도 남은 좌·우 컨트롤을 **전역 탐욕**으로 짝지음(오른쪽 각각에 대해 왼쪽 후보 중 최고점).
 *
 * `bestScore >= 2.75` 미만이면 매칭하지 않는다 — 절대 좌표·요약 유사도만으로 오매칭하면 added/removed 노이즈가 커지기 때문.
 * 문단 정렬이 신뢰되는 경우에는 `pairAlignmentSlotControls`가 같은 논리 문단 안에서 점수 보너스를 준다.
 */
function pairControlsFallback(
  left: CompareControlSnapshot[],
  right: CompareControlSnapshot[],
): ControlPair[] {
  const pairs: ControlPair[] = [];
  const usedL = new Set<number>();
  for (let ri = 0; ri < right.length; ri += 1) {
    const r = right[ri];
    let bestLi = -1;
    let bestScore = -1;
    for (let li = 0; li < left.length; li += 1) {
      if (usedL.has(li)) continue;
      const l = left[li];
      const score = scoreControlFallback(l, r);
      if (score > bestScore) {
        bestScore = score;
        bestLi = li;
      }
    }
    if (bestLi >= 0 && bestScore >= 2.75) {
      usedL.add(bestLi);
      pairs.push({ left: left[bestLi], right: r });
    }
  }
  return pairs;
}

/**
 * 표 한 쌍의 “같은 표에서 셀만 바뀜” 정도를 0~1로 본다. 행·열(r/c)이 다르면 낮게 나와 전액 감점된다.
 */
function tableControlPairContentSimilarity(l: CompareControlSnapshot, r: CompareControlSnapshot): number {
  const lk = parseSummaryKV(l.summary);
  const rk = parseSummaryKV(r.summary);
  const sameGrid = (lk.r ?? '') === (rk.r ?? '') && (lk.c ?? '') === (rk.c ?? '');
  if (!sameGrid) return 0.08;

  const pickCells = (kv: Record<string, string>) => {
    const cp = kv.cprev;
    if (cp && cp !== '(없음)') return cp;
    const tp = kv.tprev;
    if (tp && tp !== '(없음)') return tp;
    return kv.txt ?? '';
  };
  const a = pickCells(lk);
  const b = pickCells(rk);
  if (a || b) return textSimilarity(a, b);

  const shaL = lk.csha ?? '';
  const shaR = rk.csha ?? '';
  if (shaL && shaR && shaL === shaR) return 1;

  return textSimilarity(l.summary.replace(/\s+/g, ' '), r.summary.replace(/\s+/g, ' '));
}

function tableSummaryMismatchPenaltyFactor(l: CompareControlSnapshot, r: CompareControlSnapshot): number {
  const sim = tableControlPairContentSimilarity(l, r);
  if (sim >= TABLE_PAIR_SIM_NO_PENALTY) return 0;
  if (sim <= TABLE_PAIR_SIM_FULL_PENALTY) return 1;
  return (TABLE_PAIR_SIM_NO_PENALTY - sim) / (TABLE_PAIR_SIM_NO_PENALTY - TABLE_PAIR_SIM_FULL_PENALTY);
}

/**
 * `pairControlsFallback` / `pairAlignmentSlotControls` 공통 점수 함수.
 *
 * - kind 불일치 → -1 (즉시 탈락).
 * - 동일 `type`, 동일 `summary`에 큰 가중(표·도형은 요약 문자열에 구조·텍스트 요약이 들어 있음).
 * - **표끼리 요약이 다르면** `TABLE_SUMMARY_MISMATCH_PENALTY`를 **내용 유사도에 비례해** 감점(같은 그리드·거의 같은 셀 문자열이면 감점 없음).
 * - 그다음 `sid`/`loc` 키에서 뽑은 **control index** 일치, 같은 section, 페이지 간격, 앵커 박스 x/y/wh 근접.
 *
 * 문단 삽입으로 y가 크게 밀리면 위치 항만으로는 2.75에 못 미칠 수 있어, alignment 맵이 있을 때는
 * 슬롯 단계에서 보너스를 더해 같은 문제를 완화한다.
 */
function scoreControlFallback(l: CompareControlSnapshot, r: CompareControlSnapshot): number {
  if (l.kind !== r.kind) return -1;
  let score = 0;
  if (l.type === r.type) score += 1.2;
  if (l.summary === r.summary) score += 2.6;
  else {
    const ld = l.summary.toLowerCase();
    const rd = r.summary.toLowerCase();
    if (ld.includes('table') && rd.includes('table')) score += 1.0;
    if (ld.includes('shape') && rd.includes('shape')) score += 0.8;
  }
  const lci = extractControlIndexFromKey(l.key);
  const rci = extractControlIndexFromKey(r.key);
  if (lci != null && rci != null && lci === rci) score += 1.15;
  if (l.section === r.section) score += 0.45;
  const pageGap = Math.abs(l.anchor.pageIndex - r.anchor.pageIndex);
  if (pageGap === 0) score += 0.9;
  else if (pageGap === 1) score += 0.45;
  const xGap = Math.abs(l.anchor.x - r.anchor.x);
  const yGap = Math.abs(l.anchor.y - r.anchor.y);
  const wGap = Math.abs(l.anchor.width - r.anchor.width);
  const hGap = Math.abs(l.anchor.height - r.anchor.height);
  if (xGap < 120) score += 0.35;
  else if (xGap < 260) score += 0.18;
  if (yGap < 80) score += 0.6;
  else if (yGap < 180) score += 0.3;
  if (wGap < 45 && hGap < 45) score += 0.45;
  else if (wGap < 120 && hGap < 120) score += 0.22;
  if (l.kind === 'table' && r.kind === 'table' && l.summary !== r.summary) {
    const factor = tableSummaryMismatchPenaltyFactor(l, r);
    score -= TABLE_SUMMARY_MISMATCH_PENALTY * factor;
  }
  return score;
}

/**
 * 각 DiffItem에 구역 내 "사람이 읽는" 쪽번호(`sectionPage`)를 붙인다.
 * identity id 패턴·path·controlKey 내 sid를 역추적해 문단을 찾고, 실패 시 anchor의 전역 pageDisplayNumbers로 보완.
 */
export function annotateDiffSectionPages(
  diffs: DiffItem[],
  left: CompareDocumentSnapshot,
  right: CompareDocumentSnapshot,
): void {
  const lByPos = new Map<string, CompareParaSnapshot>();
  const rByPos = new Map<string, CompareParaSnapshot>();
  const lByStable = new Map<string, CompareParaSnapshot>();
  const rByStable = new Map<string, CompareParaSnapshot>();
  for (const p of left.paragraphs) {
    lByPos.set(`${p.section}:${p.paragraph}`, p);
    lByStable.set(p.stableId, p);
  }
  for (const p of right.paragraphs) {
    rByPos.set(`${p.section}:${p.paragraph}`, p);
    rByStable.set(p.stableId, p);
  }

  for (const d of diffs) {
    let lp: CompareParaSnapshot | undefined;
    let rp: CompareParaSnapshot | undefined;

    const sidMatch = d.id.match(/id-(?:added|removed|modified|moved|ctrlcount):(.+)$/);
    if (sidMatch) {
      const sid = sidMatch[1];
      lp = lByStable.get(sid);
      rp = rByStable.get(sid);
    } else {
      const key = `${d.path.section}:${d.path.paragraph ?? -1}`;
      if (d.severity === 'removed') {
        lp = lByPos.get(key);
      } else if (d.severity === 'added') {
        rp = rByPos.get(key);
      } else {
        lp = lByPos.get(key);
        rp = rByPos.get(key);
      }
    }

    // 컨트롤 diff는 controlKey에 부모 문단 stable_id가 포함될 수 있다.
    // path 기반 문단 매핑이 어긋난 경우 sid로 좌/우 문단을 재식별해 쪽번호를 고정한다.
    if ((!lp || !rp) && d.path.controlKey) {
      const sidMatch = d.path.controlKey.match(/sid:([^:]+):\d+:/);
      if (sidMatch) {
        const sid = sidMatch[1];
        if (!lp) lp = lByStable.get(sid);
        if (!rp) rp = rByStable.get(sid);
      }
    }

    if (!lp && d.contextOnLeft) {
      lp = lByPos.get(`${d.contextOnLeft.section}:${d.contextOnLeft.paragraph}`);
    }
    if (!rp && d.contextOnRight) {
      rp = rByPos.get(`${d.contextOnRight.section}:${d.contextOnRight.paragraph}`);
    }

    if (lp) d.leftSectionPage = lp.sectionPage;
    if (rp) d.rightSectionPage = rp.sectionPage;

    // context 짝 문단이 잡혔는데 구역 쪽번호가 비어 있으면(특정 조판에서 생략) 앵커 기준 표시쪽으로 보강한다.
    if (!d.leftSectionPage && lp?.anchor?.pageIndex !== undefined && left.meta.pageDisplayNumbers) {
      const pn = left.meta.pageDisplayNumbers[lp.anchor.pageIndex];
      if (pn && pn > 0) d.leftSectionPage = pn;
    }
    if (!d.rightSectionPage && rp?.anchor?.pageIndex !== undefined && right.meta.pageDisplayNumbers) {
      const pn = right.meta.pageDisplayNumbers[rp.anchor.pageIndex];
      if (pn && pn > 0) d.rightSectionPage = pn;
    }

    // 문단 매핑 실패(특히 컨트롤/일부 메타) 시에도 렌더 엔진이 계산한 표시 쪽번호를 사용.
    if (!d.leftSectionPage && d.leftAnchor) {
      const pn = left.meta.pageDisplayNumbers?.[d.leftAnchor.pageIndex];
      if (pn && pn > 0) d.leftSectionPage = pn;
    }
    if (!d.rightSectionPage && d.rightAnchor) {
      const pn = right.meta.pageDisplayNumbers?.[d.rightAnchor.pageIndex];
      if (pn && pn > 0) d.rightSectionPage = pn;
    }
  }
}
