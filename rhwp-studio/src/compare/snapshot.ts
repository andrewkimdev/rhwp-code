/** 스냅샷 수집 — WASM 단일 문서를 `CompareDocumentSnapshot`으로 만들고, 노이즈 문단 억제와 stable_id 맵을 제공한다. */
import { WasmBridge } from '@/core/wasm-bridge';
import type { ControlLayoutItem, DocumentInfo, ParaProperties } from '@/core/types';
import { compareDbg, isCompareDebugEnabled } from './compare-debug';
import {
  STRUCTURAL_ANCHOR_MIN_LEN,
  isAnchorTextQualityOk,
  isStructuralBlockTitleLine,
  resolveAnchorTuning,
} from './tuning';
import {
  buildTableSummary,
  canonicalControlKey,
  controlSnapshotQuality,
  mapControlKind,
  normalizeText,
  simpleHash,
  simpleHashBytes,
} from './signature';
import type { ParagraphAlignStep } from './align-assembly';
import type {
  CompareControlSnapshot,
  CompareDocumentSnapshot,
  CompareOptions,
  CompareParaSnapshot,
  DiffAnchor,
} from './types';

// ─── 스냅샷 수집: WASM 단일 문서 → CompareDocumentSnapshot ─────────────────────

/** 앵커용 문단 모양 요약 — 텍스트만 같고 번호/정렬이 다른 문단을 구분한다. */
function compactParaShapeForAnchor(pp: ParaProperties): string {
  const q = (v: number | undefined) => (v == null || Number.isNaN(v) ? 0 : Math.round(v / 2) * 2);
  return JSON.stringify({
    a: pp.alignment ?? '',
    h: pp.headType ?? '',
    lv: pp.paraLevel ?? 0,
    n: pp.numberingId ?? 0,
    i: q(pp.indent),
    ml: q(pp.marginLeft),
  });
}

/**
 * `getCursorRect(sec,para,0)`이 실패하는 문단(표 셀 내부·빈 줄 등)에 대해 오프셋을 바꿔 재시도한다.
 */
function tryResolveCompareParaAnchorFromCursor(
  wasm: WasmBridge,
  sec: number,
  para: number,
  textLength: number,
): DiffAnchor | undefined {
  const offsets = new Set<number>([0]);
  if (textLength > 0) {
    offsets.add(1);
    offsets.add(Math.max(0, textLength - 1));
    if (textLength > 2) offsets.add(Math.floor(textLength / 2));
  }
  for (const off of offsets) {
    try {
      const rect = wasm.getCursorRect(sec, para, off);
      if (rect && typeof rect.pageIndex === 'number' && Number.isFinite(rect.x) && Number.isFinite(rect.y)) {
        return {
          pageIndex: rect.pageIndex,
          x: rect.x,
          y: rect.y,
          width: 320,
          height: Math.max(18, rect.height || 18),
        };
      }
    } catch {
      /* 다음 오프셋 */
    }
  }
  return undefined;
}

/**
 * 커서 rect를 못 얻은 문단에 대해, 해당 문단에 붙은 첫 레이아웃 개체 박스로 앵커를 채운다(비교 상세 캔버스용).
 */
function fillMissingParaAnchorsFromPageLayout(
  wasm: WasmBridge,
  info: DocumentInfo,
  paragraphs: CompareParaSnapshot[],
  displayedPageByGlobalPage: Map<number, number>,
): void {
  const firstBoxByPara = new Map<string, DiffAnchor>();
  for (let page = 0; page < info.pageCount; page += 1) {
    let controls: ControlLayoutItem[];
    try {
      controls = wasm.getPageControlLayout(page).controls;
    } catch {
      continue;
    }
    for (const item of controls) {
      const sec = item.secIdx;
      const pIdx = item.paraIdx;
      if (sec == null || pIdx == null || sec < 0 || pIdx < 0) continue;
      const key = `${sec}:${pIdx}`;
      if (firstBoxByPara.has(key)) continue;
      firstBoxByPara.set(key, {
        pageIndex: page,
        x: item.x,
        y: item.y,
        width: Math.max(48, Math.round(item.w)),
        height: Math.max(18, Math.round(item.h)),
      });
    }
  }
  for (const p of paragraphs) {
    if (p.anchor) continue;
    const fb = firstBoxByPara.get(`${p.section}:${p.paragraph}`);
    if (!fb) continue;
    p.anchor = fb;
    p.sectionPage = displayedPageByGlobalPage.get(fb.pageIndex) ?? (fb.pageIndex + 1);
  }
}

/**
 * 레이아웃에도 없으면 같은 구역에서 가장 가까운 앵커가 있는 문단 좌표를 복사한다.
 * 이전 문단 앵커를 **그대로** 쓰면(표가 있는 문단 등) 비교 상세 마커가 표 위에 겹쳐 “문단 추가 = 표”로 보이므로 세로로 한 칸 밀어 구분한다.
 */
function fillMissingParaAnchorsFromNeighbors(
  paragraphs: CompareParaSnapshot[],
  displayedPageByGlobalPage: Map<number, number>,
): void {
  for (const p of paragraphs) {
    if (p.anchor) continue;
    let nbr: DiffAnchor | undefined;
    let fromPreviousPara = false;
    for (let gi = p.globalIndex - 1; gi >= 0; gi -= 1) {
      const q = paragraphs[gi];
      if (q.section !== p.section) break;
      if (q.anchor) {
        nbr = q.anchor;
        fromPreviousPara = true;
        break;
      }
    }
    if (!nbr) {
      for (let gi = p.globalIndex + 1; gi < paragraphs.length; gi += 1) {
        const q = paragraphs[gi];
        if (q.section !== p.section) break;
        if (q.anchor) {
          nbr = q.anchor;
          fromPreviousPara = false;
          break;
        }
      }
    }
    if (!nbr) continue;
    if (fromPreviousPara) {
      const dy = Math.min(200, Math.max(28, Math.round(nbr.height) + 10));
      p.anchor = {
        ...nbr,
        y: nbr.y + dy,
        width: nbr.width,
        height: Math.max(18, nbr.height),
      };
    } else {
      p.anchor = {
        ...nbr,
        y: Math.max(0, nbr.y - 28),
        width: nbr.width,
        height: Math.max(18, nbr.height),
      };
    }
    p.sectionPage = displayedPageByGlobalPage.get(nbr.pageIndex) ?? (nbr.pageIndex + 1);
  }
}

/**
 * WASM이 열린 단일 문서에서 비교용 스냅샷을 만든다.
 * - 문단: 텍스트, 정규화 텍스트, stable_id, 레이아웃 앵커(커서 rect·다중 오프셋 → 페이지 개체 박스 → 이웃 문단), 구역 내 쪽번호,
 *   `signature`(정규화 텍스트·컨트롤 개수 + `getParaPropertiesAt` 기반 문단모양 요약 `ps:`)
 * - 개체: 페이지 레이아웃 + 문단별 table 순회를 합쳐 키를 `sid:` 우선으로 통일
 * - 마지막에 `canonicalControlKey`로 중복을 합치고 `controlSnapshotQuality`로 더 나은 요약을 남긴다.
 */
function fillSnapshotFromWasm(
  wasm: WasmBridge,
  info: DocumentInfo,
  displayName: string,
  options: CompareOptions,
): CompareDocumentSnapshot {
  // 비교 스냅샷 직전에 강제 재조판하여 폰트/도형 반영 지연으로 인한 페이지 밀림을 줄인다.
  wasm.refreshLayout();

  const displayedPageByGlobalPage = new Map<number, number>();
  for (let page = 0; page < info.pageCount; page += 1) {
    try {
      const pi = wasm.getPageInfo(page);
      displayedPageByGlobalPage.set(page, pi.pageNumber ?? (page + 1));
    } catch {
      // page info 조회 실패 시 후속 fallback 사용
    }
  }

  const paragraphs: CompareParaSnapshot[] = [];
  /** 구역·문단별: 글머리/번호/개요 등 HWP `headType`이 Outline·Number인지(짧은 제목 앵커 후보용) */
  const paraHeadOutlineOrNumber = new Map<string, boolean>();
  let globalIndex = 0;
  for (let sec = 0; sec < info.sectionCount; sec += 1) {
    const paraCount = wasm.getParagraphCount(sec);
    for (let para = 0; para < paraCount; para += 1) {
      const length = wasm.getParagraphLength(sec, para);
      const text = length > 0 ? wasm.getTextRange(sec, para, 0, length) : '';
      const controls = wasm.getControlTextPositions(sec, para);
      const normalizedText = normalizeText(text, options);
      let shapeDigest = '';
      try {
        const pp = wasm.getParaPropertiesAt(sec, para);
        shapeDigest = simpleHash(compactParaShapeForAnchor(pp));
        const ht = pp.headType ?? 'None';
        paraHeadOutlineOrNumber.set(`${sec}:${para}`, ht === 'Outline' || ht === 'Number');
      } catch {
        shapeDigest = '';
        paraHeadOutlineOrNumber.set(`${sec}:${para}`, false);
      }
      // 문단 "내용+컨트롤 개수+문단모양" 지문 — alignment에서 앵커/유일쌍 찾기에 사용
      const signature = simpleHash(
        `${normalizedText}|cc:${controls.length}${shapeDigest ? `|ps:${shapeDigest}` : ''}`,
      );
      const stableId = wasm.getParagraphStableId(sec, para);
      const anchor = tryResolveCompareParaAnchorFromCursor(wasm, sec, para, length);
      const sectionPage = (() => {
        if (!anchor) return 1;
        return displayedPageByGlobalPage.get(anchor.pageIndex) ?? (anchor.pageIndex + 1);
      })();
      paragraphs.push({
        section: sec,
        paragraph: para,
        sectionPage,
        globalIndex,
        stableId,
        text,
        normalizedText,
        controlCount: controls.length,
        signature,
        isAnchorCandidate: false,
        anchor,
      });
      globalIndex += 1;
    }
  }

  fillMissingParaAnchorsFromPageLayout(wasm, info, paragraphs, displayedPageByGlobalPage);
  fillMissingParaAnchorsFromNeighbors(paragraphs, displayedPageByGlobalPage);

  // 글로벌 앵커 후보(`isAnchorCandidate`): 시그니처 중복은 즉시 탈락. 그 다음
  // - 길이·엔트로피 등으로 “긴” 문단은 `isAnchorTextQualityOk`
  // - 짧은 제목 줄은 `[…]` / 번호 목록 패턴(`isStructuralBlockTitleLine`) 또는 HWP Outline/Number 머리
  // `buildAnchorPairs` 단계에서 짧은 후보는 `buildBothSidesUniqueTrimNorm`으로 양쪽 각 1회만 추가 검증.
  const anchorTuning = resolveAnchorTuning(options);
  const sigCount = new Map<string, number>();
  for (const p of paragraphs) {
    sigCount.set(p.signature, (sigCount.get(p.signature) ?? 0) + 1);
  }
  for (const p of paragraphs) {
    const isDuplicate = (sigCount.get(p.signature) ?? 0) > 1;
    if (isDuplicate) {
      p.isAnchorCandidate = false;
      continue;
    }
    const t = p.normalizedText.trim();
    const headOutlineOrNumber = paraHeadOutlineOrNumber.get(`${p.section}:${p.paragraph}`) ?? false;
    const structBracket =
      t.length >= STRUCTURAL_ANCHOR_MIN_LEN &&
      t.length < anchorTuning.minTextLen &&
      isStructuralBlockTitleLine(t);
    const structHeading =
      headOutlineOrNumber && t.length >= 3 && t.length < anchorTuning.minTextLen;
    const okNormal = isAnchorTextQualityOk(p.normalizedText, anchorTuning);
    p.isAnchorCandidate = okNormal || structBracket || structHeading;
  }
  const paraStableByPos = new Map<string, string>();
  const paraTextByPos = new Map<string, string>();
  for (const p of paragraphs) {
    paraStableByPos.set(`${p.section}:${p.paragraph}`, p.stableId);
    paraTextByPos.set(`${p.section}:${p.paragraph}`, p.normalizedText.slice(0, 48));
  }

  const controls: CompareControlSnapshot[] = [];
  const shapeDebugRows: Array<{
    source: 'layout' | 'direct';
    sec: number;
    para: number;
    ci: number;
    type: string;
    shapeTextLen: number;
    usedDescription: boolean;
    usedParaFallback: boolean;
    hasPix: boolean;
  }> = [];
  for (let page = 0; page < info.pageCount; page += 1) {
    const layout = wasm.getPageControlLayout(page);
    for (const item of layout.controls) {
      const sec = item.secIdx ?? -1;
      const para = item.paraIdx ?? -1;
      const ci = item.controlIdx ?? -1;
      const paraStableId = sec >= 0 && para >= 0 ? paraStableByPos.get(`${sec}:${para}`) : undefined;
      const key = paraStableId
        ? `sid:${paraStableId}:${ci}:${item.type}`
        : `loc:${sec}:${para}:${ci}:${item.type}`;
      let summary = `${item.type} ${Math.round(item.w)}x${Math.round(item.h)}`;

      try {
        if (item.type === 'table' && sec >= 0 && para >= 0 && ci >= 0) {
          summary = buildTableSummary(wasm, options, sec, para, ci);
        } else if (item.type === 'image' && sec >= 0 && para >= 0 && ci >= 0) {
          const pic = wasm.getPictureProperties(sec, para, ci);
          const w = Math.round(pic.width);
          const h = Math.round(pic.height);
          const crop = [pic.cropLeft, pic.cropTop, pic.cropRight, pic.cropBottom].map((v) => Math.round(v)).join(',');
          const effect = `${pic.effect}:${pic.rotationAngle ?? 0}`;
          const bc = `${pic.brightness ?? 0}/${pic.contrast ?? 0}`;
          const paraText = paraTextByPos.get(`${sec}:${para}`) ?? '';
          const desc = ((pic.description ?? '').trim() || paraText).replaceAll('"', "'").slice(0, 48);
          let pix = 'nopix';
          try {
            const raw = wasm.getControlImageData(sec, para, ci);
            pix = simpleHashBytes(raw);
          } catch {
            pix = 'nopix';
          }
          summary = `image box=${w}x${h} crop=${crop} effect=${effect} bc=${bc} text="${desc || '(없음)'}" pix=${pix}`;
        } else if ((item.type === 'shape' || item.type === 'group') && sec >= 0 && para >= 0 && ci >= 0) {
          const props = wasm.getShapeProperties(sec, para, ci);
          const box = `${Math.round(props.width)}x${Math.round(props.height)}`;
          const rot = Math.round(props.rotationAngle ?? 0);
          const flip = `${props.horzFlip ? 1 : 0}${props.vertFlip ? 1 : 0}`;
          const wrap = props.textWrap ?? 'none';
          const rel = `${props.horzRelTo ?? '-'}:${props.vertRelTo ?? '-'}`;
          const paraText = paraTextByPos.get(`${sec}:${para}`) ?? '';
          let shapeText = '';
          try {
            const st = wasm.getShapeText(sec, para, ci);
            if (st.ok && st.text) shapeText = st.text;
          } catch {
            shapeText = '';
          }
          const desc = (shapeText.trim() || (props.description ?? '').trim() || paraText).replaceAll('"', "'").slice(0, 120);
          const usedDescription = !shapeText.trim() && Boolean((props.description ?? '').trim());
          const usedParaFallback = !shapeText.trim() && !(props.description ?? '').trim() && Boolean(paraText.trim());
          let pix = 'nopix';
          try {
            const raw = wasm.getControlImageData(sec, para, ci);
            pix = simpleHashBytes(raw);
          } catch {
            pix = 'nopix';
          }
          summary = `shape box=${box} rot=${rot} flip=${flip} wrap=${wrap} rel=${rel} text="${desc || '(없음)'}" pix=${pix}`;
          shapeDebugRows.push({
            source: 'layout',
            sec,
            para,
            ci,
            type: item.type,
            shapeTextLen: shapeText.trim().length,
            usedDescription,
            usedParaFallback,
            hasPix: pix !== 'nopix',
          });
        }
      } catch {
        // 일부 개체 타입은 속성 조회 API가 제한될 수 있다.
      }

      controls.push({
        key,
        type: item.type,
        section: sec,
        paragraph: para,
        summary,
        kind: mapControlKind(item.type, summary),
        anchor: {
          pageIndex: page,
          x: item.x,
          y: item.y,
          width: Math.max(12, item.w),
          height: Math.max(12, item.h),
        },
      });
    }
  }

  // RTMS 계열에서 유효했던 방식과 동일한 취지:
  // 페이지 레이아웃 매핑(item.secIdx/paraIdx)에만 의존하지 말고,
  // 문단의 컨트롤 인덱스를 직접 순회하며 table/shape/image를 식별/요약한다.
  for (const p of paragraphs) {
    const controlCount = wasm.getControlTextPositions(p.section, p.paragraph).length;
    for (let ci = 0; ci < controlCount; ci += 1) {
      try {
        const summary = buildTableSummary(wasm, options, p.section, p.paragraph, ci);
        const key = p.stableId
          ? `sid:${p.stableId}:${ci}:table`
          : `loc:${p.section}:${p.paragraph}:${ci}:table`;
        let anchor = p.anchor ?? { pageIndex: 0, x: 0, y: 0, width: 12, height: 12 };
        try {
          const bbox = wasm.getTableBBox(p.section, p.paragraph, ci);
          anchor = {
            pageIndex: bbox.pageIndex,
            x: bbox.x,
            y: bbox.y,
            width: Math.max(12, bbox.width),
            height: Math.max(12, bbox.height),
          };
        } catch {
          // bbox 조회 실패 시 문단 anchor 유지
        }
        controls.push({
          key,
          type: 'table',
          section: p.section,
          paragraph: p.paragraph,
          summary,
          kind: 'table',
          anchor,
        });
      } catch {
        // getTableDimensions 실패 => table이 아닌 컨트롤
      }

      // 요소별 변경 추적 정책(도형/그림):
      // - shape/image는 layout 경로만 사용해 오매핑을 최소화한다.
      // - direct 경로는 table 전용으로 제한한다.
    }
  }

  const uniqueControls = new Map<string, CompareControlSnapshot>();
  for (const c of controls) {
    const ck = canonicalControlKey(c);
    const prev = uniqueControls.get(ck);
    if (!prev) {
      uniqueControls.set(ck, c);
      continue;
    }
    // 같은 개체로 판정되면 더 품질 높은 스냅샷으로 교체
    if (controlSnapshotQuality(c) > controlSnapshotQuality(prev)) {
      uniqueControls.set(ck, c);
    }
  }

  if (isCompareDebugEnabled()) {
    const rows = shapeDebugRows;
    const bySource = {
      layout: rows.filter((r) => r.source === 'layout').length,
      direct: rows.filter((r) => r.source === 'direct').length,
    };
    const withShapeText = rows.filter((r) => r.shapeTextLen > 0).length;
    const withDescription = rows.filter((r) => r.usedDescription).length;
    const withParaFallback = rows.filter((r) => r.usedParaFallback).length;
    const withPix = rows.filter((r) => r.hasPix).length;
    compareDbg('[shape-text-debug] 수집 요약', {
      total: rows.length,
      bySource,
      withShapeText,
      withDescription,
      withParaFallback,
      withPix,
    });
    compareDbg(
      '[shape-text-debug] 샘플(최대 20)',
      rows.slice(0, 20).map((r) => ({
        src: r.source,
        sec: r.sec,
        para: r.para,
        ci: r.ci,
        type: r.type,
        shapeTextLen: r.shapeTextLen,
        desc: r.usedDescription,
        paraFallback: r.usedParaFallback,
        pix: r.hasPix,
      })),
    );
    compareDbg(
      '[shape-text-debug] 미추출 대상(shapeTextLen=0)',
      rows
        .filter((r) => r.shapeTextLen === 0)
        .map((r) => ({
          src: r.source,
          sec: r.sec,
          para: r.para,
          ci: r.ci,
          type: r.type,
          desc: r.usedDescription,
          paraFallback: r.usedParaFallback,
          pix: r.hasPix,
        })),
    );
  }

  return {
    meta: {
      name: displayName,
      sectionCount: info.sectionCount,
      pageCount: info.pageCount,
      pageDisplayNumbers: Array.from({ length: info.pageCount }, (_, pageIndex) =>
        displayedPageByGlobalPage.get(pageIndex) ?? (pageIndex + 1),
      ),
    },
    paragraphs,
    controls: [...uniqueControls.values()],
  };
}

// ─── 스냅샷 빌더 export (bytes vs 편집기 WASM) ─────────────────────────────────

/** 디스크/외부 바이트 → 별도 WASM 인스턴스로 파싱 (stable_id는 이 인스턴스 세션 기준) */
export async function buildSnapshotFromBytes(
  bytes: Uint8Array,
  fileName: string,
  options: CompareOptions,
): Promise<CompareDocumentSnapshot> {
  const wasm = new WasmBridge();
  await wasm.initialize();
  const info = wasm.loadDocument(bytes, fileName);
  return fillSnapshotFromWasm(wasm, info, fileName, options);
}

/** 편집기에 올라온 문서 그대로 스냅샷 — 이력 비교 시 stable_id 유지 */
export function buildSnapshotFromWasm(
  wasm: WasmBridge,
  displayName: string,
  options: CompareOptions,
): CompareDocumentSnapshot {
  const info = wasm.getDocumentInfo();
  return fillSnapshotFromWasm(wasm, info, displayName, options);
}

/**
 * IR `stable_id` → 문단 스냅샷. 값이 비어 있으면 identity 경로를 쓸 수 없어 null.
 * 동일 stable_id가 여러 번 나오면 `#0,#1,…` occurrence suffix로 키를 유일화한다(빈 문단 다수 문서).
 */
export function buildStableIdMap(snap: CompareDocumentSnapshot): Map<string, CompareParaSnapshot> | null {
  const m = new Map<string, CompareParaSnapshot>();
  const seen = new Map<string, number>();
  for (const p of snap.paragraphs) {
    if (!p.stableId) return null;
    // fallback stable_id가 중복되는 문서(빈 문단 다수 등)도 identity를 사용하기 위해
    // 등장 순서 기반 suffix로 키를 정규화한다.
    const occ = seen.get(p.stableId) ?? 0;
    seen.set(p.stableId, occ + 1);
    const key = occ === 0 ? p.stableId : `${p.stableId}#${occ}`;
    m.set(key, p);
  }
  return m;
}

/** ZWSP·NBSP 등만 남은 문단도 “빈 문단”으로 본다. */
function isEffectivelyEmptyParaNormalized(normalizedText: string): boolean {
  const t = normalizedText
    .replace(/[\u200b-\u200d\ufeff]/g, '')
    .replace(/\u00a0/g, ' ')
    .replace(/\s+/g, ' ')
    .trim();
  return t.length === 0;
}

/** 텍스트·개체 없는 문단: 정렬/스냅샷 노이즈로 추가·삭제만 남는 경우 diff에서 생략한다. */
export function shouldSuppressNoiseParagraphOnly(p: CompareParaSnapshot): boolean {
  return p.controlCount === 0 && isEffectivelyEmptyParaNormalized(p.normalizedText);
}

/** 정렬 스텝에서 빈 고아 문단만 제거해 cleanup 2글자 패턴이 깨지지 않게 한다. */
export function stripNoiseOnlyParagraphAlignSteps(steps: ParagraphAlignStep[]): ParagraphAlignStep[] {
  return steps.filter((s) => {
    if (s.kind === 'r') return !shouldSuppressNoiseParagraphOnly(s.r);
    if (s.kind === 'l') return !shouldSuppressNoiseParagraphOnly(s.l);
    return true;
  });
}

export function formatParaLocTitle(p: { section: number; paragraph: number }): string {
  return `구역 ${p.section}, 문단 ${p.paragraph}`;
}
