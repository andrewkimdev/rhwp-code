import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

// [#3648] hwpctl 은 히스토리를 경유하지 않는 자동화 표면으로 판정됐다(②안, 2026-07-31).
//
// 그 판정에서 "지금 상태가 가장 나쁘다"고 지목된 것은 `Undo`/`Redo` 를 등록해 두고
// `console.warn('미구현 (향후 Phase 4에서 구현)')` + `false` 를 돌려주던 부분이다. 그 메시지는
// 두 번 틀렸다 — 구현 대기가 아니라 정책상 미지원이고, iframe 안 콘솔은 통합자에게 보이지
// 않아 원인 판별이 불가능한 침묵 실패가 된다.
//
// 여기서 고정하는 것은 **세 상태가 구분되는가** 다: 미등록(오타) / 미구현(대기) / 미지원(정책).
//
// `command.ts` 처럼 `src/hwpctl` 도 `node --test` strip-only 모드로 import 할 수 없어
// (`index.ts` 가 TS parameter property 를 쓴다) 소스 가드로 확인한다.

const rootDir = dirname(dirname(fileURLToPath(import.meta.url)));
const source = (rel: string): string => readFileSync(join(rootDir, rel), 'utf8');

test('[#3648] Undo/Redo 등록은 유지하되 정책상 미지원으로 표시한다', () => {
  const clipboard = source('src/hwpctl/actions/clipboard.ts');

  // 등록 제거가 아니다 — 지우면 호출자에게 오타와 의도적 미지원이 똑같이 `미등록` 으로 보인다.
  for (const id of ['Undo', 'Redo']) {
    assert.match(
      clipboard,
      new RegExp(`registerAction\\(\\{[\\s\\S]{0,200}?id: '${id}'`),
      `${id} 등록은 유지해야 한다 — 등록을 지우면 오타와 미지원이 구분되지 않는다`,
    );
  }

  // 사유는 코드·메시지·근거를 함께 갖는다.
  assert.match(clipboard, /code: 'notSupportedByDesign'/, '기계가 분기할 코드');
  assert.match(clipboard, /issues\/3648/, '판정 근거로 갈 수 있는 참조');
  assert.match(clipboard, /unsupported: UNDO_UNSUPPORTED/, 'Undo/Redo 에 사유를 실어야 한다');

  // 종전의 잘못된 서술이 **실행 경로에** 남아 있으면 안 된다.
  //
  // 주석에서 과거 문구를 인용하는 것은 기록이므로 막지 않는다 — 실제로 호출되는 코드에서
  // "구현 대기" 로 보고하던 스텁이 사라졌는지만 본다.
  assert.doesNotMatch(
    clipboard,
    /executeUndo|executeRedo/,
    'console.warn 스텁 executor 는 제거됐다 — 사유는 정의(ActionDef)가 든다',
  );
  // 등록에 executor 가 없어야 한다 — 있으면 디스패처가 미지원 판정 대신 그 함수를 부른다.
  for (const id of ['Undo', 'Redo']) {
    const start = clipboard.indexOf(`id: '${id}'`);
    const entry = clipboard.slice(start, start + clipboard.slice(start).indexOf('});'));
    assert.match(entry, /executor: null/, `${id} 등록에 실행 스텁이 남아 있으면 안 된다`);
    assert.match(entry, /unsupported: UNDO_UNSUPPORTED/, `${id} 등록에 사유가 있어야 한다`);
  }
});

test('[#3648] 디스패처는 미지원을 미구현과 다르게 다룬다', () => {
  const action = source('src/hwpctl/action.ts');

  // 정의에 사유 슬롯이 있고, 그 슬롯이 executor 유무보다 먼저 판정된다.
  assert.match(action, /unsupported\?: ActionUnsupportedReason/, 'ActionDef 에 사유 슬롯');
  for (const method of ['Execute(set: ParameterSet): boolean {', 'Run(): boolean {']) {
    const start = action.indexOf(method);
    assert.notEqual(start, -1, `${method} 를 찾지 못했다`);
    const body = action.slice(start, start + action.slice(start).indexOf('\n  }'));
    const idxUnsupported = body.indexOf('this.def.unsupported');
    const idxExecutor = body.indexOf('this.def.executor');
    assert.ok(
      idxUnsupported !== -1 && idxUnsupported < idxExecutor,
      `${method}: 미지원 판정이 executor 분기보다 먼저여야 한다 — 뒤에 두면 executor 가 없는 `
        + `미지원 액션이 "미구현" 으로 보고된다`,
    );
  }

  // 사유 로그는 코드·메시지·근거·조회 경로를 함께 알린다.
  const report = action.slice(action.indexOf('private reportUnsupported'));
  assert.match(report, /reason\.code/, '코드');
  assert.match(report, /reason\.message/, '사유');
  assert.match(report, /reason\.reference/, '근거');
  assert.match(report, /GetActionSupport/, '기계 판독 경로를 로그에서 안내해야 한다');
});

test('[#3648] GetActionSupport 는 미등록·미구현·미지원·지원을 구분한다', () => {
  const index = source('src/hwpctl/index.ts');
  const start = index.indexOf('GetActionSupport(actionId: string)');
  assert.notEqual(start, -1, 'GetActionSupport 가 있어야 한다');
  const body = index.slice(start, start + index.slice(start).indexOf('\n  }'));

  // 미등록은 `null` — 오타를 미지원과 섞으면 판정 API 의 의미가 사라진다.
  assert.match(body, /if \(!def\) return null;/, '미등록은 null');
  assert.match(body, /status: 'unsupported'/, '정책 미지원 상태');
  assert.match(body, /status: 'unimplemented'/, '구현 대기 상태');
  assert.match(body, /status: 'supported'/, '지원 상태');

  // 미지원 판정이 executor 유무보다 먼저 — 순서가 뒤바뀌면 미지원이 미구현으로 보인다.
  const idxUnsupported = body.indexOf("status: 'unsupported'");
  const idxExecutor = body.indexOf('def.executor');
  assert.ok(
    idxUnsupported < idxExecutor,
    '미지원을 executor 분기보다 먼저 판정해야 한다',
  );
});
