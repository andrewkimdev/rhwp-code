#!/usr/bin/env node
/**
 * rhwp-studio의 빌드 산출물(dist/)을 형제 저장소 hwpx-template-engine의 정적 리소스로
 * 벤더링한다. hwpx-template-engine이 자기 서버(StaticFileHandler, "/rhwp" 컨텍스트,
 * HwpxTemplateEngineApplication.java)에서 그대로 서빙할 수 있도록, dist/의 내용을
 * hwpx-template-engine/src/main/resources/static-rhwp/에 그대로 복사한다.
 *
 * 이 스크립트는 `npm run sync:hwpx-template-engine`(package.json)을 통해서만 실행한다 — 그
 * 스크립트가 `vite build --base=/rhwp/`로 먼저 빌드한 뒤 이 파일을 호출한다. 평범한
 * `npm run build`(base 미지정, 기본값 `/`)의 산출물을 그대로 쓰면 자산 경로가 `/rhwp/` 없이
 * 루트 기준으로 나가 hwpx-template-engine의 `/rhwp` 컨텍스트에서 깨진다 — 반드시
 * `--base=/rhwp/`로 다시 빌드된 dist/여야 한다.
 */
import { execFileSync } from 'node:child_process';
import { existsSync } from 'node:fs';
import { cp, mkdir, rm, writeFile } from 'node:fs/promises';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const studioDir = dirname(dirname(fileURLToPath(import.meta.url)));
const distDir = join(studioDir, 'dist');
// rhwp-code와 hwpx-template-engine은 chosun-form 아래 형제 디렉터리다(CLAUDE.md 참고) — 로컬
// 경로 관계일 뿐, 빌드 시스템 레벨의 참조는 아니다.
const targetRoot = resolve(studioDir, '..', '..', 'hwpx-template-engine');
const targetDir = join(targetRoot, 'src', 'main', 'resources', 'static-rhwp');

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
        '(내부에서 --base=/rhwp/로 먼저 빌드한다).',
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

  // hcr/kopub 실물 TTF(native-fonts/)는 vite.config.ts가 RHWP_SYNC_HWPX_TEMPLATE_ENGINE=1일 때
  // dist/에 담지 않는다 — 이미 hwpx-template-engine의 resources/fonts에 있는 같은 파일을
  // static-rhwp/에 159MB 중복 vendor하지 않기 위해서다. 배포本은 HwpxTemplateEngineApplication의
  // /rhwp-fonts 컨텍스트가 그 원본을 직접 서빙한다. 여기서는 그 가정이 깨지지 않았는지만 확인한다.
  if (existsSync(join(targetDir, 'native-fonts'))) {
    console.error(
      `${join(targetDir, 'native-fonts')} 가 존재합니다 — native-fonts/가 dist/에 잘못 포함된 채 ` +
        '동기화됐습니다. package.json의 sync:hwpx-template-engine이 RHWP_SYNC_HWPX_TEMPLATE_ENGINE=1로 ' +
        'vite build를 실행하는지 확인하세요.',
    );
    process.exitCode = 1;
    return;
  }

  const commit = gitCommit(studioDir);
  const timestamp = new Date().toISOString();
  await writeFile(
    join(targetDir, 'VERSION'),
    `rhwp-studio ${commit}\nsynced ${timestamp}\n`,
    'utf-8',
  );
  await writeFile(
    join(targetDir, 'README.md'),
    [
      '# 벤더링된 rhwp-studio 빌드',
      '',
      '이 디렉터리는 `rhwp-code/rhwp-studio`에서 `npm run sync:hwpx-template-engine`으로',
      '생성된다 — 직접 손으로 고치지 않는다. StaticFileHandler(`/rhwp` 컨텍스트,',
      '`HwpxTemplateEngineApplication.java`)가 그대로 서빙한다.',
      '',
      `- 소스 커밋: ${commit}`,
      `- 동기화 시각: ${timestamp}`,
      '',
      '갱신하려면 `rhwp-code/rhwp-studio`에서 `npm run sync:hwpx-template-engine`을 다시',
      '실행한다(내부적으로 `npm run build`를 먼저 돌린다).',
      '',
    ].join('\n'),
    'utf-8',
  );

  console.log(`rhwp-studio dist/를 ${targetDir}에 동기화했습니다 (commit ${commit}).`);
}

await main();
