import test from 'node:test';
import assert from 'node:assert/strict';

import { resolveUniqueName } from '../src/core/field-name-dedup.ts';

test('resolveUniqueName: 충돌 없으면 원래 이름 그대로', () => {
  assert.equal(resolveUniqueName('신청인', new Set(), new Set()), '신청인');
});

test('resolveUniqueName: existingNames와 충돌하면 _2', () => {
  assert.equal(resolveUniqueName('전화번호', new Set(['전화번호']), new Set()), '전화번호_2');
});

test('resolveUniqueName: usedInBatch와 충돌해도 동일하게 접미어', () => {
  assert.equal(resolveUniqueName('전화번호', new Set(), new Set(['전화번호'])), '전화번호_2');
});

test('resolveUniqueName: 둘 다와 충돌하면 겹치지 않는 접미어까지 건너뛴다', () => {
  const existing = new Set(['전화번호', '전화번호_2']);
  const usedInBatch = new Set(['전화번호_3']);
  assert.equal(resolveUniqueName('전화번호', existing, usedInBatch), '전화번호_4');
});

test('resolveUniqueName: 연쇄 충돌 시 _2, _3, ... 순서로 증가', () => {
  const existing = new Set(['x', 'x_2', 'x_3', 'x_4']);
  assert.equal(resolveUniqueName('x', existing, new Set()), 'x_5');
});
