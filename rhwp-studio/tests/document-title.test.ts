import test from 'node:test';
import assert from 'node:assert/strict';

import { documentTitleFor } from '../src/app/document-title.ts';

test('documentTitleFor: clean 문서는 파일명에 앱 접미를 붙인다', () => {
  assert.equal(documentTitleFor('보고서.hwp', false), '보고서.hwp | rhwp');
  assert.equal(documentTitleFor('report.hwpx', false), 'report.hwpx | rhwp');
  assert.equal(documentTitleFor('새 문서.hwp', false), '새 문서.hwp | rhwp');
});

test('documentTitleFor: dirty 문서는 파일명 뒤에 * 마커를 붙인다', () => {
  assert.equal(documentTitleFor('보고서.hwp', true), '보고서.hwp* | rhwp');
  assert.equal(documentTitleFor('새 문서.hwp', true), '새 문서.hwp* | rhwp');
});

test('documentTitleFor: 빈 파일명이어도 접미 형식을 유지한다', () => {
  assert.equal(documentTitleFor('', false), ' | rhwp');
});
