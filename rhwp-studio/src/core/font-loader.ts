/**
 * 웹폰트 로더 — web/editor.html의 폰트 로딩 시스템을 TypeScript로 포팅
 *
 * 2계층 로딩:
 *   1. CSS @font-face 규칙 생성 (Canvas 2D 호환)
 *   2. FontFace API로 즉시 로드 + document.fonts.add()
 */

interface FontEntry {
  name: string;
  file: string;
  /** woff2(기본) / woff / truetype(hcr, kopub 원본 TTF) */
  format?: 'woff2' | 'woff' | 'truetype';
  /** CSS unicode-range — 지정 시 해당 코드포인트만 매칭, 다운로드도 해당 영역 사용 시에만 발생 */
  unicodeRange?: string;
}

export interface WebFontLoadOptions {
  /** true면 CDN 등 외부 URL 웹폰트 등록/로드를 건너뛴다. */
  disableExternalWebFonts?: boolean;
  /**
   * `native-fonts/` 상대 경로를 이 URL 아래의 자산으로 바꾼다. dev 서버는 별도
   * 미들웨어(vite.config.ts)가 상대 경로 그대로 서빙하므로 미지정으로 두고,
   * hwpx-template-engine에 vendor된 배포 빌드는 `/rhwp-fonts`로 지정한다.
   */
  nativeFontBaseUrl?: string;
}

export interface CanvasKitBundledFontSource {
  url: string;
  aliases: string[];
}

export interface CanvasKitFontPlanOptions extends WebFontLoadOptions {
  /** `fonts/` 상대 경로를 이 URL 아래의 확장/앱 자산으로 바꾼다. */
  localFontBaseUrl?: string;
  /** 배포 표면이 실제로 포함한 로컬 파일만 허용한다. 미지정 시 전체 카탈로그를 허용한다. */
  availableLocalFiles?: ReadonlySet<string>;
}

export interface CanvasKitFontPlan {
  sources: CanvasKitBundledFontSource[];
  unavailableFonts: string[];
}

// 한컴 webhwp CSS(@font-face) 매핑 기준 + HWP 문서에서 사용하는 별칭
const FONT_LIST: FontEntry[] = [
  // === 함초롬(HCR)/한컴 폰트 — hwpx-template-engine 소스 트리의 실물 HWP 네이티브 TTF.
  // (HANBatang.ttf/HANDotum.ttf의 내부 name 테이블 family 이름이 그대로 함초롬바탕/
  // 함초롬돋움이다.) CDN·시스템 폰트 대신 이 파일들만 사용한다 — native-fonts/ 는
  // dev 서버 미들웨어 또는 배포 시 /rhwp-fonts 엔드포인트에서 서빙된다.
  { name: '함초롬돋움', file: 'native-fonts/hcr/HANDotum.ttf', format: 'truetype' },
  { name: '함초롬바탕', file: 'native-fonts/hcr/HANBatang.ttf', format: 'truetype' },
  { name: '함초롱돋움', file: 'native-fonts/hcr/HANDotum.ttf', format: 'truetype' },
  { name: '함초롱바탕', file: 'native-fonts/hcr/HANBatang.ttf', format: 'truetype' },
  { name: '한컴돋움', file: 'native-fonts/hcr/HANDotum.ttf', format: 'truetype' },
  { name: '한컴바탕', file: 'native-fonts/hcr/HANBatang.ttf', format: 'truetype' },
  { name: '한컴산뜻돋움', file: 'native-fonts/hcr/HANDotum.ttf', format: 'truetype' },
  { name: '새돋움', file: 'native-fonts/hcr/HANDotum.ttf', format: 'truetype' },
  { name: '새바탕', file: 'native-fonts/hcr/HANBatang.ttf', format: 'truetype' },
  // === KoPub World (공공 배포용, hwpx-template-engine 소스 트리의 실물 TTF) ===
  // KoPub는 무가중치 "Regular"가 없어 Medium을 기본 별칭으로 쓴다.
  { name: 'KoPubWorldBatang', file: 'native-fonts/kopub/KoPubWorld Batang Medium.ttf', format: 'truetype' },
  { name: 'KoPubWorld바탕체', file: 'native-fonts/kopub/KoPubWorld Batang Medium.ttf', format: 'truetype' },
  { name: 'KoPubWorldBatang Light', file: 'native-fonts/kopub/KoPubWorld Batang Light.ttf', format: 'truetype' },
  { name: 'KoPubWorld바탕체 Light', file: 'native-fonts/kopub/KoPubWorld Batang Light.ttf', format: 'truetype' },
  { name: 'KoPubWorldBatang Medium', file: 'native-fonts/kopub/KoPubWorld Batang Medium.ttf', format: 'truetype' },
  { name: 'KoPubWorld바탕체 Medium', file: 'native-fonts/kopub/KoPubWorld Batang Medium.ttf', format: 'truetype' },
  { name: 'KoPubWorldBatang Bold', file: 'native-fonts/kopub/KoPubWorld Batang Bold.ttf', format: 'truetype' },
  { name: 'KoPubWorld바탕체 Bold', file: 'native-fonts/kopub/KoPubWorld Batang Bold.ttf', format: 'truetype' },
  { name: 'KoPubWorldDotum', file: 'native-fonts/kopub/KoPubWorld Dotum Medium.ttf', format: 'truetype' },
  { name: 'KoPubWorld돋움체', file: 'native-fonts/kopub/KoPubWorld Dotum Medium.ttf', format: 'truetype' },
  { name: 'KoPubWorldDotum Light', file: 'native-fonts/kopub/KoPubWorld Dotum Light.ttf', format: 'truetype' },
  { name: 'KoPubWorld돋움체 Light', file: 'native-fonts/kopub/KoPubWorld Dotum Light.ttf', format: 'truetype' },
  { name: 'KoPubWorldDotum Medium', file: 'native-fonts/kopub/KoPubWorld Dotum Medium.ttf', format: 'truetype' },
  { name: 'KoPubWorld돋움체 Medium', file: 'native-fonts/kopub/KoPubWorld Dotum Medium.ttf', format: 'truetype' },
  { name: 'KoPubWorldDotum Bold', file: 'native-fonts/kopub/KoPubWorld Dotum Bold.ttf', format: 'truetype' },
  { name: 'KoPubWorld돋움체 Bold', file: 'native-fonts/kopub/KoPubWorld Dotum Bold.ttf', format: 'truetype' },
  // === 한컴 HY 폰트 → 오픈소스 대체 ===
  { name: 'HY헤드라인M', file: 'fonts/NotoSansKR-Bold.woff2' },
  { name: 'HYHeadLine M', file: 'fonts/NotoSansKR-Bold.woff2' },
  { name: 'HYHeadLine Medium', file: 'fonts/NotoSansKR-Bold.woff2' },
  { name: 'HY견고딕', file: 'fonts/NotoSansKR-Bold.woff2' },
  { name: 'HYGothic-Extra', file: 'fonts/NotoSansKR-Bold.woff2' },
  { name: 'HY그래픽', file: 'fonts/NotoSansKR-Regular.woff2' },
  { name: 'HYGraphic-Medium', file: 'fonts/NotoSansKR-Regular.woff2' },
  { name: 'HY그래픽M', file: 'fonts/NotoSansKR-Regular.woff2' },
  { name: 'HY견명조', file: 'fonts/NotoSerifKR-Bold.woff2' },
  { name: 'HYMyeongJo-Extra', file: 'fonts/NotoSerifKR-Bold.woff2' },
  { name: 'HY신명조', file: 'fonts/NotoSerifKR-Regular.woff2' },
  { name: 'HY중고딕', file: 'fonts/NotoSansKR-Regular.woff2' },
  { name: '양재튼튼체B', file: 'fonts/NotoSansKR-Bold.woff2' },
  // === 한글 시스템 폰트 → 오픈소스 대체 (OS 폰트 없을 때 폴백) ===
  { name: 'Malgun Gothic', file: 'fonts/Pretendard-Regular.woff2' },
  { name: '맑은 고딕', file: 'fonts/Pretendard-Regular.woff2' },
  // Task #1224: 한컴 돋움/MS 돋움·굴림 계열은 한컴 돋움(획 두께 페이지밀도 0.265)에
  // 근접한 Noto Sans KR ExtraLight 로 대체. 기존 NotoSansKR-Regular(밀도 0.378)는
  // 획이 +43% 두꺼워 PDF 대비 과도하게 굵게 보였다(네이티브 generic_fallback 와 정합).
  { name: '돋움', file: 'fonts/NotoSansKR-ExtraLight.woff2' },
  { name: '돋움체', file: 'fonts/NotoSansKR-ExtraLight.woff2' },
  { name: '굴림', file: 'fonts/NotoSansKR-ExtraLight.woff2' },
  { name: '굴림체', file: 'fonts/D2Coding-Regular.woff2' },
  { name: '새굴림', file: 'fonts/NotoSansKR-ExtraLight.woff2' },
  // Haansoft Dotum: HWP 문서가 직접 지정하는 한컴 돋움 영문명(예: 수능 모의고사 본문).
  // 기존 미등록 → 체인의 'Malgun Gothic'(Pretendard) 가 먼저 매칭되어 굵게 렌더됐다.
  // Task #1224 당시엔 실물 파일이 없어 밀도로 근사한 NotoSansKR-ExtraLight를 썼으나,
  // 이제 실물 HANDotum.ttf를 쓸 수 있어 바이트 단위로 정확한 렌더로 바뀐다 — 기존
  // 밀도 근사와 결과가 달라지므로 수능 모의고사류 문서로 render-diff 확인이 필요하다.
  { name: 'Haansoft Dotum', file: 'native-fonts/hcr/HANDotum.ttf', format: 'truetype' },
  { name: '바탕', file: 'fonts/NotoSerifKR-Regular.woff2' },
  { name: '바탕체', file: 'fonts/D2Coding-Regular.woff2' },
  { name: '궁서', file: 'fonts/GowunBatang-Regular.woff2' },
  { name: '궁서체', file: 'fonts/GowunBatang-Regular.woff2' },
  { name: '새궁서', file: 'fonts/GowunBatang-Regular.woff2' },
  // === 나눔 폰트 (OFL, 로컬) ===
  { name: '나눔고딕', file: 'fonts/NanumGothic-Regular.woff2' },
  { name: '나눔명조', file: 'fonts/NanumMyeongjo-Regular.woff2' },
  { name: '나눔고딕코딩', file: 'fonts/NanumGothicCoding-Regular.woff2' },
  // === 영문 폰트 → OS 폴백 (번들 제거) ===
  { name: 'Palatino Linotype', file: 'fonts/NotoSerifKR-Regular.woff2' },
  // === Noto (OFL, 로컬) ===
  { name: 'Noto Sans KR', file: 'fonts/NotoSansKR-Regular.woff2' },
  // Task #1224: generic_fallback sans 체인 말단의 'Noto Sans KR ExtraLight' 해석용.
  // 미등록 고딕 문서폰트가 체인을 따라 내려올 때 무거운 Noto 직전에 ExtraLight 매칭.
  { name: 'Noto Sans KR ExtraLight', file: 'fonts/NotoSansKR-ExtraLight.woff2' },
  { name: 'Noto Serif KR', file: 'fonts/NotoSerifKR-Regular.woff2' },
  // === Pretendard ===
  { name: 'Pretendard', file: 'fonts/Pretendard-Regular.woff2' },
  { name: 'Pretendard Thin', file: 'fonts/Pretendard-Thin.woff2' },
  { name: 'Pretendard ExtraLight', file: 'fonts/Pretendard-ExtraLight.woff2' },
  { name: 'Pretendard Light', file: 'fonts/Pretendard-Light.woff2' },
  { name: 'Pretendard Medium', file: 'fonts/Pretendard-Medium.woff2' },
  { name: 'Pretendard SemiBold', file: 'fonts/Pretendard-SemiBold.woff2' },
  { name: 'Pretendard Bold', file: 'fonts/Pretendard-Bold.woff2' },
  { name: 'Pretendard ExtraBold', file: 'fonts/Pretendard-ExtraBold.woff2' },
  { name: 'Pretendard Black', file: 'fonts/Pretendard-Black.woff2' },
  // === D2 Coding (OFL, 로컬) ===
  { name: 'D2Coding', file: 'fonts/D2Coding-Regular.woff2' },
  // === Happiness Sans ===
  { name: '해피니스 산스 레귤러', file: 'fonts/Happiness-Sans-Regular.woff2' },
  { name: 'Happiness Sans Regular', file: 'fonts/Happiness-Sans-Regular.woff2' },
  { name: '해피니스 산스 볼드', file: 'fonts/Happiness-Sans-Bold.woff2' },
  { name: 'Happiness Sans Bold', file: 'fonts/Happiness-Sans-Bold.woff2' },
  { name: '해피니스 산스 타이틀', file: 'fonts/Happiness-Sans-Title.woff2' },
  { name: 'Happiness Sans Title', file: 'fonts/Happiness-Sans-Title.woff2' },
  { name: '해피니스 산스 VF', file: 'fonts/HappinessSansVF.woff2' },
  { name: 'Happiness Sans VF', file: 'fonts/HappinessSansVF.woff2' },
  // === Cafe24 ===
  { name: 'Cafe24 Ssurround Bold', file: 'fonts/Cafe24Ssurround-v2.0.woff2' },
  { name: '카페24 슈퍼매직', file: 'fonts/Cafe24Supermagic-Regular-v1.0.woff2' },
  { name: 'Cafe24 Supermagic', file: 'fonts/Cafe24Supermagic-Regular-v1.0.woff2' },
  // === 수식 전용 폰트 (OFL/GUST, 로컬) ===
  { name: 'Latin Modern Math', file: 'fonts/LatinModernMath-Regular.woff2' },
  // === 기타 ===
  { name: 'SpoqaHanSans', file: 'fonts/SpoqaHanSans-Regular.woff2' },
  // === Gowun (OFL, 로컬) ===
  { name: '고운바탕', file: 'fonts/GowunBatang-Regular.woff2' },
  { name: '고운돋움', file: 'fonts/GowunDodum-Regular.woff2' },
  // === Source Han Serif K Old Hangul (Task #528, OFL, 로컬, 옛한글 자모 한정 subset) ===
  // PUA 옛한글 (HanCom 자체 인코딩) 을 KS X 1026-1:2007 자모 시퀀스로 변환 후
  // 합자 렌더링용. unicode-range 로 옛한글 영역에서만 매칭 → 일반 한글 영향 0.
  {
    name: 'Source Han Serif K Old Hangul',
    file: 'fonts/SourceHanSerifK-OldHangul-subset.woff2',
    unicodeRange: 'U+1100-11FF, U+A960-A97F, U+D7B0-D7FF',
  },
];

/** @font-face에 등록된 폰트 이름 Set */
export const REGISTERED_FONTS = new Set(FONT_LIST.map(f => f.name));

/** 초기 렌더링에 필수인 폰트 (대부분의 HWP 문서 기본 서체) */
const CRITICAL_FONTS = new Set(['함초롬바탕', '함초롬돋움']);

/** CSS @font-face 등록 여부 (중복 등록 방지) */
let fontFaceRegistrationMode: 'all' | 'local-only' | null = null;

/** 이미 로드 완료된 woff2 파일 (중복 네트워크 요청 방지) */
const loadedFiles = new Set<string>();

function isExternalFontFile(file: string): boolean {
  return /^https?:\/\//i.test(file);
}

function selectableFontList(options?: WebFontLoadOptions): FontEntry[] {
  if (options?.disableExternalWebFonts !== true) return FONT_LIST;
  return FONT_LIST.filter(f => !isExternalFontFile(f.file));
}

function normalizeFontFamily(value: string): string {
  return value
    .replace(/\u0000/g, '')
    .normalize('NFC')
    .replace(/\s+/g, ' ')
    .trim()
    .toLocaleLowerCase('en-US');
}

/** 경로 세그먼트 단위로 인코딩한다 — kopub 원본 파일명의 공백을 안전하게 URL화한다. */
function encodeFontPath(path: string): string {
  return path.split('/').map(encodeURIComponent).join('/');
}

/**
 * `fonts/`·`native-fonts/` 상대 경로를 배포 환경에 맞는 base URL로 바꾸고 인코딩한다.
 * base가 없으면(로컬 dev) 상대 경로 그대로 인코딩만 적용한다.
 */
function rebaseFontFile(
  file: string,
  options?: { localFontBaseUrl?: string; nativeFontBaseUrl?: string },
): string {
  if (isExternalFontFile(file)) return file;
  if (file.startsWith('native-fonts/') && options?.nativeFontBaseUrl) {
    const base = options.nativeFontBaseUrl.replace(/\/+$/, '');
    return `${base}/${encodeFontPath(file.slice('native-fonts/'.length))}`;
  }
  if (file.startsWith('fonts/') && options?.localFontBaseUrl) {
    const base = options.localFontBaseUrl.replace(/\/+$/, '');
    return `${base}/${encodeFontPath(file.slice('fonts/'.length))}`;
  }
  return encodeFontPath(file);
}

/** CanvasKit이 첫 replay 전에 등록해야 하는 실제 font byte source를 계산한다. */
export function resolveCanvasKitFontPlan(
  requiredFontFamilies: readonly string[],
  options: CanvasKitFontPlanOptions = {},
): CanvasKitFontPlan {
  const canvasKitSubstitutes = new Map([
    [normalizeFontFamily('휴먼명조'), normalizeFontFamily('HY신명조')],
    [normalizeFontFamily('한양중고딕'), normalizeFontFamily('HY중고딕')],
    [normalizeFontFamily('한컴 윤고딕 230'), normalizeFontFamily('Noto Sans KR ExtraLight')],
  ]);
  const entriesByFamily = new Map<string, FontEntry>();
  for (const entry of FONT_LIST) {
    entriesByFamily.set(normalizeFontFamily(entry.name), entry);
  }

  const sourcesByUrl = new Map<string, Set<string>>();
  const unavailableFonts = new Map<string, string>();
  const requiredEntries: Array<{ entry: FontEntry; requested: string }> = [];
  for (const requested of requiredFontFamilies) {
    const normalized = normalizeFontFamily(requested);
    if (!normalized) continue;
    const entry = entriesByFamily.get(normalized)
      ?? entriesByFamily.get(canvasKitSubstitutes.get(normalized) ?? '');
    if (!entry) {
      unavailableFonts.set(normalized, requested.trim());
      continue;
    }
    const localFile = entry.file.startsWith('fonts/')
      ? entry.file.slice('fonts/'.length)
      : null;
    const unavailable = (options.disableExternalWebFonts === true && isExternalFontFile(entry.file))
      || (localFile !== null
        && options.availableLocalFiles !== undefined
        && !options.availableLocalFiles.has(localFile));
    if (unavailable) {
      unavailableFonts.set(normalized, requested.trim());
      continue;
    }
    requiredEntries.push({ entry, requested: requested.trim() });
  }

  for (const { entry, requested } of requiredEntries) {
    const url = rebaseFontFile(entry.file, options);
    const aliases = sourcesByUrl.get(url) ?? new Set<string>();
    aliases.add(requested);
    for (const candidate of FONT_LIST) {
      if (candidate.file === entry.file) aliases.add(candidate.name);
    }
    sourcesByUrl.set(url, aliases);
  }

  return {
    sources: [...sourcesByUrl.entries()].map(([url, aliases]) => ({
      url,
      aliases: [...aliases].sort((left, right) => left.localeCompare(right, 'ko')),
    })),
    unavailableFonts: [...unavailableFonts.values()]
      .sort((left, right) => left.localeCompare(right, 'ko')),
  };
}

function registerFontFaces(options?: WebFontLoadOptions): void {
  const disableExternal = options?.disableExternalWebFonts === true;
  const mode = disableExternal ? 'local-only' : 'all';
  if (fontFaceRegistrationMode === mode) return;

  const styleId = 'rhwp-web-font-faces';
  let style = document.getElementById(styleId) as HTMLStyleElement | null;
  if (!style) {
    style = document.createElement('style');
    style.id = styleId;
    document.head.appendChild(style);
  }
  style.textContent = selectableFontList(options).map(f => {
    const fmt = f.format ?? 'woff2';
    const ur = f.unicodeRange ? ` unicode-range: ${f.unicodeRange};` : '';
    const url = rebaseFontFile(f.file, options);
    return `@font-face { font-family: "${f.name}"; src: url("${url}") format("${fmt}"); font-display: swap;${ur} }`;
  }).join('\n');
  fontFaceRegistrationMode = mode;
}

/**
 * 웹폰트를 선별 로드한다.
 *   1단계(동기): CSS @font-face 등록
 *   2단계: 대상 폰트 로드 (이미 로드된 파일은 건너뜀)
 *
 * @param docFonts 문서에서 사용하는 폰트 이름 목록 (있으면 해당 폰트 + CRITICAL만 로드, 없으면 전체)
 * @param onProgress 폰트 로드 진행률 콜백 (loaded, total)
 * @param options 외부 웹폰트 사용 여부 등 로드 옵션
 */
export async function loadWebFonts(
  docFonts?: string[],
  onProgress?: (loaded: number, total: number) => void,
  options?: WebFontLoadOptions,
): Promise<void> {
  // 1) CSS @font-face 규칙 등록. 오프라인 옵션이면 외부 URL 폰트는 제외한다.
  registerFontFaces(options);

  // 2) 로드 대상 결정: docFonts에 포함된 폰트 + CRITICAL만 로드.
  //    OS/시스템 폰트는 절대 신뢰하지 않는다 — 번들 파일만 로드한다.
  const targetSet = new Set([...(docFonts ?? []), ...CRITICAL_FONTS]);
  const toLoad = selectableFontList(options).filter(f => targetSet.has(f.name));

  // woff2 파일 기준으로 중복 제거 + 이미 로드된 파일 건너뜀
  const seenFiles = new Set<string>();
  const uniqueToLoad: FontEntry[] = [];
  for (const f of toLoad) {
    if (!seenFiles.has(f.file) && !loadedFiles.has(f.file)) {
      seenFiles.add(f.file);
      uniqueToLoad.push(f);
    }
  }

  if (uniqueToLoad.length === 0) return;

  const total = uniqueToLoad.length;
  console.log(`[FontLoader] 웹폰트 로드 시작: ${total}개 woff2 (이미 로드됨: ${loadedFiles.size}개)`);

  // 같은 woff2 파일에 매핑된 모든 이름도 함께 등록
  const fileToNames = new Map<string, string[]>();
  for (const f of toLoad) {
    if (!loadedFiles.has(f.file)) {
      const names = fileToNames.get(f.file) ?? [];
      names.push(f.name);
      fileToNames.set(f.file, names);
    }
  }

  let loaded = 0;
  let failed = 0;
  const BATCH = 4;

  for (let i = 0; i < uniqueToLoad.length; i += BATCH) {
    const batch = uniqueToLoad.slice(i, i + BATCH);
    await Promise.all(batch.map(async (f) => {
      try {
        const names = fileToNames.get(f.file) ?? [f.name];
        const fmt = f.format ?? 'woff2';
        const url = rebaseFontFile(f.file, options);
        for (const name of names) {
          const face = new FontFace(name, `url(${url}) format('${fmt}')`);
          const result = await face.load();
          document.fonts.add(result);
        }
        loadedFiles.add(f.file);
        loaded++;
      } catch {
        failed++;
      }
      onProgress?.(loaded + failed, total);
    }));
    if (i + BATCH < uniqueToLoad.length) {
      await new Promise(r => setTimeout(r, 0));
    }
  }

  console.log(`[FontLoader] 폰트 로드 완료: ${loaded}개 성공, ${failed}개 실패 (총 ${loadedFiles.size}개 woff2 로드됨)`);
}
