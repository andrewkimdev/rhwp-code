#!/usr/bin/env node
/**
 * @rhwp/core(wasm-pack이 ./pkg에 빌드한 rhwp.js + rhwp_bg.wasm + rhwp.d.ts)를 형제 저장소
 * hwpx-template-engine의 정적 리소스로 벤더링한다. 서식공방의 읽기전용 미리보기 페이지
 * (preview.html, item G)가 `import init, { HwpDocument } from '/vendor/rhwp-core/rhwp.js'`로
 * 이 사본을 그대로 쓴다 — @rhwp/editor(iframe 임베드, item F의 live-demo.html)와 달리 필드
 * 목록/커서 좌표 API(getFieldList/getCursorRect*)를 직접 호출해야 하는 필드-매핑 오버레이에는
 * iframe 너머 postMessage RPC가 아니라 WASM 객체에 대한 직접 접근이 필요하기 때문이다.
 *
 * `static/`(항상 서빙되는 개발자 UI 트리) 안이 아니라 `static-rhwp-core/`(별도 최상위
 * 클래스패스 루트, `static-rhwp/`와 같은 관례)에 벤더링한다 — HwpxTemplateEngineApplication의
 * "/" 캐치올 컨텍스트는 `static/` 서브트리 전체를 조건 없이 서빙하므로, `static/` 아래 뒀다면
 * `public-preview.enabled`를 꺼도 이 파일들이 그대로 새어나갔을 것이다. 별도 컨텍스트
 * (`/vendor/rhwp-core` → `static-rhwp-core/`)로 등록해야 플래그가 꺼졌을 때 진짜로
 * 404가 난다.
 *
 * 빌드 단계 없음 — `./pkg`가 이미 `wasm-pack build`(+ `scripts/prepare-npm.sh`)의 산출물이라고
 * 가정하고 그대로 복사한다. `./pkg`가 없으면 먼저 그 둘을 실행한다.
 *
 * 이 동기화는 자동으로 다시 실행되지 않는다 — rhwp 코어 API를 바꾼 뒤에는 이 스크립트를 다시
 * 실행해 hwpx-template-engine의 벤더링 사본을 갱신한다.
 */
import { execFileSync } from 'node:child_process';
import { existsSync } from 'node:fs';
import { copyFile, mkdir, rm, writeFile } from 'node:fs/promises';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const rhwpCodeDir = dirname(dirname(fileURLToPath(import.meta.url)));
const pkgDir = join(rhwpCodeDir, 'pkg');
// rhwp-code와 hwpx-template-engine은 chosun-form 아래 형제 디렉터리다(CLAUDE.md 참고) — 로컬
// 경로 관계일 뿐, 빌드 시스템 레벨의 참조는 아니다.
const targetRoot = resolve(rhwpCodeDir, '..', 'hwpx-template-engine');
const targetDir = join(targetRoot, 'src', 'main', 'resources', 'static-rhwp-core');

function gitCommit(cwd) {
  try {
    return execFileSync('git', ['rev-parse', 'HEAD'], { cwd, encoding: 'utf-8' }).trim();
  } catch {
    return '(unknown — git rev-parse failed)';
  }
}

async function main() {
  if (!existsSync(pkgDir)) {
    console.error(
      `${pkgDir}가 없습니다. 먼저 'wasm-pack build --target web --out-dir pkg'와 ` +
        "'bash scripts/prepare-npm.sh'를 실행하세요.",
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

  await rm(targetDir, { recursive: true, force: true });
  await mkdir(targetDir, { recursive: true });
  for (const file of ['rhwp.js', 'rhwp_bg.wasm', 'rhwp.d.ts']) {
    await copyFile(join(pkgDir, file), join(targetDir, file));
  }

  const commit = gitCommit(rhwpCodeDir);
  const timestamp = new Date().toISOString();
  await writeFile(
    join(targetDir, 'VERSION'),
    `@rhwp/core ${commit}\nsynced ${timestamp}\n`,
    'utf-8',
  );
  await writeFile(
    join(targetDir, 'README.md'),
    [
      '# 벤더링된 @rhwp/core',
      '',
      '이 디렉터리는 `rhwp-code`에서 `node scripts/sync-core-to-hwpx-template-engine.mjs`로',
      '생성된다 — 직접 손으로 고치지 않는다. `preview.html`(item G의 읽기전용 필드-매핑',
      '미리보기)이 `/vendor/rhwp-core/rhwp.js`로 이 사본을 그대로 임포트해',
      '`HwpDocument`(렌더링 + `getFieldList`/`getCursorRect*`)를 iframe 없이 직접 호출한다.',
      '',
      'editor 번들(`vendor/rhwp-editor/`, item F의 live-demo.html)과는 별개다 — 저 번들은',
      'iframe 임베드형 전체 에디터 SDK라 필드 좌표 API를 노출하지 않는다.',
      '',
      `- 소스 커밋: ${commit}`,
      `- 동기화 시각: ${timestamp}`,
      '',
      '갱신하려면 rhwp-code에서 wasm-pack으로 ./pkg를 다시 빌드한 뒤 이 스크립트를 다시',
      '실행한다.',
      '',
    ].join('\n'),
    'utf-8',
  );

  console.log(`@rhwp/core를 ${targetDir}에 동기화했습니다 (commit ${commit}).`);
}

await main();
