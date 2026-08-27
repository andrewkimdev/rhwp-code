/** alignment 본문 조립 — 앵커 → 구간 정렬(`matchSegment`) → 스텝 스트림 → cleanup → `buildTextDiffs`. */
import { compareDbg, isCompareDebugEnabled } from './compare-debug';
import {
  buildAnchorPairs,
  isLeftParagraphSplitIntoTwoRightParas,
  matchSegment,
  preferRightPath,
  shouldMergeRemovedAddedAsModify,
  shouldPromoteEmptyTextEdit,
} from './align-core';
import type { AlignedPair } from './align-core';
import { buildRightToLeftParaMapFromAligned, mkDiffId } from './signature';
import {
  formatParaLocTitle,
  shouldSuppressNoiseParagraphOnly,
  stripNoiseOnlyParagraphAlignSteps,
} from './snapshot';
import { MOVE_DISTANCE_THRESHOLD, resolveAnchorTuning } from './tuning';
import type { CompareContext, SegmentAnchorBoundary } from './tuning';
import type {
  CompareDocumentSnapshot,
  CompareParaSnapshot,
  DiffItem,
} from './types';

// ─── alignment 본문: 앵커 → 구간 정렬 → 단계 스트림 → cleanup ─────────────────

/**
 * `AlignedPair`를 한 칸씩 전진하는 스트림으로 바꾼다(DMP cleanup 전 단계).
 * lookahead 없이 `cleanupParagraphAlignStepsToDiffItems`에서만 2칸 패턴을 처리한다.
 */
export type ParagraphAlignStep =
  | { kind: 'lr'; l: CompareParaSnapshot; r: CompareParaSnapshot }
  | { kind: 'l'; l: CompareParaSnapshot }
  | { kind: 'r'; r: CompareParaSnapshot };

/** 정렬 스트림에서 직전에 등장한 왼쪽 문단(짝 맞춤 기준) — 오른쪽 전용 추가 항목의 왼쪽 패널 프리뷰용 */
function nearestAlignedLeftPeer(steps: ParagraphAlignStep[], fromIndex: number): CompareParaSnapshot | null {
  for (let j = fromIndex - 1; j >= 0; j--) {
    const s = steps[j];
    if (s.kind === 'lr') return s.l;
    if (s.kind === 'l') return s.l;
  }
  return null;
}

/** 정렬 스트림에서 직전에 등장한 오른쪽 문단 — 왼쪽 전용 삭제 항목의 오른쪽 패널 프리뷰용 */
function nearestAlignedRightPeer(steps: ParagraphAlignStep[], fromIndex: number): CompareParaSnapshot | null {
  for (let j = fromIndex - 1; j >= 0; j--) {
    const s = steps[j];
    if (s.kind === 'lr') return s.r;
    if (s.kind === 'r') return s.r;
  }
  return null;
}

function buildParagraphAlignStepsFromAligned(aligned: AlignedPair[]): ParagraphAlignStep[] {
  const steps: ParagraphAlignStep[] = [];
  for (const pair of aligned) {
    const { left: l, right: r } = pair;
    if (!l && !r) continue;
    if (l && !r) steps.push({ kind: 'l', l });
    else if (!l && r) steps.push({ kind: 'r', r });
    else steps.push({ kind: 'lr', l: l!, r: r! });
  }
  return steps;
}

/**
 * 정렬 스트림을 `DiffItem[]`로 바꾼다. DMP `diff_cleanupSemantic`과 같이
 * “기계적 변환 + 고정 순서 cleanup”을 한곳에 모은다.
 */
function cleanupParagraphAlignStepsToDiffItems(
  steps: ParagraphAlignStep[],
  lps: CompareParaSnapshot[],
  rps: CompareParaSnapshot[],
): DiffItem[] {
  const diffs: DiffItem[] = [];
  let i = 0;
  while (i < steps.length) {
    const s0 = steps[i];
    const s1 = i + 1 < steps.length ? steps[i + 1] : null;

    if (s0.kind === 'l' && s1?.kind === 'r' && shouldPromoteEmptyTextEdit(s0.l, s1.r, lps, rps)) {
      diffs.push({
        id: mkDiffId('text', `modified-empty-edit:${s0.l.section}:${s0.l.paragraph}`),
        kind: 'text',
        severity: 'modified',
        path: preferRightPath(s0.l, s1.r),
        title: `텍스트 변경 (${formatParaLocTitle(s0.l)})`,
        leftPreview: s0.l.text,
        rightPreview: s1.r.text,
        leftAnchor: s0.l.anchor,
        rightAnchor: s1.r.anchor,
      });
      i += 2;
      continue;
    }
    if (s0.kind === 'r' && s1?.kind === 'l' && shouldPromoteEmptyTextEdit(s1.l, s0.r, lps, rps)) {
      diffs.push({
        id: mkDiffId('text', `modified-empty-edit:${s1.l.section}:${s1.l.paragraph}`),
        kind: 'text',
        severity: 'modified',
        path: preferRightPath(s1.l, s0.r),
        title: `텍스트 변경 (${formatParaLocTitle(s1.l)})`,
        leftPreview: s1.l.text,
        rightPreview: s0.r.text,
        leftAnchor: s1.l.anchor,
        rightAnchor: s0.r.anchor,
      });
      i += 2;
      continue;
    }

    if (s0.kind === 'l' && s1?.kind === 'r' && shouldMergeRemovedAddedAsModify(s0.l, s1.r)) {
      const r2 = s1.r;
      diffs.push({
        id: mkDiffId('text', `modified-merged:${s0.l.section}:${s0.l.paragraph}`),
        kind: 'text',
        severity: 'modified',
        path: preferRightPath(s0.l, r2),
        title: `텍스트 변경 (${formatParaLocTitle(r2)})`,
        leftPreview: s0.l.text,
        rightPreview: r2.text,
        leftAnchor: s0.l.anchor,
        rightAnchor: r2.anchor,
        leftSectionPage: s0.l.sectionPage,
        rightSectionPage: r2.sectionPage,
      });
      i += 2;
      continue;
    }
    if (s0.kind === 'r' && s1?.kind === 'l' && shouldMergeRemovedAddedAsModify(s1.l, s0.r)) {
      const l2 = s1.l;
      diffs.push({
        id: mkDiffId('text', `modified-merged:${l2.section}:${l2.paragraph}`),
        kind: 'text',
        severity: 'modified',
        path: preferRightPath(l2, s0.r),
        title: `텍스트 변경 (${formatParaLocTitle(s0.r)})`,
        leftPreview: l2.text,
        rightPreview: s0.r.text,
        leftAnchor: l2.anchor,
        rightAnchor: s0.r.anchor,
        leftSectionPage: l2.sectionPage,
        rightSectionPage: s0.r.sectionPage,
      });
      i += 2;
      continue;
    }

    if (s0.kind === 'r' && s1?.kind === 'lr' && isLeftParagraphSplitIntoTwoRightParas(s1.l, s0.r, s1.r)) {
      const L = s1.l;
      const rHead = s0.r;
      const rTail = s1.r;
      diffs.push(
        {
          id: mkDiffId('text', `modified:${L.section}:${L.paragraph}`),
          kind: 'text',
          severity: 'modified',
          path: preferRightPath(L, rHead),
          title: `텍스트 변경 (${formatParaLocTitle(rHead)})`,
          leftPreview: L.text,
          rightPreview: rHead.text,
          leftAnchor: L.anchor,
          rightAnchor: rHead.anchor,
          leftSectionPage: L.sectionPage,
          rightSectionPage: rHead.sectionPage,
        },
        ...(shouldSuppressNoiseParagraphOnly(rTail)
          ? []
          : [
              {
                id: mkDiffId('text', `added:${rTail.section}:${rTail.paragraph}`),
                kind: 'text' as const,
                severity: 'added' as const,
                path: { section: rTail.section, paragraph: rTail.paragraph },
                title: `문단 추가 (${formatParaLocTitle(rTail)})`,
                leftPreview: '',
                rightPreview: rTail.text,
                rightAnchor: rTail.anchor,
                rightSectionPage: rTail.sectionPage,
                contextOnLeft: { section: L.section, paragraph: L.paragraph },
              },
            ]),
      );
      i += 2;
      continue;
    }

    if (s0.kind === 'r') {
      const r = s0.r;
      if (!shouldSuppressNoiseParagraphOnly(r)) {
        const ctxL = nearestAlignedLeftPeer(steps, i);
        diffs.push({
          id: mkDiffId('text', `added:${r.section}:${r.paragraph}`),
          kind: 'text',
          severity: 'added',
          path: { section: r.section, paragraph: r.paragraph },
          title: `문단 추가 (${formatParaLocTitle(r)})`,
          leftPreview: '',
          rightPreview: r.text,
          rightAnchor: r.anchor,
          rightSectionPage: r.sectionPage,
          ...(ctxL ? { contextOnLeft: { section: ctxL.section, paragraph: ctxL.paragraph } } : {}),
        });
      }
      i += 1;
      continue;
    }
    if (s0.kind === 'l') {
      const l = s0.l;
      if (!shouldSuppressNoiseParagraphOnly(l)) {
        const ctxR = nearestAlignedRightPeer(steps, i);
        diffs.push({
          id: mkDiffId('text', `removed:${l.section}:${l.paragraph}`),
          kind: 'text',
          severity: 'removed',
          path: preferRightPath(l, null),
          title: `문단 삭제 (${formatParaLocTitle(l)})`,
          leftPreview: l.text,
          rightPreview: '',
          leftAnchor: l.anchor,
          leftSectionPage: l.sectionPage,
          ...(ctxR ? { contextOnRight: { section: ctxR.section, paragraph: ctxR.paragraph } } : {}),
        });
      }
      i += 1;
      continue;
    }

    const l = s0.l;
    const r = s0.r;
    if (l.normalizedText !== r.normalizedText) {
      diffs.push({
        id: mkDiffId('text', `modified:${l.section}:${l.paragraph}`),
        kind: 'text',
        severity: 'modified',
        path: preferRightPath(l, r),
        title: `텍스트 변경 (${formatParaLocTitle(l)})`,
        leftPreview: l.text,
        rightPreview: r.text,
        leftAnchor: l.anchor,
        rightAnchor: r.anchor,
      });
    }

    if (l.signature === r.signature && Math.abs(l.globalIndex - r.globalIndex) > MOVE_DISTANCE_THRESHOLD) {
      diffs.push({
        id: mkDiffId('paragraphMeta', `moved:${l.section}:${l.paragraph}`),
        kind: 'paragraphMeta',
        severity: 'modified',
        path: { section: l.section, paragraph: l.paragraph },
        title: `문단 이동 감지 (${formatParaLocTitle(l)})`,
        leftPreview: `A idx=${l.globalIndex}`,
        rightPreview: `B idx=${r.globalIndex}`,
        leftAnchor: l.anchor,
        rightAnchor: r.anchor,
      });
    }

    if (l.controlCount !== r.controlCount) {
      diffs.push({
        id: mkDiffId('paragraphMeta', `ctrlcount:${l.section}:${l.paragraph}`),
        kind: 'paragraphMeta',
        severity: 'modified',
        path: { section: l.section, paragraph: l.paragraph },
        title: `문단 개체 수 변경 (${formatParaLocTitle(l)})`,
        leftPreview: `controls=${l.controlCount}`,
        rightPreview: `controls=${r.controlCount}`,
        leftAnchor: l.anchor,
        rightAnchor: r.anchor,
      });
    }

    i += 1;
  }
  return diffs;
}

/**
 * 문서 대 문서(alignment) 텍스트 diff + 컨트롤 매칭용 문단 맵.
 *
 * 1. `buildAnchorPairs`: 글로벌 앵커(단조 `ri`, 시그니처 유일, 짧은 줄은 trim 양쪽 1회).
 * 2. 앵커 경계마다 `matchSegment(..., anchorBoundary)`로 `AlignedPair[]` 누적.
 * 3. `buildRightToLeftParaMapFromAligned(aligned)`: 오른쪽 문단 좌표 → 짝 왼쪽 문단(양쪽 non-null만).
 * 4. `buildParagraphAlignStepsFromAligned` → `cleanupParagraphAlignStepsToDiffItems`로 `DiffItem[]` 생성.
 *
 * 반환의 `rightToLeftPara`는 cleanup 이전 `aligned`와 일치한다. 컨트롤 단계는 이 맵으로
 * 물리 `paragraph`가 달라도 같은 논리 문단의 개체를 다시 붙인다(`buildControlDiffs`).
 */
export function buildTextDiffs(ctx: CompareContext, left: CompareDocumentSnapshot, right: CompareDocumentSnapshot): {
  diffs: DiffItem[];
  rightToLeftPara: Map<string, CompareParaSnapshot>;
} {
  const lps = left.paragraphs;
  const rps = right.paragraphs;
  const anchorTuning = resolveAnchorTuning(ctx.options);
  const anchors = buildAnchorPairs(lps, rps, anchorTuning);
  if (isCompareDebugEnabled()) {
    compareDbg(
      '[③ 앵커] 시그니처 유일 + 품질필터(길이/공백비율/엔트로피). 빈 줄·패턴 문장이 많으면 구간이 어긋날 수 있음.',
      `앵커 ${anchors.length}쌍 (앞 20개)`,
      anchors.slice(0, 20).map(({ li, ri }) => ({
        li,
        ri,
        left: lps[li].text.slice(0, 44),
        right: rps[ri].text.slice(0, 44),
        normLenL: lps[li].normalizedText.length,
        normLenR: rps[ri].normalizedText.length,
        anchorMinLen: anchorTuning.minTextLen,
      })),
    );
  }
  // 경계: (문서 시작) — 앵커1 — … — 앵커N — (문서 끝). 각 앵커 쌍 자체는 1:1 고정 매칭.
  const boundaries = [{ li: -1, ri: -1 }, ...anchors, { li: lps.length, ri: rps.length }];

  const aligned: AlignedPair[] = [];
  for (let i = 0; i < boundaries.length - 1; i += 1) {
    const a = boundaries[i];
    const b = boundaries[i + 1];

    if (a.li >= 0 && a.ri >= 0) {
      aligned.push({ left: lps[a.li], right: rps[a.ri] });
    }

    // 앵커 (a)와 (b) "사이"의 문단만 DP/그리디에 넘긴다. 앵커 문단 본인은 위에서 이미 짝을 맞춤.
    const leftSeg = lps.slice(a.li + 1, b.li);
    const rightSeg = rps.slice(a.ri + 1, b.ri);
    if (leftSeg.length === 0 && rightSeg.length === 0) continue;
    const anchorBoundary: SegmentAnchorBoundary | null =
      a.li >= 0 && a.ri >= 0
        ? { leftAnchorGi: lps[a.li].globalIndex, rightAnchorGi: rps[a.ri].globalIndex }
        : null;
    aligned.push(...matchSegment(ctx, leftSeg, rightSeg, 0, anchorBoundary));
  }

  const rightToLeftPara = buildRightToLeftParaMapFromAligned(aligned);
  const steps = stripNoiseOnlyParagraphAlignSteps(buildParagraphAlignStepsFromAligned(aligned));
  const diffs = cleanupParagraphAlignStepsToDiffItems(steps, lps, rps);
  return { diffs, rightToLeftPara };
}
