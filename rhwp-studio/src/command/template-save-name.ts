import { fileNameForFormat } from './save-target.ts';

/**
 * 파일명에서 hwpx-template-engine 산출물임을 표시하는 `_template` 접미사 이름을
 * 만든다. `entity-section.ts`의 `defaultEntityCodeFromFileName`과 같은 "확장자
 * 제거 후 접미사" 패턴이지만, 여기서는 code 제약(sanitize)이 필요 없어 사람이
 * 읽는 파일명을 그대로 유지한다.
 *
 * 이미 `_template`로 끝나는 파일(= 이전에 저장한 템플릿을 다시 열어 또 저장하는
 * 경우)에는 접미사를 다시 붙이지 않는다 — 그래야 `template:save-as-template`가
 * 같은 파일명으로 저장을 반복해 제자리에서 덮어쓸 수 있다(`_template_template`로
 * 계속 불어나지 않는다).
 *
 * `commands/file.ts`(UI 대화상자 등 `@/` alias 의존 모듈)를 거치지 않는 순수
 * 로직이라 별도 파일로 둔다 — `save-target.ts`처럼 node --test로 직접 단위
 * 테스트할 수 있어야 한다(`tests/template-save-as.test.ts`).
 */
export function templateFileName(fileName: string): string {
  const base = fileName.replace(/\.(hwpx?|hml)$/i, '');
  const withSuffix = /_template$/i.test(base) ? base : `${base}_template`;
  return fileNameForFormat(withSuffix, 'hwpx');
}
