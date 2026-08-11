import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

import {
  EMBED_HIDDEN_FILE_COMMAND_IDS,
  resolveChromeMode,
  resolveChromeModeRequest,
} from '../src/ui/chrome-mode.ts';

test('chrome 프로파일 리졸버는 full을 기본값으로 두고 embed만 opt-in으로 받는다', () => {
  assert.equal(resolveChromeMode(''), 'full');
  assert.equal(resolveChromeMode('?chrome=embed'), 'embed');
  assert.equal(resolveChromeMode('?chrome=EMBED'), 'embed');
  assert.equal(resolveChromeMode('?chrome=full'), 'full');
  assert.deepEqual(resolveChromeModeRequest(''), { mode: 'full', source: 'default' });
  assert.deepEqual(resolveChromeModeRequest('?chrome=embed'), {
    mode: 'embed',
    source: 'url',
    requested: 'embed',
  });
  assert.deepEqual(resolveChromeModeRequest('?chrome=full'), {
    mode: 'full',
    source: 'url',
    requested: 'full',
  });
});

test('미지원 chrome 값은 full로 폴백하고 이유를 보고한다', () => {
  assert.equal(resolveChromeMode('?chrome=kiosk'), 'full');
  assert.deepEqual(resolveChromeModeRequest('?chrome=kiosk'), {
    mode: 'full',
    source: 'url',
    requested: 'kiosk',
    unsupportedReason: 'unsupportedChromeMode',
  });
  // 빈 값은 명시 요청이 아니라 기본으로 취급한다 (renderer 리졸버와 동일).
  assert.deepEqual(resolveChromeModeRequest('?chrome='), { mode: 'full', source: 'default' });
});

test('chrome 프로파일 리졸버는 저장소로 지속하는 opt-in 경로를 두지 않는다', () => {
  const source = readFileSync(new URL('../src/ui/chrome-mode.ts', import.meta.url), 'utf8');
  assert.equal(source.includes('localStorage'), false);
  assert.equal(source.includes('persistChromeMode'), false);
});

test('embed 프로파일이 걸러내는 커맨드는 전부 fileCommands에 실존한다', () => {
  const commandSource = readFileSync(
    new URL('../src/command/commands/file.ts', import.meta.url),
    'utf8',
  );
  assert.ok(EMBED_HIDDEN_FILE_COMMAND_IDS.length > 0);
  for (const id of EMBED_HIDDEN_FILE_COMMAND_IDS) {
    assert.match(commandSource, new RegExp(`id: '${id}',`), id);
  }
});

test('embed 프로파일도 편집 용지와 제품 정보 표면은 유지한다', () => {
  assert.equal(EMBED_HIDDEN_FILE_COMMAND_IDS.includes('file:page-setup'), false);
  assert.equal(EMBED_HIDDEN_FILE_COMMAND_IDS.includes('file:about'), false);
});

test('main은 embed에서 수명주기 커맨드 등록만 거르고 메뉴는 런타임에 정리한다', () => {
  const mainSource = readFileSync(new URL('../src/main.ts', import.meta.url), 'utf8');
  assert.match(mainSource, /resolveChromeModeRequest\(window\.location\.search\)/);
  assert.match(mainSource, /fileCommands\.filter\(/);
  assert.match(mainSource, /EMBED_HIDDEN_FILE_COMMAND_IDS/);
  assert.match(mainSource, /pruneEmbedFileMenu\(\)/);
  // 자동저장 복구 다이얼로그의 드래프트 복원도 호스트가 감지할 수 없는 문서 교체
  // 경로이므로 embed에서는 띄우지 않는다.
  assert.match(
    mainSource,
    /if \(chromeMode !== 'embed'\) void offerAutosaveRecoveryIfIdle\(\);/,
  );
  // index.html은 수정하지 않는다 — 기본 full 프로파일의 정적 마크업 검사가 그대로 유효하다.
  const indexHtml = readFileSync(new URL('../index.html', import.meta.url), 'utf8');
  assert.match(indexHtml, /data-cmd="file:save"/);
  assert.match(indexHtml, /data-recent/);
});

test('shortcut-map은 embed 프로파일과 무관하게 파일 단축키 매핑을 유지한다', () => {
  // 매핑이 남아야 Ctrl+S/Ctrl+P가 preventDefault로 계속 삼켜져 브라우저 저장/인쇄
  // 대화상자로 빠지지 않고, Ctrl+Shift+S가 후순위 table:block-sum 매핑으로
  // 폴스루하지 않는다. 미등록 커맨드 dispatch는 무해하게 false를 반환한다.
  const shortcutSource = readFileSync(
    new URL('../src/command/shortcut-map.ts', import.meta.url),
    'utf8',
  );
  assert.match(shortcutSource, /'file:save'\]/);
  assert.match(shortcutSource, /'file:save-as'\]/);
  assert.match(shortcutSource, /'file:print'\]/);
  assert.doesNotMatch(shortcutSource, /chrome-mode|ChromeMode|EMBED_HIDDEN/);
});
