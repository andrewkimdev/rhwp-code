/** 대화상자 공유 DOM 프리미티브 — 속성 다이얼로그들이 공통으로 쓰는 생성 헬퍼 */

/** 영역 내 모든 input/select/button을 disabled 처리 */
export function setAreaDisabled(area: HTMLElement, disabled: boolean): void {
  area.querySelectorAll('input, select, button').forEach(el => {
    (el as HTMLInputElement).disabled = disabled;
  });
}

export function fieldset(title: string): HTMLFieldSetElement {
  const fs = document.createElement('fieldset');
  fs.className = 'cs-fieldset';
  const legend = document.createElement('legend');
  legend.textContent = title;
  fs.appendChild(legend);
  return fs;
}

export function row(): HTMLDivElement {
  const r = document.createElement('div');
  r.className = 'dialog-row';
  return r;
}

export function label(text: string): HTMLSpanElement {
  const l = document.createElement('span');
  l.className = 'dialog-label';
  l.textContent = text;
  return l;
}

export function unit(text: string): HTMLSpanElement {
  const u = document.createElement('span');
  u.className = 'dialog-unit';
  u.textContent = text;
  return u;
}

export function numberInput(min?: number, max?: number, step?: number): HTMLInputElement {
  const inp = document.createElement('input');
  inp.type = 'number';
  inp.className = 'dialog-input';
  if (min !== undefined) inp.min = String(min);
  if (max !== undefined) inp.max = String(max);
  if (step !== undefined) inp.step = String(step);
  return inp;
}

export function colorInput(defaultVal: string): HTMLInputElement {
  const inp = document.createElement('input');
  inp.type = 'color';
  inp.className = 'cs-color-btn';
  inp.value = defaultVal;
  return inp;
}

export function selectEl(options: [string, string][]): HTMLSelectElement {
  const sel = document.createElement('select');
  sel.className = 'dialog-select';
  for (const [val, lbl] of options) {
    const opt = document.createElement('option');
    opt.value = val;
    opt.textContent = lbl;
    sel.appendChild(opt);
  }
  return sel;
}

export function sizeTypeSelect(): HTMLSelectElement {
  return selectEl([['fixed', '고정 값']]);
}

export function checkboxLabel(text: string): HTMLLabelElement {
  const lb = document.createElement('label');
  lb.className = 'dialog-checkbox';
  const cb = document.createElement('input');
  cb.type = 'checkbox';
  lb.appendChild(cb);
  lb.appendChild(document.createTextNode(` ${text}`));
  return lb;
}
