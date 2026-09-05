#!/usr/bin/env node
/**
 * @rhwp/editor(index.js + transport.js)를 형제 저장소 hwpx-template-engine의 정적 리소스로
 * 벤더링한다. 서식공방의 라이브 데모 페이지(live-demo.html)가 `import { createEditor } from
 * './vendor/rhwp-editor/index.js'`로 이 사본을 그대로 쓴다 — npm 레지스트리 설치나 별도 빌드
 * 없이, hwpx-template-engine 자신의 StaticFileHandler("/")가 서빙하는 정적 파일 두 개로 충분한
 * 이유는 이 패키지가 의존성 없는 순수 ES 모듈이기 때문이다(package.json 참고).
 *
 * static-rhwp(rhwp-studio 전체 앱 번들, sync-to-hwpx-template-engine.mjs 참고)와 달리 빌드
 * 단계가 없다 — index.js/transport.js를 그대로 복사할 뿐이다.
 *
 * 이 동기화는 자동으로 다시 실행되지 않는다 — @rhwp/editor를 바꾼 뒤에는
 * `npm run check:hwpx-template-engine-sync`로 hwpx-template-engine의 벤더링 사본이
 * 뒤처졌는지 직접 확인한다.
 */
import { execFileSync } from 'node:child_process';
import { existsSync } from 'node:fs';
import { copyFile, mkdir, rm, writeFile } from 'node:fs/promises';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const editorDir = dirname(dirname(fileURLToPath(import.meta.url)));
const rhwpCodeDir = resolve(editorDir, '..', '..');
// rhwp-code와 hwpx-template-engine은 chosun-form 아래 형제 디렉터리다(CLAUDE.md 참고) — 로컬
// 경로 관계일 뿐, 빌드 시스템 레벨의 참조는 아니다.
const targetRoot = resolve(rhwpCodeDir, '..', 'hwpx-template-engine');
const targetDir = join(targetRoot, 'src', 'main', 'resources', 'static', 'vendor', 'rhwp-editor');

function gitCommit(cwd) {
  try {
    return execFileSync('git', ['rev-parse', 'HEAD'], { cwd, encoding: 'utf-8' }).trim();
  } catch {
    return '(unknown — git rev-parse failed)';
  }
}

async function main() {
  if (!existsSync(targetRoot)) {
    console.error(
      `hwpx-template-engine을 찾을 수 없습니다(${targetRoot}). rhwp-code와 hwpx-template-engine이 ` +
        '같은 chosun-form 디렉터리 아래 형제로 체크아웃돼 있는지 확인하세요.',
    );
    process.exitCode = 1;
    return;
  }

  await rm(targetDir, { recursive: true, force: true });
  await mkdir(targetDir, { recursive: true });
  await copyFile(join(editorDir, 'index.js'), join(targetDir, 'index.js'));
  await copyFile(join(editorDir, 'transport.js'), join(targetDir, 'transport.js'));

  const commit = gitCommit(rhwpCodeDir);
  const timestamp = new Date().toISOString();
  await writeFile(
    join(targetDir, 'VERSION'),
    `@rhwp/editor ${commit}\nsynced ${timestamp}\n`,
    'utf-8',
  );
  await writeFile(
    join(targetDir, 'README.md'),
    [
      '# 벤더링된 @rhwp/editor',
      '',
      '이 디렉터리는 `rhwp-code/npm/editor`에서 `npm run sync:hwpx-template-engine`으로',
      '생성된다 — 직접 손으로 고치지 않는다. `live-demo.html`이 `/vendor/rhwp-editor/index.js`로',
      '그대로 임포트한다. 이 SDK가 iframe으로 띄우는 studioUrl(`/rhwp/`)은 별도로',
      '`static-rhwp/`에 벤더링돼 있다(rhwp-studio의 sync 스크립트, item B의 `--rhwp-studio` 플래그).',
      '',
      `- 소스 커밋: ${commit}`,
      `- 동기화 시각: ${timestamp}`,
      '',
      '갱신하려면 `rhwp-code/npm/editor`에서 `npm run sync:hwpx-template-engine`을 다시 실행한다.',
      '',
    ].join('\n'),
    'utf-8',
  );

  console.log(`@rhwp/editor를 ${targetDir}에 동기화했습니다 (commit ${commit}).`);
}

await main();
