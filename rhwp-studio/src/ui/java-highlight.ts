/**
 * `template_entity`가 방출한 Java 소스(record/모듈 클래스 초안)를 위한 최소 하이라이터.
 *
 * 새 npm 의존성 없이 직접 만든다 — studio는 런타임 의존을 4개로 유지하는 방침이고,
 * 방출된 소스는 이미 결정론적으로 포맷되어 있어(고정 들여쓰기) 포매터는 필요 없다. 필요한
 * 건 색상 강조뿐이므로 주석/문자열/annotation/키워드/타입 5종만 구분한다 — 온전한 Java
 * 파서가 아니다.
 */
const KEYWORDS = new Set([
  'package', 'import', 'public', 'private', 'protected', 'class', 'record', 'interface',
  'implements', 'extends', 'static', 'final', 'void', 'return', 'new', 'throws', 'throw',
  'this', 'super', 'true', 'false', 'null',
]);

// 우선순위 순 대안: 블록 주석 → 줄 주석 → 문자열 → annotation → 식별자.
// 매치되지 않은 구간(공백/구두점/한글 필드명 등)은 그대로 이스케이프해 내보낸다.
const TOKEN_RE = /(\/\*[\s\S]*?\*\/)|(\/\/[^\n]*)|("(?:[^"\\]|\\.)*")|(@[A-Za-z_][A-Za-z0-9_]*)|\b([A-Za-z_][A-Za-z0-9_]*)\b/g;

function escapeHtml(text: string): string {
  return text.replaceAll('&', '&amp;').replaceAll('<', '&lt;').replaceAll('>', '&gt;');
}

/** Java 소스 → 이스케이프된 HTML(`entity-tok-*` 클래스로 감싼 토큰 포함). */
export function highlightJava(source: string): string {
  let out = '';
  let last = 0;
  for (const m of source.matchAll(TOKEN_RE)) {
    const idx = m.index ?? 0;
    out += escapeHtml(source.slice(last, idx));
    const [, blockComment, lineComment, str, annotation, word] = m;
    if (blockComment || lineComment) {
      out += `<span class="entity-tok-comment">${escapeHtml(blockComment ?? lineComment ?? '')}</span>`;
    } else if (str) {
      out += `<span class="entity-tok-string">${escapeHtml(str)}</span>`;
    } else if (annotation) {
      out += `<span class="entity-tok-annotation">${escapeHtml(annotation)}</span>`;
    } else if (word) {
      if (KEYWORDS.has(word)) {
        out += `<span class="entity-tok-keyword">${escapeHtml(word)}</span>`;
      } else if (/^[A-Z]/.test(word)) {
        // 대문자로 시작하는 ASCII 식별자 — 타입/클래스명으로 취급(String, List, IOException...).
        out += `<span class="entity-tok-type">${escapeHtml(word)}</span>`;
      } else {
        out += escapeHtml(word);
      }
    }
    last = idx + m[0].length;
  }
  out += escapeHtml(source.slice(last));
  return out;
}
