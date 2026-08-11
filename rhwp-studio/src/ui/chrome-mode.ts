/**
 * UI chrome 프로파일 리졸버 (#4564).
 *
 * `?chrome=embed` — iframe 임베드처럼 문서 수명주기(열기/저장)를 호스트가 소유하는
 * 구성용 opt-in 프로파일. `?renderer=`와 같은 패턴을 따른다: 순수 resolve 함수,
 * URL 파라미터만 읽고(저장소 지속 없음), 미지원 값은 기본(full)으로 폴백하며
 * unsupportedReason으로 보고한다.
 */

export type ChromeMode = 'full' | 'embed';
export type ChromeModeRequestSource = 'default' | 'url';
export type ChromeModeUnsupportedReason = 'unsupportedChromeMode';

export interface ChromeModeRequest {
  mode: ChromeMode;
  source: ChromeModeRequestSource;
  requested?: string;
  unsupportedReason?: ChromeModeUnsupportedReason;
}

/**
 * embed 프로파일에서 등록하지 않는 파일 수명주기 커맨드.
 *
 * 문서 수명주기를 호스트가 소유하는 구성에서 로컬 저장류는 "저장됐다"는 오인을
 * 만들고(다운로드 폴더로 떨어질 뿐 호스트 저장소에는 반영되지 않는다), 열기/새
 * 문서는 호스트가 감지할 수 없는 문서 교체 경로를 연다. `file:page-setup`과
 * `file:about`은 수명주기가 아니라 편집·정보 표면이므로 유지한다.
 */
export const EMBED_HIDDEN_FILE_COMMAND_IDS: readonly string[] = [
  'file:new-doc',
  'file:open',
  'file:open-recent',
  'file:clear-recent',
  'file:save',
  'file:save-as',
  'file:save-as-hwp',
  'file:save-as-hwpx',
  'file:print-to-pdf',
  'file:print',
];

export function resolveChromeMode(search = ''): ChromeMode {
  return resolveChromeModeRequest(search).mode;
}

export function resolveChromeModeRequest(search = ''): ChromeModeRequest {
  const explicit = new URLSearchParams(search).get('chrome');
  const normalized = explicit?.trim().toLowerCase();
  if (!normalized) return { mode: 'full', source: 'default' };
  if (normalized === 'embed') {
    return { mode: 'embed', source: 'url', requested: normalized };
  }
  if (normalized === 'full') {
    return { mode: 'full', source: 'url', requested: normalized };
  }
  return {
    mode: 'full',
    source: 'url',
    requested: explicit ?? normalized,
    unsupportedReason: 'unsupportedChromeMode',
  };
}
