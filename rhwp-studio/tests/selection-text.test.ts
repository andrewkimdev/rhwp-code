import test from 'node:test';
import assert from 'node:assert/strict';

import {
  singleParagraphSelectionQuery,
  readSelectionText,
  extractSelectedLabel,
} from '../src/core/selection-text.ts';
import type { DocumentPosition } from '../src/core/types.ts';

function bodyPos(overrides: Partial<DocumentPosition> = {}): DocumentPosition {
  return { sectionIndex: 0, paragraphIndex: 0, charOffset: 0, ...overrides };
}

function cellPos(overrides: Partial<DocumentPosition> = {}): DocumentPosition {
  return {
    sectionIndex: 0,
    paragraphIndex: 0,
    charOffset: 0,
    parentParaIndex: 1,
    controlIndex: 0,
    cellIndex: 2,
    cellParaIndex: 0,
    cellPath: [{ controlIndex: 0, cellIndex: 2, cellParaIndex: 0 }],
    ...overrides,
  };
}

test('singleParagraphSelectionQuery: 본문 같은 문단이면 pos/count 반환', () => {
  const start = bodyPos({ paragraphIndex: 2, charOffset: 3 });
  const end = bodyPos({ paragraphIndex: 2, charOffset: 6 });
  assert.deepEqual(singleParagraphSelectionQuery(start, end), { pos: start, count: 3 });
});

test('singleParagraphSelectionQuery: 본문 다른 문단이면 null', () => {
  const start = bodyPos({ paragraphIndex: 2, charOffset: 3 });
  const end = bodyPos({ paragraphIndex: 3, charOffset: 6 });
  assert.equal(singleParagraphSelectionQuery(start, end), null);
});

test('singleParagraphSelectionQuery: 단일 셀 같은 문단이면 pos/count 반환', () => {
  const start = cellPos({ charOffset: 0 });
  const end = cellPos({ charOffset: 3 });
  assert.deepEqual(singleParagraphSelectionQuery(start, end), { pos: start, count: 3 });
});

test('singleParagraphSelectionQuery: 다른 셀이면 null', () => {
  const start = cellPos({ cellIndex: 2, charOffset: 0 });
  const end = cellPos({ cellIndex: 3, charOffset: 3 });
  assert.equal(singleParagraphSelectionQuery(start, end), null);
});

test('singleParagraphSelectionQuery: 본문↔셀 혼합이면 null', () => {
  const start = bodyPos({ charOffset: 0 });
  const end = cellPos({ charOffset: 3 });
  assert.equal(singleParagraphSelectionQuery(start, end), null);
});

test('singleParagraphSelectionQuery: 중첩 표(cellPath.length>1)면 null', () => {
  const nestedPath = [
    { controlIndex: 0, cellIndex: 2, cellParaIndex: 0 },
    { controlIndex: 1, cellIndex: 0, cellParaIndex: 0 },
  ];
  const start = cellPos({ cellPath: nestedPath, charOffset: 0 });
  const end = cellPos({ cellPath: nestedPath, charOffset: 3 });
  assert.equal(singleParagraphSelectionQuery(start, end), null);
});

test('singleParagraphSelectionQuery: 글상자면 null', () => {
  const start = bodyPos({ charOffset: 0, isTextBox: true });
  const end = bodyPos({ charOffset: 3, isTextBox: true });
  assert.equal(singleParagraphSelectionQuery(start, end), null);
});

test('singleParagraphSelectionQuery: 빈/역방향 범위면 null', () => {
  assert.equal(singleParagraphSelectionQuery(bodyPos({ charOffset: 5 }), bodyPos({ charOffset: 5 })), null);
  assert.equal(singleParagraphSelectionQuery(bodyPos({ charOffset: 5 }), bodyPos({ charOffset: 2 })), null);
});

test('readSelectionText: 본문은 getTextRange를 호출', () => {
  const wasm = {
    getTextRange: (sec: number, para: number, charOffset: number, count: number) =>
      `body:${sec}:${para}:${charOffset}:${count}`,
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
  } as any;
  const pos = bodyPos({ paragraphIndex: 4, charOffset: 1 });
  assert.equal(readSelectionText(wasm, pos, 5), 'body:0:4:1:5');
});

test('readSelectionText: 단일 셀은 getTextInCell을 호출', () => {
  const wasm = {
    getTextInCell: (
      sec: number,
      parentPara: number,
      controlIdx: number,
      cellIdx: number,
      cellParaIdx: number,
      charOffset: number,
      count: number,
    ) => `cell:${sec}:${parentPara}:${controlIdx}:${cellIdx}:${cellParaIdx}:${charOffset}:${count}`,
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
  } as any;
  const pos = cellPos({ charOffset: 2 });
  assert.equal(readSelectionText(wasm, pos, 4), 'cell:0:1:0:2:0:2:4');
});

test('extractSelectedLabel: 앞뒤 공백은 trim, insertPos는 선택 끝', () => {
  const wasm = {
    getTextRange: () => '  신청인  ',
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
  } as any;
  const start = bodyPos({ paragraphIndex: 1, charOffset: 3 });
  const end = bodyPos({ paragraphIndex: 1, charOffset: 10 });
  assert.deepEqual(extractSelectedLabel(wasm, start, end), { text: '신청인', insertPos: end });
});

test('extractSelectedLabel: 공백뿐이면 null', () => {
  const wasm = {
    getTextRange: () => '   ',
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
  } as any;
  const start = bodyPos({ paragraphIndex: 1, charOffset: 3 });
  const end = bodyPos({ paragraphIndex: 1, charOffset: 6 });
  assert.equal(extractSelectedLabel(wasm, start, end), null);
});

test('extractSelectedLabel: 문단 경계를 넘으면 null(wasm을 부르지 않는다)', () => {
  let called = false;
  const wasm = {
    getTextRange: () => {
      called = true;
      return '신청인';
    },
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
  } as any;
  const start = bodyPos({ paragraphIndex: 1, charOffset: 3 });
  const end = bodyPos({ paragraphIndex: 2, charOffset: 6 });
  assert.equal(extractSelectedLabel(wasm, start, end), null);
  assert.equal(called, false);
});
