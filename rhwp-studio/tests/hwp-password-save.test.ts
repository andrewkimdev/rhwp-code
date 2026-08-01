import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

const commandSource = readFileSync(new URL('../src/command/commands/file.ts', import.meta.url), 'utf8');
const bridgeSource = readFileSync(new URL('../src/core/wasm-bridge.ts', import.meta.url), 'utf8');
const dialogSource = readFileSync(new URL('../src/ui/hwp-password-dialog.ts', import.meta.url), 'utf8');
const publicWasmSource = readFileSync(new URL('../public/rhwp.js', import.meta.url), 'utf8');
const publicWasmTypes = readFileSync(new URL('../public/rhwp.d.ts', import.meta.url), 'utf8');

function between(source: string, start: string, end: string): string {
  const startIndex = source.indexOf(start);
  assert.notEqual(startIndex, -1, `시작 표식이 있어야 합니다: ${start}`);
  const endIndex = source.indexOf(end, startIndex + start.length);
  assert.notEqual(endIndex, -1, `끝 표식이 있어야 합니다: ${end}`);
  return source.slice(startIndex, endIndex);
}

test('암호 저장 dialog는 확인 입력, 최소 길이, 닫기 시 DOM 초기화를 제공한다', () => {
  const saveDialog = between(dialogSource, 'class HwpSavePasswordDialog', '/** 새 암호와 확인 입력');
  assert.match(saveDialog, /'hwp-save-password-input'/, '새 암호 입력이 있어야 합니다');
  assert.match(saveDialog, /'hwp-save-password-confirmation'/, '암호 확인 입력이 있어야 합니다');
  assert.match(saveDialog, /input\.type = 'password'/, '암호 입력을 마스킹해야 합니다');
  assert.match(saveDialog, /autocomplete = 'off'/, '브라우저 암호 자동완성을 요청하지 않아야 합니다');
  assert.match(saveDialog, /password\.length < 5/, '한컴 UI와 같은 최소 5자 제한이 있어야 합니다');
  assert.match(saveDialog, /password !== this\.confirmationInput\.value/, '확인 입력 일치 여부를 검사해야 합니다');
  assert.match(saveDialog, /this\.passwordInput\.value = ''/, '닫을 때 새 암호 DOM 값을 비워야 합니다');
  assert.match(saveDialog, /this\.confirmationInput\.value = ''/, '닫을 때 확인 DOM 값을 비워야 합니다');
});

test('Studio 암호 저장은 전용 command와 serializer를 사용하고 HML을 거부한다', () => {
  assert.match(commandSource, /id: 'file:save-as-password'/, '파일 메뉴 command가 있어야 합니다');
  assert.match(commandSource, /showHwpSavePasswordDialog/, '암호/확인 대화상자를 열어야 합니다');
  assert.match(commandSource, /exportPasswordProtectedDocumentForFormat/, '전용 암호 serializer를 선택해야 합니다');
  assert.match(commandSource, /암호 설정 저장은 HWP 또는 HWPX 형식에서만 지원합니다/, 'HML 암호 저장을 거부해야 합니다');
});

test('Studio는 암호 문자열을 보관하지 않고 보호 저장 여부만 기억한다', () => {
  const protectedSave = between(commandSource, 'async function saveAsFormatWithPassword', 'function reportSaveError');
  const currentSave = between(commandSource, 'export async function saveCurrentDocument', 'async function fallbackNameForCurrentSave');
  assert.match(protectedSave, /password = '';/, '암호 저장 시도 뒤 지역 암호 참조를 비워야 합니다');
  assert.match(currentSave, /services\.wasm\.requiresPasswordForSave/, '다음 저장에서 재입력을 결정할 상태가 있어야 합니다');
  assert.match(currentSave, /password = '';/, '일반 저장의 재입력 암호 참조도 비워야 합니다');
  assert.doesNotMatch(protectedSave, /localStorage|sessionStorage|console\.|fileName\s*[:=]\s*password/i, '암호를 영속/로그/파일명 경로로 보내면 안 됩니다');
  assert.match(bridgeSource, /private _requiresPasswordForSave = false/, 'bridge는 boolean 상태만 보관해야 합니다');
  assert.match(bridgeSource, /exportHwpWithPassword\(password: string\)/, 'HWP password WASM facade가 있어야 합니다');
  assert.match(bridgeSource, /exportHwpxWithPassword\(password: string\)/, 'HWPX password WASM facade가 있어야 합니다');
});

test('Studio public WASM 배포물도 암호 저장 binding을 제공한다', () => {
  assert.match(publicWasmSource, /exportHwpWithPassword\(password\)/, 'public JS HWP binding이 있어야 합니다');
  assert.match(publicWasmSource, /exportHwpxWithPassword\(password\)/, 'public JS HWPX binding이 있어야 합니다');
  assert.match(publicWasmTypes, /exportHwpWithPassword\(password: string\): Uint8Array/, 'public HWP type이 있어야 합니다');
  assert.match(publicWasmTypes, /exportHwpxWithPassword\(password: string\): Uint8Array/, 'public HWPX type이 있어야 합니다');
});
