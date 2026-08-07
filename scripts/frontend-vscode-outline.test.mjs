/**
 * VS Code 뷰어 개요 패널의 키보드 경로 계약.
 *
 * PR #4093 리뷰 지적: 접기/펼치기 `<button>` 에 초점을 두고 Enter/Space 를 누르면
 * keydown 이 부모 개요 항목으로 전파돼 `navigateToOutline()` 이 실행되고, 부모가 부른
 * `preventDefault()` 가 버튼의 기본 click 까지 억제한다 — 키보드만으로는 접기/펼치기가
 * 동작하지 않았다.
 *
 * rhwp-vscode 에는 DOM 을 띄우는 테스트 러너가 없어(webview 는 webpack 번들 스크립트)
 * `scripts/frontend-extension-dist.test.mjs` 와 같은 소스 계약 방식으로 가드한다 —
 * 부모 handler 가 자기 자신이 초점일 때만 동작하고, toggle 은 전파를 막는지 확인한다.
 */
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

const ROOT = path.resolve(fileURLToPath(new URL('..', import.meta.url)));
const VIEWER = path.join(ROOT, 'rhwp-vscode/src/webview/viewer.ts');

/** `marker` 로 시작하는 블록을 중괄호 짝을 맞춰 잘라낸다. */
function sliceBlock(source, marker) {
  const start = source.indexOf(marker);
  assert.notEqual(start, -1, `${marker} 를 viewer.ts 에서 찾지 못했다`);
  const open = source.indexOf('{', start);
  assert.notEqual(open, -1, `${marker} 뒤에 블록이 없다`);

  let depth = 0;
  for (let i = open; i < source.length; i += 1) {
    if (source[i] === '{') depth += 1;
    else if (source[i] === '}') {
      depth -= 1;
      if (depth === 0) return source.slice(start, i + 1);
    }
  }
  throw new Error(`${marker} 블록의 끝을 찾지 못했다`);
}

const viewerSource = readFileSync(VIEWER, 'utf8');
const renderOutlineTree = sliceBlock(viewerSource, 'function renderOutlineTree');

/** 줄 주석을 걷어낸 코드. 주석에 적힌 낱말이 순서 검사를 흐리지 않게 한다. */
function withoutLineComments(source) {
  return source.replace(/^\s*\/\/.*$/gm, '');
}

test('개요 항목 keydown 은 항목 자신이 초점일 때만 이동한다', () => {
  const handler = withoutLineComments(
    sliceBlock(renderOutlineTree, 'item.addEventListener("keydown"'),
  );

  const guard = handler.search(/event\.target\s*!==\s*item\s*\)\s*return;/);
  assert.notEqual(
    guard,
    -1,
    'keydown handler 에 `event.target !== item` 조기 반환 가드가 없다 — '
      + '접기/펼치기 버튼의 Enter/Space 가 이동으로 흘러간다',
  );

  const prevented = handler.indexOf('preventDefault');
  assert.notEqual(prevented, -1, 'keydown handler 가 preventDefault 를 호출하지 않는다');
  assert.ok(
    guard < prevented,
    'preventDefault 보다 뒤에 가드가 있으면 버튼 기본 click 이 이미 취소된다',
  );
});

test('접기/펼치기 toggle click 은 개요 항목으로 전파되지 않는다', () => {
  const handler = sliceBlock(renderOutlineTree, 'toggle.addEventListener("click"');

  assert.match(
    handler,
    /event\.stopPropagation\(\)/,
    'toggle click 이 전파를 막지 않으면 접을 때 이동까지 함께 일어난다',
  );
});

test('토글 뒤 초점을 같은 항목의 버튼으로 되돌린다', () => {
  // buildOutline() 이 패널을 다시 그리면서 초점 버튼이 사라진다. 되돌리지 않으면
  // activeElement 가 body 로 떨어져 두 번째 Enter/Space 부터 죽는다 (headless 재현).
  const handler = withoutLineComments(
    sliceBlock(renderOutlineTree, 'toggle.addEventListener("click"'),
  );

  const rebuilt = handler.indexOf('buildOutline()');
  const refocused = handler.indexOf('focusOutlineToggle(');
  assert.notEqual(rebuilt, -1, 'toggle click 이 buildOutline() 을 부르지 않는다');
  assert.notEqual(refocused, -1, '재렌더 뒤 초점을 되돌리는 호출이 없다');
  assert.ok(rebuilt < refocused, '초점 복원은 재렌더 뒤에 일어나야 한다');

  assert.match(
    renderOutlineTree,
    /item\.dataset\.outlineKey\s*=\s*key;/,
    '초점 복원이 항목을 다시 찾을 수 있도록 data-outline-key 가 필요하다',
  );

  const focusHelper = sliceBlock(viewerSource, 'function focusOutlineToggle');
  assert.match(
    focusHelper,
    /data-outline-key=/,
    'focusOutlineToggle 이 data-outline-key 로 같은 항목을 찾아야 한다',
  );
  assert.match(focusHelper, /\.focus\(\)/, 'focusOutlineToggle 이 초점을 주지 않는다');
});
