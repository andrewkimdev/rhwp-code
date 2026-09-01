import test from 'node:test';
import assert from 'node:assert/strict';

import { analyzeDocumentFonts } from '../src/core/document-font-status.ts';

test('저장된 로컬 snapshot이 없으면 미확인 글꼴에 로컬 확인 필요 상태를 부여한다', () => {
  const report = analyzeDocumentFonts(['미설치원본', '함초롬바탕'], {
    localFonts: [],
    localSupported: true,
    localSnapshotLoaded: true,
    localSnapshotStored: false,
  });

  assert.equal(report.shouldPromptLocalAccess, true);
  assert.deepEqual(report.summary, {
    available: 1,
    needsLocalCheck: 1,
    webSubstitute: 0,
    missing: 0,
  });
  assert.deepEqual(report.fonts.map(f => [f.fontName, f.status]), [
    ['미설치원본', 'needs-local-check'],
    ['함초롬바탕', 'available'],
  ]);
});

test('저장된 로컬 snapshot에 있는 원본 글꼴은 사용 가능 상태가 된다', () => {
  const report = analyzeDocumentFonts(['문서원본', '없는글꼴'], {
    localFonts: ['문서원본'],
    localSupported: true,
    localSnapshotLoaded: true,
    localSnapshotStored: true,
  });

  assert.equal(report.shouldPromptLocalAccess, false);
  assert.deepEqual(report.fonts.map(f => [f.fontName, f.status, f.source]), [
    ['문서원본', 'available', 'local'],
    ['없는글꼴', 'missing', 'unknown'],
  ]);
});

test('문서 후보 probe snapshot은 아직 확인하지 않은 새 글꼴에 다시 prompt를 띄운다', () => {
  const report = analyzeDocumentFonts(['문서원본', '확인된미설치', '새글꼴'], {
    localFonts: ['문서원본'],
    localSupported: true,
    localSnapshotLoaded: true,
    localSnapshotStored: true,
    localSnapshotComplete: false,
    localSnapshotSource: 'font-presence-probe',
    localCheckedFonts: ['문서원본', '확인된미설치'],
    detectionMethod: 'font-presence-probe',
  });

  assert.equal(report.shouldPromptLocalAccess, true);
  assert.deepEqual(report.fonts.map(f => [f.fontName, f.status, f.source]), [
    ['문서원본', 'available', 'local'],
    ['새글꼴', 'needs-local-check', 'unknown'],
    ['확인된미설치', 'missing', 'unknown'],
  ]);
});

test('로컬 감지 미지원 환경에서는 prompt 없이 웹 대체와 누락을 구분한다', () => {
  const report = analyzeDocumentFonts(['휴먼명조', '없는글꼴'], {
    localFonts: [],
    localSupported: false,
    localSnapshotLoaded: true,
    localSnapshotStored: false,
  });

  assert.equal(report.shouldPromptLocalAccess, false);
  assert.deepEqual(report.fonts.map(f => [f.fontName, f.status, f.substituteFont]), [
    ['없는글꼴', 'missing', null],
    ['휴먼명조', 'web-substitute', 'HY신명조'],
  ]);
});

test('한양* 글꼴은 HY* 동일 서체 별칭으로 로컬 확인 prompt 없이 웹 대체된다', () => {
  // 4914206.hwp 재현 시나리오 — 한양견고딕/한양중고딕 문서를 로컬 폰트 없는
  // 기기에서 열면 종전엔 needs-local-check 로 다이얼로그가 떴다. 한양X ≡ HYX
  // (g_SubstFonts 1순위 쌍)이고 HY*는 등록 웹폰트이므로 곧바로 대체 연결한다.
  const report = analyzeDocumentFonts(['굴림', '바탕', '한양견고딕', '한양중고딕'], {
    localFonts: [],
    localSupported: true,
    localSnapshotLoaded: true,
    localSnapshotStored: false,
  });

  assert.equal(report.shouldPromptLocalAccess, false);
  assert.deepEqual(report.summary, {
    available: 2,
    needsLocalCheck: 0,
    webSubstitute: 2,
    missing: 0,
  });
  assert.deepEqual(report.fonts.map(f => [f.fontName, f.status, f.substituteFont]), [
    ['굴림', 'available', null],
    ['바탕', 'available', null],
    ['한양견고딕', 'web-substitute', 'HY견고딕'],
    ['한양중고딕', 'web-substitute', 'HY중고딕'],
  ]);
});

test('한양* 대응 HY*가 로컬에 설치돼 있으면 available 로 분류한다', () => {
  const report = analyzeDocumentFonts(['한양견고딕', '미설치원본'], {
    localFonts: ['HY견고딕'],
    localSupported: true,
    localSnapshotLoaded: true,
    localSnapshotStored: true,
  });

  assert.deepEqual(report.fonts.map(f => [f.fontName, f.status, f.source]), [
    ['미설치원본', 'missing', 'unknown'],
    ['한양견고딕', 'available', 'local'],
  ]);
});

test('대응하는 HY*가 어디에도 없는 한양* 글꼴은 기존 치환 체인을 그대로 따른다', () => {
  // HY궁서는 등록 웹폰트가 아니므로 identity 규칙이 발화하지 않는다 —
  // 한양궁서 → 궁서 치환(g_SubstFonts)이 기존처럼 이어진다.
  const report = analyzeDocumentFonts(['한양궁서'], {
    localFonts: [],
    localSupported: false,
    localSnapshotLoaded: true,
    localSnapshotStored: false,
  });

  const item = report.fonts.find(f => f.fontName === '한양궁서');
  assert.ok(item);
  assert.equal(item.status, 'web-substitute');
  assert.equal(item.substituteFont, '궁서');
});
