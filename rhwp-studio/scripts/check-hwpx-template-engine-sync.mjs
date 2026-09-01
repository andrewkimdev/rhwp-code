#!/usr/bin/env node
/**
 * hwpx-template-engine에 벤더링된 rhwp-studio 빌드(static-rhwp/)가 현재 rhwp-code HEAD
 * 기준으로 드리프트됐는지 점검한다. sync-to-hwpx-template-engine.mjs는 실제 동기화를
 * 수행하고, 이 스크립트는 "다시 돌려야 하는지" 판정만 한다 — 아무것도 쓰지 않는다.
 *
 * 판정 기준은 static-rhwp/VERSION에 적힌 소스 커밋과 현재 HEAD 사이에 rhwp-studio/
 * 경로를 실제로 건드린 커밋이 있는지다. HEAD가 앞서 있어도 그 사이 커밋이 전부
 * rhwp-studio 바깥만 건드렸다면 동기화가 필요 없다.
 */
import { execFileSync } from 'node:child_process';
import { existsSync, readFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const studioDir = dirname(dirname(fileURLToPath(import.meta.url)));
const rhwpCodeDir = resolve(studioDir, '..');
const targetRoot = resolve(studioDir, '..', '..', 'hwpx-template-engine');
const versionFile = join(targetRoot, 'src', 'main', 'resources', 'static-rhwp', 'VERSION');

function git(args, cwd) {
  return execFileSync('git', args, { cwd, encoding: 'utf-8' }).trim();
}

function main() {
  if (!existsSync(targetRoot)) {
    console.error(
      `hwpx-template-engine을 찾을 수 없습니다(${targetRoot}). rhwp-code와 hwpx-template-engine이 ` +
        '같은 chosun-form 디렉터리 아래 형제로 체크아웃돼 있는지 확인하세요.',
    );
    process.exitCode = 1;
    return;
  }
  if (!existsSync(versionFile)) {
    console.error(
      `${versionFile}이 없습니다. 아직 한 번도 동기화되지 않은 것으로 보입니다 — ` +
        '"npm run sync:hwpx-template-engine"을 먼저 실행하세요.',
    );
    process.exitCode = 1;
    return;
  }

  const versionText = readFileSync(versionFile, 'utf-8');
  const match = versionText.match(/^rhwp-studio ([0-9a-f]{7,40})$/m);
  if (!match) {
    console.error(`${versionFile}에서 "rhwp-studio <sha>" 줄을 찾지 못했습니다:\n${versionText}`);
    process.exitCode = 1;
    return;
  }
  const syncedSha = match[1];
  const head = git(['rev-parse', 'HEAD'], rhwpCodeDir);

  if (head === syncedSha || head.startsWith(syncedSha)) {
    console.log(`in sync (rhwp-code HEAD ${head} = 동기화된 커밋).`);
    return;
  }

  const range = `${syncedSha}..${head}`;
  let touchingCommits;
  try {
    touchingCommits = git(['log', '--oneline', range, '--', 'rhwp-studio'], rhwpCodeDir);
  } catch {
    console.error(
      `동기화된 커밋(${syncedSha})이 현재 rhwp-code 히스토리에서 조회되지 않습니다 — shallow clone이거나 ` +
        '히스토리가 재작성됐을 수 있습니다. 직접 확인 후 필요하면 동기화를 다시 실행하세요.',
    );
    process.exitCode = 1;
    return;
  }

  if (!touchingCommits) {
    console.log(
      `HEAD는 ${syncedSha.slice(0, 9)} 이후로 움직였지만(${head}), rhwp-studio/ 경로를 건드린 ` +
        '커밋은 없습니다 — 동기화 불필요.',
    );
    return;
  }

  console.error(
    `stale: 동기화된 커밋(${syncedSha.slice(0, 9)}) 이후 rhwp-studio/ 를 건드린 커밋이 있습니다:\n\n` +
      `${touchingCommits}\n\n` +
      '"npm run sync:hwpx-template-engine"을 실행해 hwpx-template-engine의 벤더링 사본을 갱신하세요.',
  );
  process.exitCode = 1;
}

main();
