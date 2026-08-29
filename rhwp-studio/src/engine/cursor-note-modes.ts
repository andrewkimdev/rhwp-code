/** CursorState 머리말/꼬리말(HF)·각주(FN) 편집 모드 메서드 — extracted from CursorState class */
/* eslint-disable @typescript-eslint/no-explicit-any */

// ─── 머리말/꼬리말 편집 모드 ────────────────────────────

/** 머리말/꼬리말 편집 모드에 진입한다. */
export function enterHeaderFooterMode(this: any, isHeader: boolean, sectionIdx: number, applyTo: number, preferredPage = -1): void {
  // 현재 본문 커서 위치 저장
  this._savedBodyPosition = { ...this.position };

  this._headerFooterMode = isHeader ? 'header' : 'footer';
  this._hfSectionIdx = sectionIdx;
  this._hfApplyTo = applyTo;
  this._hfParaIdx = 0;
  this._hfCharOffset = 0;
  this._hfPreferredPage = preferredPage;

  // 선택 해제
  this.clearSelection();

  // 커서 좌표 갱신
  this.updateRect();
}

/** 머리말/꼬리말 편집 모드에서 탈출한다. */
export function exitHeaderFooterMode(this: any): void {
  if (this._headerFooterMode === 'none') return;

  this._headerFooterMode = 'none';

  // 본문 커서 위치 복원
  if (this._savedBodyPosition) {
    // 머리말/꼬리말 마커 para_index(usize::MAX 계열)가 저장된 경우 → 문서 시작으로 초기화
    if (this._savedBodyPosition.paragraphIndex >= 0xFFFFFF00) {
      this._savedBodyPosition.paragraphIndex = 0;
      this._savedBodyPosition.charOffset = 0;
    }
    this.position = { ...this._savedBodyPosition };
    this._savedBodyPosition = null;
  }

  this.clearSelection();
  this.updateRect();
}

/** 다른 머리말/꼬리말로 직접 전환한다 (exit→enter 사이의 updateRect 호출을 피함). */
export function switchHeaderFooterTarget(this: any, isHeader: boolean, sectionIdx: number, applyTo: number, targetPage = -1): void {
  if (this._headerFooterMode === 'none') return;
  this._headerFooterMode = isHeader ? 'header' : 'footer';
  this._hfSectionIdx = sectionIdx;
  this._hfApplyTo = applyTo;
  this._hfParaIdx = 0;
  this._hfCharOffset = 0;
  this._hfPreferredPage = targetPage >= 0 ? targetPage : (this.rect?.pageIndex ?? this._hfPreferredPage);
  this.clearSelection();
  this.updateRect();
}

/** 머리말/꼬리말 내 커서 위치를 설정한다. */
export function setHfCursorPosition(this: any, paraIdx: number, charOffset: number): void {
  this._hfParaIdx = paraIdx;
  this._hfCharOffset = charOffset;
  this.updateRect();
}

/** 머리말/꼬리말 내 수평 이동 */
export function moveHorizontalInHf(this: any, delta: number): void {
  if (this._headerFooterMode === 'none') return;
  const isHeader = this._headerFooterMode === 'header';

  try {
    const info = JSON.parse(this.wasm.getHeaderFooterParaInfo(
      this._hfSectionIdx, isHeader, this._hfApplyTo, this._hfParaIdx
    ));
    const paraCount = info.paraCount as number;
    const charCount = info.charCount as number;

    const newOffset = this._hfCharOffset + delta;

    if (newOffset >= 0 && newOffset <= charCount) {
      // 같은 문단 내 이동
      this._hfCharOffset = newOffset;
    } else if (delta > 0 && this._hfParaIdx + 1 < paraCount) {
      // 다음 문단 시작으로
      this._hfParaIdx++;
      this._hfCharOffset = 0;
    } else if (delta < 0 && this._hfParaIdx > 0) {
      // 이전 문단 끝으로
      this._hfParaIdx--;
      const prevInfo = JSON.parse(this.wasm.getHeaderFooterParaInfo(
        this._hfSectionIdx, isHeader, this._hfApplyTo, this._hfParaIdx
      ));
      this._hfCharOffset = prevInfo.charCount as number;
    }
    // else: 문서 경계 — 이동 불가
  } catch {
    // WASM 호출 실패 시 무시
  }

  this.updateRect();
}

// ─── 각주 편집 모드 ──────────────────────────────────────

/** 각주 편집 모드에 진입한다.
 *
 * [Task #1058 reopen Round 5] 신규/기존 각주 inner_para 의 한컴 contract 는
 * 두 placeholder space + AutoNumber 8 cu 차지 (text="  ", char_offsets=[0, 8]).
 * caret 초기 위치를 char_offset=2 로 설정하여 사용자 입력이 placeholder 뒤
 * (실제 본문 작성 영역) 부터 시작하도록 한다. char_offset=0/1 위치는 placeholder
 * 자리이므로 사용자 입력 시 AutoNumber jump 8 byte contract 깨짐 (한컴 거부).
 */
export function enterFootnoteMode(
  this: any,
  sectionIdx: number, paraIdx: number, controlIdx: number,
  footnoteIndex: number, pageNum: number,
): void {
  this._savedBodyPosition = { ...this.position };
  this._footnoteMode = true;
  this._fnSectionIdx = sectionIdx;
  this._fnParaIdx = paraIdx;
  this._fnControlIdx = controlIdx;
  this._fnFootnoteIndex = footnoteIndex;
  this._fnInnerParaIdx = 0;
  this._fnCharOffset = 2;
  this._fnPageNum = pageNum;
  this.clearSelection();
  this.updateRect();
}

/** 각주 편집 모드에서 탈출한다. */
export function exitFootnoteMode(this: any): void {
  if (!this._footnoteMode) return;
  this._footnoteMode = false;
  if (this._savedBodyPosition) {
    if (this._savedBodyPosition.paragraphIndex >= 0xFFFFFF00) {
      this._savedBodyPosition.paragraphIndex = 0;
      this._savedBodyPosition.charOffset = 0;
    }
    this.position = { ...this._savedBodyPosition };
    this._savedBodyPosition = null;
  }
  this.clearSelection();
  this.updateRect();
}

/** 각주 내 커서 위치를 설정한다. */
export function setFnCursorPosition(this: any, fnParaIdx: number, charOffset: number): void {
  this._fnInnerParaIdx = fnParaIdx;
  this._fnCharOffset = charOffset;
  this.updateRect();
}

/** 각주 내 수평 이동 */
export function moveHorizontalInFn(this: any, delta: number): void {
  if (!this._footnoteMode) return;

  try {
    const info = this.wasm.getFootnoteInfo(this._fnSectionIdx, this._fnParaIdx, this._fnControlIdx);
    const paraCount = info.paraCount;
    // 현재 문단의 텍스트 길이
    const currentText = info.texts[this._fnInnerParaIdx] ?? '';
    const charCount = currentText.length;

    const newOffset = this._fnCharOffset + delta;

    if (newOffset >= 0 && newOffset <= charCount) {
      this._fnCharOffset = newOffset;
    } else if (delta > 0 && this._fnInnerParaIdx + 1 < paraCount) {
      this._fnInnerParaIdx++;
      this._fnCharOffset = 0;
    } else if (delta < 0 && this._fnInnerParaIdx > 0) {
      this._fnInnerParaIdx--;
      const prevText = info.texts[this._fnInnerParaIdx] ?? '';
      this._fnCharOffset = prevText.length;
    }
  } catch {
    // WASM 호출 실패 시 무시
  }

  this.updateRect();
}
