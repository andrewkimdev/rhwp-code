/** 정규화·해시·DiffID·컨트롤 키 정규화·표 요약 파싱·정밀 컨트롤 diff — 두 비교 경로가 공유하는 시그니처 유틸 모음. */
import type { WasmBridge } from '@/core/wasm-bridge';
import type { AlignedPair } from './align-core';
import type {
  CompareControlSnapshot,
  CompareOptions,
  CompareParaSnapshot,
  DiffItem,
  DiffKind,
} from './types';

// ─── 정규화·해시·Diff ID (문단 시그니처·컨트롤 요약에 공통 사용) ───────────────

/** 비교 옵션에 따른 문단 텍스트 정규화. `ignoreWhitespace`/`caseSensitive`는 여기서만 일괄 적용된다. */
export function normalizeText(text: string, options: CompareOptions): string {
  const base = options.ignoreWhitespace ? text.replace(/\s+/g, ' ').trim() : text;
  return options.caseSensitive ? base : base.toLowerCase();
}

/** 짧은 FNV-1a digest — 문단 시그니처·표 텍스트 digest 등 충돌 가능성은 있으나 비교용으론 충분 */
export function simpleHash(input: string): string {
  let h = 2166136261;
  for (let i = 0; i < input.length; i += 1) {
    h ^= input.charCodeAt(i);
    h = Math.imul(h, 16777619);
  }
  return (h >>> 0).toString(16);
}

/** 바이너리(이미지 픽셀 등)용 FNV-1a 계열 해시 — 짧은 digest 문자열로 요약 비교에 사용 */
export function simpleHashBytes(bytes: Uint8Array): string {
  let h = 2166136261;
  for (let i = 0; i < bytes.length; i += 1) {
    h ^= bytes[i];
    h = Math.imul(h, 16777619);
  }
  return (h >>> 0).toString(16);
}

/** DiffItem.id — kind와 고유 키를 합쳐 UI 목록·로그·reflow 필터(`id-moved:` 등)에서 항목을 식별 */
export function mkDiffId(kind: DiffKind, key: string): string {
  return `${kind}:${key}`;
}

/**
 * WASM `item.type` + 요약 문자열을 DiffKind로 정규화.
 * chart는 summary 키워드로 shape/group과 구분한다(도형 안 차트 등).
 */
export function mapControlKind(type: string, summary: string): DiffKind {
  if (type === 'table') return 'table';
  if (type === 'image') return 'image';
  if (type === 'shape' || type === 'group') {
    if (summary.toLowerCase().includes('chart')) return 'chart';
    return 'shape';
  }
  if (summary.toLowerCase().includes('chart')) return 'chart';
  return 'paragraphMeta';
}

/**
 * 동일 실개체가 `group`↔`shape`처럼 타입만 바뀌며 중복 수집되는 경우를 한 키로 묶는다.
 * 매칭/중복 제거(`uniqueControls`)의 기준이 되므로 stem(`sid:…` 또는 `loc:…`) 추출 규칙을 바꿀 때는
 * 레이아웃 경로·문단 직접 경로 양쪽의 key 포맷을 함께 점검해야 한다.
 */
export function canonicalControlKey(c: CompareControlSnapshot): string {
  const m = c.key.match(/^(sid:[^:]+:\d+|loc:-?\d+:-?\d+:\d+):[^:]+$/);
  if (!m) return `${c.kind}::${c.key}`;
  const stem = m[1];
  // 요소별 변경 추적 안정화:
  // - 동일 anchor(control index)인데 type만 group/shape로 달라지는 중복을 하나로 합친다.
  // - 매칭 단계에서는 canonical key를 기준으로 "같은 개체"를 판정한다.
  if (c.kind === 'shape' || c.kind === 'chart') return `${c.kind}::${stem}:shape`;
  if (c.kind === 'table') return `${c.kind}::${stem}:table`;
  return `${c.kind}::${stem}:image`;
}

/** 동일 canonical 키로 여러 스냅샷이 들어왔을 때, UI에 더 유의미한 요약을 남기기 위한 휴리스틱 점수 */
export function controlSnapshotQuality(c: CompareControlSnapshot): number {
  let score = 0;
  // 요소별 변경 추적 품질 점수:
  // - 텍스트/픽셀해시/유효 bbox가 있는 스냅샷을 우선 채택해
  //   동일 key 중 더 "정보가 풍부한" 항목을 남긴다.  
  if (c.summary.includes('text="') && !c.summary.includes('text="(없음)"')) score += 4;
  if (!c.summary.includes('pix=nopix')) score += 2;
  if (!c.summary.includes('nobox')) score += 1;
  if (c.type === 'shape') score += 1; // group보다 shape 상세값이 많은 편
  return score;
}

/** DiffKind → 짧은 한글 제목(컨트롤 추가/삭제 카드 등). UI `kindLabel`과 문구를 맞출 때 동기화 */
export function kindLabel(kind: DiffKind): string {
  if (kind === 'table') return '표';
  if (kind === 'shape') return '도형';
  if (kind === 'image') return '이미지';
  if (kind === 'chart') return '그래프';
  if (kind === 'text') return '텍스트';
  return '메타';
}

/**
 * 표 셀 텍스트·속성을 요약한 짧은 문자열 — 컨트롤 diff에서 "같은 표인지" 판별용.
 * WASM에 `getTableSignature`가 있으면 그걸 우선(정확·빠름), 없으면 셀 순회 폴백.
 */
export function buildTableSummary(
  wasm: WasmBridge,
  options: CompareOptions,
  sec: number,
  para: number,
  ci: number,
): string {
  let sigDigest = 'nosig';
  try {
    const sigJson = wasm.getTableSignature(sec, para, ci);
    sigDigest = simpleHash(sigJson);
  } catch {
    // 신/구 WASM 호환: 시그니처 API가 없으면 기존 JS 조합 경로 사용
  }

  const dim = wasm.getTableDimensions(sec, para, ci);
  const cellCount = Math.max(0, dim.rowCount * dim.colCount);
  const cellSnippets: string[] = [];
  const cellPreviewPairs: string[] = [];
  const cellHashPairs: string[] = [];
  for (let cellIdx = 0; cellIdx < cellCount; cellIdx += 1) {
    let paraCount = 0;
    try {
      paraCount = wasm.getCellParagraphCount(sec, para, ci, cellIdx);
    } catch {
      paraCount = 0;
    }
    const paraTexts: string[] = [];
    for (let cpi = 0; cpi < paraCount; cpi += 1) {
      try {
        const plen = wasm.getCellParagraphLength(sec, para, ci, cellIdx, cpi);
        const t = plen > 0 ? wasm.getTextInCell(sec, para, ci, cellIdx, cpi, 0, plen) : '';
        if (t) paraTexts.push(normalizeText(t, options));
      } catch {
        // 일부 셀 접근 실패 시 해당 문단만 스킵
      }
    }
    const joined = paraTexts.join('|');
    cellSnippets.push(joined);
    if (joined && cellPreviewPairs.length < 240) {
      // 요소별 변경 추적(표 텍스트):
      // - cprev: UI 표시용(사람이 읽는 셀 미리보기)
      // - csha : 긴 문장/여러 줄에서 잘림이 있어도 변경 감지를 유지하기 위한 셀 단위 해시
      const compact = joined
        .replaceAll('"', "'")
        .replaceAll('\r\n', '\n')
        .replaceAll('\n', ' ↵ ')
        .replace(/\s{2,}/g, ' ')
        .trim()
        .slice(0, 180);
      const row = dim.colCount > 0 ? Math.floor(cellIdx / dim.colCount) + 1 : 1;
      const col = dim.colCount > 0 ? (cellIdx % dim.colCount) + 1 : (cellIdx + 1);
      const key = `r${row}c${col}`;
      cellPreviewPairs.push(`${key}=${encodeURIComponent(compact)}`);
      cellHashPairs.push(`${key}=${simpleHash(joined)}`);
    }
  }
  const textDigest = simpleHash(cellSnippets.join('||'));
  const textPreview = cellSnippets
    .filter(Boolean)
    .slice(0, 2)
    .map((s) => s.replaceAll('"', "'").replace(/\s+/g, ' ').trim())
    .join(' | ')
    .slice(0, 80);
  let propsDigest = '';
  try {
    const props = wasm.getTableProperties(sec, para, ci);
    propsDigest = simpleHash(JSON.stringify(props));
  } catch {
    propsDigest = 'noprops';
  }
  let bboxDigest = 'nobox';
  try {
    const bbox = wasm.getTableBBox(sec, para, ci);
    bboxDigest = `${Math.round(bbox.width)}x${Math.round(bbox.height)}`;
  } catch {
    bboxDigest = 'nobox';
  }
  const cellPreview = cellPreviewPairs.join('&');
  const cellHash = cellHashPairs.join('&');
  return `table r=${dim.rowCount} c=${dim.colCount} tprev="${textPreview || '(없음)'}" cprev="${cellPreview || '(없음)'}" csha="${cellHash || '(없음)'}" txt=${textDigest} props=${propsDigest} box=${bboxDigest} sig=${sigDigest}`;
}

// ─── 표 요약 파싱·셀 단위 변경 집계 (buildGranularControlDiffs에서 사용) ───────

/** `sid:…:ci:type` / `loc:…:ci:type` 형태에서 컨트롤 인덱스 `ci`만 뽑아 폴백 점수에 사용 */
export function extractControlIndexFromKey(key: string): number | null {
  const m = key.match(/:(\d+):[^:]+$/);
  if (!m) return null;
  return Number.isFinite(Number(m[1])) ? Number(m[1]) : null;
}

/** 스냅샷 문단·컨트롤의 `section`+`paragraph`를 하나의 맵 키로 묶을 때 사용한다. */
export function paraPosKey(p: { section: number; paragraph: number }): string {
  return `${p.section}:${p.paragraph}`;
}

/**
 * 문단 정렬 결과(`AlignedPair[]`, cleanup 이전)에서 오른쪽 문단 위치 → 짝 왼쪽 문단을 추출한다.
 * - 키: `paraPosKey(right)` (오른쪽 HWP 좌표계).
 * - 값: DP/그리디가 같은 논리 슬롯으로 판단한 `left` 문단.
 * `(null, right)`·`(left, null)` 삽입/삭제 축은 맵에 넣지 않는다 → 해당 문단의 컨트롤은 슬롯 매칭 대상에서 제외되고
 * 이후 `extractTablePatiencePins`(선행) → `pairAlignmentSlotControls` → `pairControlsFallback` / added·removed로 처리된다.
 */
export function buildRightToLeftParaMapFromAligned(aligned: AlignedPair[]): Map<string, CompareParaSnapshot> {
  const m = new Map<string, CompareParaSnapshot>();
  for (const { left, right } of aligned) {
    if (!left || !right) continue;
    m.set(paraPosKey(right), left);
  }
  return m;
}

/** `buildTableSummary` 등이 만든 `key=value` 나열을 Record로 파싱. 값에 따옴표가 있으면 제거한다. */
export function parseSummaryKV(summary: string): Record<string, string> {
  const out: Record<string, string> = {};
  for (const m of summary.matchAll(/([a-z]+)=("([^"]*)"|[^\s]+)/g)) {
    const raw = m[2] ?? '';
    out[m[1]] = raw.startsWith('"') && raw.endsWith('"') ? raw.slice(1, -1) : raw;
  }
  return out;
}

/**
 * `csha="r1c1=…&r2c2=…"` 형태(셀별 해시)를 키 단위로 비교해 변경된 셀 개수를 센다.
 * 행/열 구조가 바뀌어 키 집합이 달라져도 union 키 기준으로 added/removed 셀을 모두 잡는다.
 */
function countChangedCellsByHash(leftHash: string, rightHash: string): number {
  const parse = (v: string): Map<string, string> => {
    const out = new Map<string, string>();
    if (!v || v === '(없음)') return out;
    for (const pair of v.split('&')) {
      const i = pair.indexOf('=');
      if (i <= 0) continue;
      const key = pair.slice(0, i);
      const value = pair.slice(i + 1);
      out.set(key, value);
    }
    return out;
  };
  const l = parse(leftHash);
  const r = parse(rightHash);
  const keys = new Set<string>([...l.keys(), ...r.keys()]);
  let changed = 0;
  for (const k of keys) {
    if ((l.get(k) ?? '') !== (r.get(k) ?? '')) changed += 1;
  }
  return changed;
}

/**
 * 동일 컨트롤 쌍(l,r)에 대해 가능한 한 잘게 쪼갠 DiffItem[]을 만든다.
 * - 표: 행/열·크기·셀 텍스트(cprev/csha)·속성(props)을 분리해 카드가 비지 않게 한다.
 * - 이미지/도형: 크기·텍스트·자르기·효과 등 필드별 push.
 * - `push` 내부에서 좌우 문자열이 같으면(강제 제외 아닌 이상) 항목을 생략한다.
 */
export function buildGranularControlDiffs(
  l: CompareControlSnapshot,
  r: CompareControlSnapshot,
  idStem: string,
): DiffItem[] {
  const lk = parseSummaryKV(l.summary);
  const rk = parseSummaryKV(r.summary);
  const items: DiffItem[] = [];
  const label = kindLabel(l.kind);
  const push = (suffix: string, title: string, leftPreview: string, rightPreview: string, force = false) => {
    if (!force && leftPreview === rightPreview) return;
    items.push({
      id: mkDiffId(l.kind, `${idStem}:${suffix}`),
      kind: l.kind,
      severity: 'modified',
      path: { section: r.section, paragraph: r.paragraph, controlKey: r.key },
      title,
      leftPreview,
      rightPreview,
      leftAnchor: l.anchor,
      rightAnchor: r.anchor,
    });
  };

  if (l.type === 'table' && r.type === 'table') {
    const hasCellText =
      (lk.cprev && lk.cprev !== '(없음)') ||
      (rk.cprev && rk.cprev !== '(없음)') ||
      (lk.tprev && lk.tprev !== '(없음)') ||
      (rk.tprev && rk.tprev !== '(없음)');
    const tableLabel = hasCellText ? '표' : '테이블';
    const rowsColsChanged = (lk.r ?? '') !== (rk.r ?? '') || (lk.c ?? '') !== (rk.c ?? '');
    push('rows-cols', `${tableLabel} 행/열 변경`, `r=${lk.r ?? '(없음)'} c=${lk.c ?? '(없음)'}`, `r=${rk.r ?? '(없음)'} c=${rk.c ?? '(없음)'}`);
    push('size', `${tableLabel} 크기 변경`, `box=${lk.box ?? '(없음)'}`, `box=${rk.box ?? '(없음)'}`);
    // UI의 행/열별 셀 비교는 cprev(r1c1=...&r1c2=...) 포맷을 기준으로 동작한다.
    // cprev가 없을 때만 tprev로 폴백한다.
    const lText = `cprev="${lk.cprev ?? lk.tprev ?? '(없음)'}"`;
    const rText = `cprev="${rk.cprev ?? rk.tprev ?? '(없음)'}"`;
    const tableTextChanged = (lk.txt ?? '') !== (rk.txt ?? '') || lText !== rText;
    const changedCells = countChangedCellsByHash(lk.csha ?? '', rk.csha ?? '');
    const textTitle = rowsColsChanged
      ? `${tableLabel} 텍스트 변경(구조변경 동반${changedCells > 0 ? `, ${changedCells}셀` : ''})`
      : `${tableLabel} 텍스트 변경${changedCells > 0 ? `(${changedCells}셀)` : ''}`;
    push('text', textTitle, lText, rText, tableTextChanged);
    // props= 는 `getTableProperties` 전체 JSON 해시라 조판·저장 경로만 달라도 달라져 노이즈가 크다. UI에는 내리지 않는다.
    return items;
  }

  if (l.type === 'image' && r.type === 'image') {
    push('size', '그림 크기 변경', `box=${lk.box ?? '(없음)'}`, `box=${rk.box ?? '(없음)'}`);
    const imageTextChanged = (lk.text ?? '') !== (rk.text ?? '') || ((lk.pix ?? '') !== (rk.pix ?? '') && (lk.text ?? '') === (rk.text ?? ''));
    push('text', '그림 텍스트 변경', `text="${lk.text ?? '(없음)'}" pix=${lk.pix ?? '(없음)'}`, `text="${rk.text ?? '(없음)'}" pix=${rk.pix ?? '(없음)'}`, imageTextChanged);
    push('crop', '그림 자르기 변경', `crop=${lk.crop ?? '(없음)'}`, `crop=${rk.crop ?? '(없음)'}`);
    push('effect', '그림 효과 변경', `effect=${lk.effect ?? '(없음)'} bc=${lk.bc ?? '(없음)'}`, `effect=${rk.effect ?? '(없음)'} bc=${rk.bc ?? '(없음)'}`);
    return items;
  }

  if ((l.type === 'shape' || l.type === 'group') && (r.type === 'shape' || r.type === 'group')) {
    push('size', `${label} 크기 변경`, `box=${lk.box ?? '(없음)'}`, `box=${rk.box ?? '(없음)'}`);
    const shapeTextChanged = (lk.text ?? '') !== (rk.text ?? '') || ((lk.pix ?? '') !== (rk.pix ?? '') && (lk.text ?? '') === (rk.text ?? ''));
    push('text', `${label} 텍스트 변경`, `text="${lk.text ?? '(없음)'}" pix=${lk.pix ?? '(없음)'}`, `text="${rk.text ?? '(없음)'}" pix=${rk.pix ?? '(없음)'}`, shapeTextChanged);
    push('rotate', `${label} 회전/대칭 변경`, `rot=${lk.rot ?? '(없음)'} flip=${lk.flip ?? '(없음)'}`, `rot=${rk.rot ?? '(없음)'} flip=${rk.flip ?? '(없음)'}`);
    push('layout', `${label} 배치 변경`, `wrap=${lk.wrap ?? '(없음)'} rel=${lk.rel ?? '(없음)'}`, `wrap=${rk.wrap ?? '(없음)'} rel=${rk.rel ?? '(없음)'}`);
    return items;
  }

  push('generic', `${label} 속성 변경`, l.summary, r.summary);
  return items;
}
