import test from 'node:test';
import assert from 'node:assert/strict';

import { templateFileName } from '../src/command/template-save-name.ts';

test('HWPX 파일명은 확장자를 유지한 채 _template 접미사를 붙인다', () => {
  assert.equal(templateFileName('신청서.hwpx'), '신청서_template.hwpx');
});

test('HWP/HML 원본은 확장자를 HWPX로 바꾸고 _template 접미사를 붙인다', () => {
  assert.equal(templateFileName('신청서.hwp'), '신청서_template.hwpx');
  assert.equal(templateFileName('신청서.hml'), '신청서_template.hwpx');
});

test('확장자가 없는 파일명도 HWPX 확장자를 붙인다', () => {
  assert.equal(templateFileName('신청서'), '신청서_template.hwpx');
});

test('대문자 확장자도 인식한다', () => {
  assert.equal(templateFileName('신청서.HWPX'), '신청서_template.hwpx');
});

test('이미 _template로 끝나는 파일명은 접미사를 다시 붙이지 않는다 (덮어쓰기 대상 고정)', () => {
  assert.equal(templateFileName('신청서_template.hwpx'), '신청서_template.hwpx');
  assert.equal(templateFileName('신청서_template.hwp'), '신청서_template.hwpx');
  assert.equal(templateFileName('신청서_template'), '신청서_template.hwpx');
});

test('_template 접미사 대소문자와 무관하게 인식하되 원래 대소문자는 보존한다', () => {
  assert.equal(templateFileName('신청서_TEMPLATE.hwpx'), '신청서_TEMPLATE.hwpx');
});

test('_template가 파일명 중간에만 있으면 접미사를 새로 붙인다', () => {
  assert.equal(templateFileName('신청서_template_v2.hwpx'), '신청서_template_v2_template.hwpx');
});
