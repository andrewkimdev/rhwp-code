#!/usr/bin/env node
/**
 * rhwp-chrome의 빌드 산출물(dist/)을 형제 저장소 hwpx-template-engine의 vendor/rhwp-chrome/에
 * 벤더링한다 — 테스터가 dist/ 릴리스 번들과 함께 "Load unpacked"로 바로 써볼 수 있게 한다.
 * rhwp-studio(sync-to-hwpx-template-engine.mjs)와 달리 이 확장은 HTTP 경로 접두어로 서빙되는
 * 게 아니라 브라우저에 직접 로드하는 언패키지 확장이므로, --base= 재빌드가 필요 없다.
 *
 * 이 스크립트는 `npm run sync:hwpx-template-engine`(package.json)을 통해서만 실행한다 — 그
 * 스크립트가 `npm run build`로 먼저 빌드한 뒤 이 파일을 호출한다.
 */
import { execFileSync } from 'node:child_process';
import { existsSync } from 'node:fs';
import { cp, mkdir, rm, writeFile } from 'node:fs/promises';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const chromeDir = dirname(dirname(fileURLToPath(import.meta.url)));
const distDir = join(chromeDir, 'dist');
// rhwp-code와 hwpx-template-engine은 chosun-form 아래 형제 디렉터리다(CLAUDE.md 참고) — 로컬
// 경로 관계일 뿐, 빌드 시스템 레벨의 참조는 아니다.
const targetRoot = resolve(chromeDir, '..', '..', 'hwpx-template-engine');
const targetDir = join(targetRoot, 'vendor', 'rhwp-chrome');

function gitCommit(cwd) {
  try {
    return execFileSync('git', ['rev-parse', 'HEAD'], { cwd, encoding: 'utf-8' }).trim();
  } catch {
    return '(unknown — git rev-parse failed)';
  }
}

async function main() {
  if (!existsSync(distDir)) {
    console.error(
      `dist/ 산출물이 없습니다(${distDir}). "npm run sync:hwpx-template-engine"으로 실행하세요 ` +
        '(내부에서 npm run build를 먼저 돌린다).',
    );
    process.exitCode = 1;
    return;
  }
  if (!existsSync(targetRoot)) {
    console.error(
      `hwpx-template-engine을 찾을 수 없습니다(${targetRoot}). rhwp-code와 hwpx-template-engine이 ` +
        '같은 chosun-form 디렉터리 아래 형제로 체크아웃돼 있는지 확인하세요.',
    );
    process.exitCode = 1;
    return;
  }

  // 이전 동기화의 잔재(삭제된 파일 등)가 남지 않도록 대상 디렉터리를 통째로 비우고 다시 채운다.
  await rm(targetDir, { recursive: true, force: true });
  await mkdir(targetDir, { recursive: true });
  await cp(distDir, targetDir, { recursive: true });

  const commit = gitCommit(chromeDir);
  const timestamp = new Date().toISOString();
  await writeFile(
    join(targetDir, 'VERSION'),
    `rhwp-chrome ${commit}\nsynced ${timestamp}\n`,
    'utf-8',
  );
  await writeFile(
    join(targetDir, 'README.md'),
    [
      '# 벤더링된 rhwp-chrome 빌드',
      '',
      '이 디렉터리는 `rhwp-code/rhwp-chrome`에서 `npm run sync:hwpx-template-engine`으로',
      '생성된다 — 직접 손으로 고치지 않는다. `hwpx-template-engine`의 `releaseDist` Gradle',
      '태스크가 이 디렉터리를 `dist/chrome-extension/`으로 그대로 복사한다(테스터가',
      '`chrome://extensions` → 개발자 모드 → "압축해제된 확장 프로그램을 로드합니다"로 바로',
      '로드할 수 있는 언패키지 확장 레이아웃).',
      '',
      `- 소스 커밋: ${commit}`,
      `- 동기화 시각: ${timestamp}`,
      '',
      '갱신하려면 `rhwp-code/rhwp-chrome`에서 `npm run sync:hwpx-template-engine`을 다시',
      '실행한다(내부적으로 `npm run build`를 먼저 돌린다).',
      '',
    ].join('\n'),
    'utf-8',
  );

  console.log(`rhwp-chrome dist/를 ${targetDir}에 동기화했습니다 (commit ${commit}).`);
}

await main();
