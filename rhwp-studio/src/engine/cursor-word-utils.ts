/**
 * 커서 단어/문장 경계 탐색 순수 유틸 — cursor.ts 에서 분리(PR #794/#811/#839).
 * 상태 없는 함수 무리: 입력 텍스트와 오프셋만 받아 경로를 계산한다.
 */

// ─── 단어 경계 탐색 유틸 (PR #794, Alt+Arrow 단어 이동) ──────────────────────────────────

const enum CharClass { Space, Hangul, Latin, Digit, Punct }

function classifyChar(ch: string): CharClass {
  const c = ch.charCodeAt(0);
  if (c === 0x20 || c === 0x09 || c === 0x0A || c === 0x0D || c === 0xA0) return CharClass.Space;
  if (c >= 0xAC00 && c <= 0xD7AF) return CharClass.Hangul; // 완성형
  if (c >= 0x3131 && c <= 0x318E) return CharClass.Hangul; // 자모
  if (c >= 0x1100 && c <= 0x11FF) return CharClass.Hangul; // 첫가끝
  if ((c >= 0x41 && c <= 0x5A) || (c >= 0x61 && c <= 0x7A)) return CharClass.Latin;
  if (c >= 0x30 && c <= 0x39) return CharClass.Digit;
  return CharClass.Punct;
}

export function findWordBoundaryForward(text: string): number {
  if (text.length === 0) return 0;
  const startClass = classifyChar(text[0]);
  let i = 0;
  // Skip current word (same class)
  if (startClass === CharClass.Space) {
    while (i < text.length && classifyChar(text[i]) === CharClass.Space) i++;
  } else {
    while (i < text.length && classifyChar(text[i]) === startClass) i++;
    // Also skip trailing spaces
    while (i < text.length && classifyChar(text[i]) === CharClass.Space) i++;
  }
  return i || 1;
}

export function findWordBoundaryBackward(text: string): number {
  if (text.length === 0) return 0;
  let i = text.length;
  const endClass = classifyChar(text[i - 1]);
  // Skip trailing spaces
  if (endClass === CharClass.Space) {
    while (i > 0 && classifyChar(text[i - 1]) === CharClass.Space) i--;
  }
  if (i === 0) return 0;
  // Skip the word (same class)
  const wordClass = classifyChar(text[i - 1]);
  while (i > 0 && classifyChar(text[i - 1]) === wordClass) i--;
  return i;
}

// ─── 단어 범위 탐색 유틸 (PR #811, F3 단계 1 단어 선택) ──────────────────────────────────

function isWordChar(c: string): boolean {
  const code = c.charCodeAt(0);
  if (code >= 0x30 && code <= 0x39) return true; // digit
  if (code >= 0x41 && code <= 0x5A) return true; // A-Z
  if (code >= 0x61 && code <= 0x7A) return true; // a-z
  if (code >= 0xAC00 && code <= 0xD7AF) return true; // Hangul
  if (code >= 0x3131 && code <= 0x318E) return true; // Hangul Jamo
  return false;
}

export function findWordAt(text: string, offset: number): { start: number; end: number } {
  if (!text || offset >= text.length) return { start: offset, end: offset };
  const atWord = isWordChar(text[offset] ?? '');
  let start = offset;
  let end = offset;
  if (atWord) {
    while (start > 0 && isWordChar(text[start - 1])) start--;
    while (end < text.length && isWordChar(text[end])) end++;
  } else {
    while (start > 0 && !isWordChar(text[start - 1])) start--;
    while (end < text.length && !isWordChar(text[end])) end++;
  }
  return { start, end };
}

// ─── 문장 범위 탐색 유틸 (#839, F3 단계 2 문장 선택) ──────────────────────────────────

const SENTENCE_TERMINATORS = new Set(['.', '?', '!', '。', '？', '！']);

export function findSentenceAt(text: string, offset: number): { start: number; end: number } {
  if (!text) return { start: offset, end: offset };
  const len = text.length;
  const clampedOffset = Math.min(offset, len);

  let start = clampedOffset;
  while (start > 0) {
    const prev = text[start - 1];
    if (SENTENCE_TERMINATORS.has(prev)) break;
    start--;
  }
  while (start < clampedOffset && (text[start] === ' ' || text[start] === '\t')) start++;

  let end = clampedOffset;
  while (end < len) {
    if (SENTENCE_TERMINATORS.has(text[end])) { end++; break; }
    end++;
  }

  return { start, end };
}
