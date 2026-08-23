CONTEXT: Continuing work on chosun-form/rhwp-code (a fork of "rhwp", the Rust HWP/HWPX
document engine + rhwp-studio TS frontend). chosun-form itself has NO .git — always cd
into rhwp-code (or its rhwp-studio subdir) before any git command.

BACKGROUND: An investigation ("Table Marker Drift") found that rhwp-studio's table-marker
tagging flow (setTableRoleMarker() in template.ts: insertTableRow -> mergeTableCells) hit
a family of bugs where a shared helper synthesized a fallback value (a fixed magic
constant, or a stale/zeroed value) instead of preserving/redistributing an already-known
invariant across an edit. Four instances of this family have been found and fixed so far,
all as local commits on `main` (never pushed, no GitHub issues filed — user wants
everything kept to the private fork, no upstream work):

1f198721e fix(model): split_table_native width-shrink (sync_ctrl_width added)
18bb5ca54 fix(model): get_column_widths() 1800-HU fallback -> distribute common.width
(fixes marker-tagging position shift + marker text-wrap collapse)
34c979b49 fix(model): merge_cells()+delete_row() height collapse to a "hairline" after
two sequential row deletions on a table with a prior vertical merge
ed5a143fc fix(studio): #PAGENO footer field silently no-op'd outside footer-edit mode;
now auto-enters + positions cursor at end (mirrors insertNote/enterNoteEditing);
also fixed Home/End being genuinely unwired in footer-edit keyboard handling
1374cfda7 style: cargo fmt 재적용 (3 stray fmt-only files from an unscoped agent run,
   committed separately per user decision -- not part of the bug-fix family)
de062f5a1 fix(model): insert_row()'s row_span-expand loop grows row_span without
   growing height (5th instance of the family -- mirror image of delete_row's
   hairline collapse). LIVE REPRO CONFIRMED via `rhwp export-render-tree` on a
   synthetic doc: a merged cell with declared height > sibling-row content sum
   (e.g. a manually-resized signature/stamp box) has its "excess" absorbed by
   the last spanned row; inserting a row into the span's interior left the
   total the same but spread over one more row, so the untouched last row
   shrank 45.8px -> 28.7px purely from an edit elsewhere in the span. Fixed
   with grow_row_span_height() (symmetric to shrink_row_span_height());
   height==0 (auto-fit) is preserved, not revived -- growth doesn't need the
   defensive 0->400 bake shrink needed. Two tests added to wasm_api::tests.

STILL OPEN / DEFERRED (not fixed, don't assume done):
- Custom label/guide text capability for footer fields (insertHfField only takes a fixed
  fieldType 1/2/3, no label param) -- explicitly out of scope; investigation found normal
  typing already works once footer-edit positioning is fixed, so no gap remains for the
  reported symptom. Only revisit if a genuinely new feature request shows up.

REUSABLE CONVENTIONS LEARNED THIS SESSION (don't re-derive):
- Toolchain gotcha: plain `cargo`/`rustc` in PATH resolve to a Homebrew install that lacks
  the wasm32-unknown-unknown target. Native `cargo build/test/fmt/clippy` all work fine as-is.
  For a WASM rebuild (only needed if Rust/model code changed and you want it live in the
  browser), you MUST prefix PATH: `PATH="$HOME/.cargo/bin:$PATH" wasm-pack build --target
  web --out-dir pkg` from the rhwp-code root (rustup toolchain 1.93.1, pinned by
  rust-toolchain.toml, has the target; Homebrew's 1.97.1 doesn't).
- Dev server: rhwp-studio runs via `npx vite --host 0.0.0.0 --port 7700` from
  rhwp-code/rhwp-studio. TS/CSS hot-reload automatically; Rust/WASM changes do NOT --
  rebuild pkg/ first (see above), then restart the vite process, to see them live.
- e2e harness is puppeteer-core (not Playwright); system Chrome at
  "/Applications/Google Chrome.app"; check `lsof -nP -iTCP:7700 -sTCP:LISTEN` before
  starting a second dev server on a different port for e2e runs, and kill it after.
- Rust test conventions (src/wasm_api/tests.rs): `HwpDocument::create_empty()` +
  `create_table_native()` + native mutators (`merge_table_cells_native`,
  `delete_table_row_native`, etc.) + helpers `issue_1481_json_usize`, `issue_1481_table`,
  `table_control_paras`; to inject raw cell state directly, use
  `let Control::Table(table) = &mut doc.document.sections[0].paragraphs[idx].controls[0]
  else { panic!(...) };` then mutate `table.cells` in place.
- Commit style: Korean conventional commits, `type(scope): 설명`, body explains root
  cause + fix rationale in Korean, `Co-Authored-By: Claude Sonnet 5 <noreply@anthropic.com>`
  trailer, no issue number unless one is actually tracked. One logical fix per commit
  (this session split a single investigation into 4 separate atomic commits).
- Full validation baseline for a `src/model/table.rs`-only change: `cargo test --lib table`
  (~455+ passed, now 457+ with the insert_row fix's 2 new tests) then `cargo test --lib`
  full suite (~3600+ passed, 13 ignored, 0 failed is the bar), then `cargo fmt --check` +
  `cargo clippy` scoped to touched files only (repo has pre-existing unrelated fmt/clippy
  issues elsewhere -- not your problem, don't fix them as a drive-by, and don't run
  unscoped `cargo fmt`).
- A model-layer field check (row_span/height on the Cell struct) can't prove a renderer-
  visible symptom by itself -- table_layout.rs's per-row height solver is private and has
  no unit tests of its own. To actually see the effect, build a synthetic doc via
  `DocumentCore` native methods, `export_hwpx_native()` it to a scratch file, then run
  `./target/debug/rhwp export-render-tree <file> -o <dir>` and diff the `Cell` bbox
  heights in the resulting JSON. This is how the insert_row row_span/height asymmetry
  below was confirmed as a real (not just theoretical) bug.
- For rhwp-studio-only changes: `npx tsc --noEmit`, then `npm test` (node --test,
  ~888 tests), then optionally the e2e harness. HF-related unit tests in tests/ are
  static source-guard regex tests against the .ts source, NOT real runtime/wasm tests --
  don't expect them to catch behavioral bugs, only structural/wiring ones.
- When two agents touch overlapping files in parallel, scope `git add`/commit explicitly
  by filename (never `git add -A`) and check `git status -s` before AND after each commit
  to confirm the other agent's in-flight changes weren't accidentally swept in.

All threads from this investigation are now resolved: the stray fmt-only files were
committed separately (1374cfda7), and the insert_row asymmetry got a confirmed live
repro and a fix (de062f5a1). Table Marker Drift is closed -- 5 fixes total. Only the
footer-field custom-label item remains explicitly out of scope (see above), to be
revisited only if a genuinely new feature request shows up.

STATUS FOR NEXT SESSION (read this first):
- Table Marker Drift (5 fixes, commits 1f198721e..de062f5a1 above): closed, all
  committed. Nothing to do here unless a genuinely new report shows up.
- Marker-row style work (two fixes below, "NEXT SESSION" + "FOLLOW-UP" blocks):
  both implemented, live-verified, and **committed** this session:
  2498827aa fix(studio): 마커 행이 원본 셀 서식(글꼴·자간·장평·안 여백)을
  물려받아 서로 다르게 보이던 문제 (the consistency fix), then
  3fd2bed65 fix(studio): 마커 행 텍스트가 셀 아래로 잘리고 안 여백이
  사실상 없던 문제 (the cosmetics fix) -- two separate commits per the
  repo's one-logical-fix-per-commit convention, split by writing the
  intermediate (consistency-only) state to disk, testing, committing, then
  re-applying the cosmetics delta on top and committing again. Not pushed
  (private fork, matches the rest of this file's commits). `CLAUDE.md`'s
  modification in `git status` is unrelated and predates this session --
  not mine, leave it alone unless asked.
- Visual reference for the cosmetics fix: `temp/bad-marker-rows.png` at the
  chosun-form root (sibling to rhwp-code) -- shows the "before" state (clipped
  text, no margin, flat gray fill) the fix addresses.
- Test fixture used for all live verification: `kr-gov-form-harvester/
  data/files/5446234/5446234-step2-header.hwpx` (sibling repo). Don't edit it
  in place -- copy it to a scratch dir first, upload the copy into rhwp-studio
  via its file-open flow. The original stays untouched (already has a
  hand-tagged `#HEADER` row, useful as a stable comparison point).

NEXT SESSION (UI/UX polish, separate from Table Marker Drift): marker-row style
consistency (`#HEADER` vs `#REPEAT-TITLE:*` etc. not looking identical) --
`rhwp-studio/src/command/commands/template.ts`'s `MARKER_ROW_STYLE`/
`applyMarkerRowStyle()` used to force only fontSize/bold/textColor/alignment/
fillColor/cellHeightPx/verticalAlign onto a newly-tagged marker row, silently
inheriting typeface, per-run letter-spacing/ratio, and cell padding from whatever
the row had before tagging. LIVE REPRO CONFIRMED at localhost:7700 on
`kr-gov-form-harvester/data/files/5446234/5446234-step2-header.hwpx`: tagging the
`변경 사항` row as `#REPEAT-TITLE:변경사항` rendered with visibly spread-out
("변 경 사 항") character spacing vs. the tight `#HEADER` row, even though both
get the same nominal style -- row height itself was already correct in both cases
(confirmed via `wasm.getTableCellBboxes`: both exactly 22px). FIXED by extending
`applyMarkerRowStyle()` to also force: `fontId` (via `wasm.findOrCreateFontId('돋움체')`,
same fontName->fontId contract already used by `format.ts`'s char-shape-dialog
handler -- no engine/Rust change needed, `parse_char_shape_mods` already supported
`fontId`/`ratios`/`spacings` in `src/document_core/helpers.rs`, just never wired
from this call site), `ratios`/`spacings` (7-language arrays, forced to
100%/0 to kill inherited letter-spacing), and `applyInnerMargin: true` +
explicit `paddingLeft/Right/Top/Bottom: 141` on the `setCellProperties` call (so
the marker row's padding no longer rides on the table's own default). Re-verified
live after the fix: `#REPEAT-TITLE:변경사항` now renders tight, matching `#HEADER`
exactly. `tests/mutation-routing-guard.test.ts`'s baseline for `template.ts` bumped
11 -> 12 for the new `findOrCreateFontId` call site (registered mutating method,
already inside the same `executeOperation` snapshot as the rest of
`setTableRoleMarker`). TS-only change -- no WASM rebuild needed. `npx tsc --noEmit`
+ `npm test` both clean (888 tests, 1 pre-existing skip). Committed as 2498827aa.

FOLLOW-UP (same session): user pasted one of every marker role into a table
(`temp/bad-marker-rows.png`, chosun-form root) and pointed out the *consistency*
fix above didn't make them look *good* -- text nearly flush against the cell
edges, bottom of the glyphs visibly clipped, and the flat #D9D9D9 gray fill
didn't read as "special/system". Root cause confirmed by reading the renderer
(not guessed): `src/renderer/layout/table_layout.rs:7278` excludes trailing
line-spacing for a cell's last line, so single-line content height is just the
font size in px -- 18pt = 1800 HWPUNIT / `HWPUNIT_PER_PX`(75) = 24px. The old
`cellHeightPx: 22` was smaller than even the bare text, guaranteeing clipping
regardless of padding; `cellPaddingHwp: 141` was ~1.9px (not ~1.87mm as
mis-noted earlier in this file -- unit-conversion mistake), too thin to read as
a margin. FIXED (same `MARKER_ROW_STYLE`/`applyMarkerRowStyle()` in
`template.ts`): `cellHeightPx` 22->36, padding raised to 4px (6px on the left,
for clearance from the new accent bar) via a `cellPaddingPx` object x
`HWPUNIT_PER_PX` (self-documenting, replaces the old hand-picked HWPUNIT
literal), fill color #D9D9D9->#DBEAFE (Tailwind blue-100, user's choice -- these
marker rows are authoring-only and never appear in the exported filled form, so
no need to match the government-form palette), and a new ~3.8px solid
`#2563EB` (Tailwind blue-600) left-edge accent border for a clearer
"badge/flag" look (also user's choice). Caught and fixed a real footgun along
the way: `set_cell_properties_native` rebuilds all 4 border sides as soon as
the JSON has *any* border key (`html_table_import.rs:830-839`) -- omitted
sides reset to blank, they don't "stay as-is". Fixed by reading the cell's
current `borderRight`/`borderTop`/`borderBottom` via `getCellProperties` first
and passing them through unchanged alongside the new `borderLeft`. Live
re-verified on the same `5446234-step2-header.hwpx` fixture (fresh tab, per the
HMR gotcha below): both `#HEADER` and `#REPEAT-TITLE:변경사항` now measure
exactly 36px/2700 HWPUNIT tall with padding `[6,4,4,4]px`, identical
`#dbeafe` fill + `#2563eb`/width-10 left border, no clipping -- while each
row's original right/top/bottom borders are correctly preserved distinct (by
design, only the left edge is meant to be uniform). No new wasm-mutating call
site this time (`getCellProperties` is a read, already used elsewhere in the
same function) so the `mutation-routing-guard.test.ts` baseline did **not**
need bumping again. `npx tsc --noEmit` + `npm test` clean (887 pass / 1
pre-existing skip). Committed as 3fd2bed65.

REUSABLE GOTCHA (found this session, not in the original conventions list):
Vite HMR did **not** reliably hot-swap `template.ts` in an already-open
rhwp-studio tab after an edit -- no `[vite] hot updated` console message ever
appeared, and the tab kept running the pre-edit code even after waiting/manual
`location.reload()` calls (blocked by the app's own `beforeunload` "unsaved
changes" native dialog, which page JS can't suppress for an external
navigation). The only reliable fix: open a **new** tab and navigate fresh --
don't trust an existing tab to have picked up a `template.ts`/command-registry
change. Also: the template panel's "태그 지정"/"마커 지우기" buttons only
refresh (`template-panel.ts`'s `refresh()`, gated on `command-state-changed`/
`table-object-selection-changed` events) after a real *edit* command runs --
a plain mouse click into a different cell moves the visible cursor and updates
the style toolbar (via a separate `emitCursorFormatState()` path) but does
**not** refresh the template panel, so the tag button can appear stuck
disabled. Workaround used repeatedly this session: click the cell, type one
throwaway character, then Backspace it -- that round-trip goes through the
command dispatcher and force-refreshes the panel.

NEXT SESSION / LATEST STATE (separate small session, not part of Table Marker
Drift or the marker-row style work above): 블록명 자동 제안 + 태그 지정 버튼
활성화, `rhwp-studio/src/ui/template-panel.ts` only. Two related changes,
both implemented and live-verified this session, **NOT YET COMMITTED**
(`git status -s` in rhwp-studio shows only `M src/ui/template-panel.ts`).

1. 블록명 자동 제안 (REPEAT_TITLE only) + Enter-to-tag: when the
   `#REPEAT-TITLE:` role is selected and 블록명 is still empty, the current
   cell's text is read (`wasm.getCellInfo`/`getCellParagraphLength`/
   `getTextInCell`, same idiom as `readTableMarkerText()` in
   `table-outline.ts`) and all whitespace stripped (`text.replace(/\s+/g,
   '')`, not just trim -- e.g. "변경 사항" -> "변경사항") to prefill
   블록명. Scoped to REPEAT_TITLE only (not HEADER/BODY/FOOTER) because only
   TITLE has a 1:1 "cell text == block name" relationship. Never overwrites
   a non-empty field; a `blockNameManuallyEdited` flag (set on real `input`
   events, cleared on role-radio `change`) prevents re-suggesting over a
   user edit even as the cursor moves. Hooked into `onRoleChanged()`, which
   already fires from role-radio change, nested-toggle change, and every
   `refresh()` (i.e. every cursor move) -- no new event wiring needed.
   Enter in the 블록명 input now triggers the same as clicking 태그 지정
   (`keydown` listener, guards `event.isComposing` for Korean IME).

2. 태그 지정 button enable/disable fix (the actual reason for this entry --
   user hit exactly the stuck-disabled gotcha noted above, but via the new
   auto-suggest flow rather than the old click/type/backspace repro).
   Root cause: `applyBtn.disabled` was set in exactly one place
   (`refresh()`), purely from cursor validity (`inTable && !isNested`), with
   **zero** dependency on 블록명 content -- and nothing re-ran that check
   when 블록명 changed via typing or auto-suggestion (both only called
   `updatePreview()`, never touched `applyBtn`). Two bugs from this, not
   one: (a) button stayed stuck disabled after a valid suggestion filled
   in (this session's repro), and (b) inversely, selecting e.g.
   REPEAT_BODY with an empty 블록명 left the button *enabled*. clicking it
   silently no-op'd because `template:tag-selection`'s `execute()`
   (`command/commands/template.ts`) catches `buildTableRoleMarkerText()`'s
   `requireBlockName`/`requireNestedPair` throw internally
   (`console.warn` + `return`) with no user-visible feedback, and
   `canExecute: (ctx) => ctx.inTable` never looks at blockName either.
   Fix: `updatePreview()` (already the try/catch caller of
   `buildTableRoleMarkerText()`, and already invoked from every place
   블록명/역할/부모블록 can change -- refresh() end, onRoleChanged() end,
   블록명 input's `input` listener, 부모블록 select's `change`) now reuses
   that same try/catch result as `markerValid` and combines it with the
   (still cursor-based) `canTag` to set both `applyBtn.disabled` and
   `clearBtn.disabled` (clearBtn stays `canTag`-only -- clearing a marker
   never needs 블록명). `refresh()`'s old direct `applyBtn.disabled =
   !canTag` / `clearBtn.disabled = !canTag` lines were deleted in favor of
   delegating to `updatePreview()`. The Enter handler now also checks
   `if (this.applyBtn.disabled) return;` before calling `applyTag()`, so
   click and Enter always agree (previously Enter bypassed disabled
   entirely). Bonus: since `buildTableRoleMarkerText` already throws for
   missing `nestedParent` too (`requireNestedPair`), the `_NESTED` roles get
   correct button gating for free, no extra branch needed.

   Verification for both (headless Puppeteer via `puppeteer-core` + system
   Chrome, driving cursor placement directly via
   `window.__inputHandler.cursor.moveTo({...cellPath...})` +
   `window.__eventBus.emit('command-state-changed')` rather than pixel
   clicks -- see the `DocumentPosition` shape with
   `parentParaIndex`/`controlIndex`/`cellIndex`/`cellParaIndex` in
   `formFieldPosition()`, `input-handler.ts`, for the field names): against
   `kr-gov-form-harvester/data/files/5446234/5446234.hwpx` (row 8, "변경
   사항" cell, full-width) -- 블록명 auto-fills to "변경사항" on selecting
   REPEAT_TITLE with `applyBtn.disabled === false` immediately after
   (asserted directly this time, not just inferred from Enter working);
   clearing 블록명 re-disables it; typing re-enables it; REPEAT_BODY with
   empty 블록명 is correctly disabled; Enter while disabled is a genuine
   no-op (outline unchanged); normal case still applies the tag via both
   click-equivalent (Enter) and produces `#REPEAT-TITLE:변경사항` in the
   표 개요. Script written to a temp file under `rhwp-studio/e2e/`, run,
   then deleted (not committed, not a permanent addition to the e2e suite).
   `npx tsc --noEmit` clean both times. No new wasm-mutating call site (all
   reads), so `mutation-routing-guard.test.ts`'s baseline needed no bump.
[UPDATE, later session: the "태그 지정" stuck-disabled half of this gotcha is
now fixed properly -- see the "블록명 자동 제안 + 태그 지정 버튼 활성화" entry
below. The click/type/backspace workaround is no longer needed to un-stick
the apply button specifically, but may still apply to the outline (표 개요)
panel / hint text, which this fix did not touch -- those still only refresh
on the same `command-state-changed`/`table-object-selection-changed` events
as before.]

NEXT SESSION / LATEST STATE (new feature, separate from everything above):
누름틀 이름 자동 제안 (field-name-suggest), `#template-panel` new group. Fully
implemented per a pre-written plan (~/.claude/plans/we-will-talk-about-jaunty-
toucan.md), all steps done and verified except the manual open-in-browser
check and the e2e test (deferred -- see below).

Motivating case: `kr-gov-form-harvester/data/files/5555817/5555817.hwpx`
(법인설립허가신청서) has `fieldCount: 0` and a label column with two
vertically-merged parent cells ("신\n청\n인", "법\n인\n명") whose child rows
each have a leaf label immediately followed by a blank cell -- naively naming
each blank cell after its adjacent label collides the moment the same label
(e.g. "전화번호") appears under more than one parent.

New files:
- `rhwp-studio/src/core/field-name-suggest.ts` -- pure, read-only detector.
  `readTableGrid()` reads every cell's row/col/rowSpan/colSpan + all-paragraph
  text (stripped of whitespace, incl. newlines -- same idiom as
  `suggestBlockNameFromCurrentCell()`) + any existing `fieldName`
  (`getCellProperties`). `buildSectionPrefixMap()` maps every row covered by
  a column-0 `rowSpan>1` anchor to that anchor's stripped text. An ordered
  `ROW_PATTERN_RULES: RowPatternRule[]` (v1 has exactly one:
  `leafLabelAdjacentBlankRule` -- label cell + the blank cell immediately to
  its right, i.e. `col+colSpan`, with `rowSpan===1`) produces candidates;
  `suggestFieldNames()` turns candidates into `sectionPrefix_leafText` (or
  bare `leafText` with no prefix), de-dupes against `wasm.getFieldList()` +
  in-batch names via `_2`/`_3` suffixes, and flags (but does not rename) any
  candidate cell that already carries a field. `isRepeatTaggedTable()`
  (exported from this same file, not the panel) tests the
  `#REPEAT-(BODY|HEADER|FOOTER|TITLE)(-NESTED)?:` pattern -- kept as a
  standalone pure function specifically so it's unit-testable without
  instantiating the DOM-heavy `TemplatePanel` class.
- `rhwp-studio/src/command/commands/field-suggest.ts` -- single command
  `field-suggest:apply`, `canExecute: hasDocument && inTable`. Mirrors
  `template.ts`'s `tagSelectionOperation`: loops `wasm.insertClickHereField()`
  (guide `'입력하세요'`, matching `field-insert-dialog.ts`'s default) once per
  checked item inside one `executeOperation({kind:'snapshot', ...})` so the
  whole batch undoes in one step. Registered in `main.ts` next to
  `templateCommands`.
- `rhwp-studio/tests/field-name-suggest.test.ts` -- new fake-WasmBridge
  fixture (simpler than `template-marker-authoring.test.ts`'s, since this
  detector always operates on one already-resolved table, no
  `findNearestControlForward` multi-table walk needed). Covers: the
  신청인/법인명-shaped fixture (rowSpan 2/3, not the literal 4-leaf-row count
  mentioned in early scoping notes -- rowSpan count and leaf-row count are
  the same number in this synthetic fixture, by construction) asserting
  `신청인_전화번호`/`법인명_전화번호` disambiguation; in-batch `_2`/`_3`
  suffixing; collision against an existing document field name; the
  already-has-field exclusion+flag path; the no-adjacent-blank no-candidate
  case; and `isRepeatTaggedTable` truth table. All pass.

Changed files:
- `rhwp-studio/src/ui/template-panel.ts` -- new "누름틀 이름 제안" `<fieldset>`
  appended after the existing 태그 지정/마커 지우기 actions block (own
  `tp-fieldsuggest-*` sub-classes, reusing `.tp-btn`/`.tp-btn--primary`). "현재
  표에서 제안 생성" button calls `suggestFieldNames()` directly (no
  dispatcher -- same precedent as `suggestBlockNameFromCurrentCell`), but
  first checks `readTableMarkerText()` + `isRepeatTaggedTable()` and shows an
  explanatory message instead of generating if the current table is
  REPEAT-tagged. Review list: one row per suggestion, `R{row+1}` location +
  checkbox (default checked, disabled+"이미 필드가 있음: <name>" badge for
  `alreadyHasField` rows) + editable name `<input>` prefilled with
  `suggestedName`. "적용" button (disabled until >=1 checked+non-empty-name
  insertable row -- recomputed via `updateFieldSuggestApplyState()` on every
  checkbox/input change, same pattern as `updatePreview()` gating
  `applyBtn`) dispatches `field-suggest:apply` with the checked rows' final
  (possibly hand-edited) names, then clears the list and calls `refresh()`.
  `refresh()` now also tracks `fieldSuggestTable: {sec,ppi,ci} | null` (the
  table the current suggestion batch was generated against) and clears the
  list whenever the cursor's current table no longer matches it (moved to a
  different table, or left any table/entered a nested one) -- same spirit as
  the existing `blockNameManuallyEdited` reset, prevents applying a stale
  batch to the wrong table.
- `rhwp-studio/src/styles/template-panel.css` -- `.tp-fieldsuggest-list`
  (column flex), `.tp-fieldsuggest-row` (bordered row), `.tp-fieldsuggest-
  row-loc`/`-input`/`-badge`, all using existing `--space-*`/`--radius-sm`/
  `--color-*` tokens, no new tokens introduced.
- `rhwp-studio/tests/mutation-routing-guard.test.ts` -- `BASELINE` gained
  `'src/command/commands/field-suggest.ts': 1` (one `insertClickHereField`
  call site inside the apply loop -- the guard counts call *sites*, not
  runtime invocation count, so looping over N items is still 1).
  `MUTATING_METHODS` needed no change -- `insertClickHereField` was already
  registered there (used by `insert:field`).
- `mydocs/manual/rhwp_studio_ui_conventions.md` -- extended the
  `#template-panel` row's description; `last_verified` bumped.
- New `mydocs/manual/field_naming_heuristics.md` -- documents the section-
  prefix + leaf-label-adjacent-blank rule, the uniqueness/suffix pass, the
  REPEAT-tagged exclusion, and how to add a new `RowPatternRule` when a
  different layout shape shows up.

Verification done this session: `npx tsc --noEmit` clean; `node --test
tests/*.test.ts` full suite green (870 pass, 1 pre-existing unrelated skip);
`node --test tests/mutation-routing-guard.test.ts` green with the new
baseline entry; `node --test tests/field-name-suggest.test.ts` green (7/7);
`python3 scripts/check_document_metadata.py` and
`check_markdown_links.py` clean on both touched/new manual docs.

Also done (beyond the plan's minimum): e2e test
`rhwp-studio/e2e/field-suggest-panel.test.mjs` -- COMMITTED as a permanent
addition (first e2e test for `#template-panel`), builds a synthetic 6-row/
3-col table via `wasm.createTable`+`mergeTableCells`+`insertTextInCell`
(신청인 rowSpan2/법인명 rowSpan3 shape + one row outside any section),
clicks the real `.tp-fieldsuggest-generate-btn`, asserts the 6 rendered
review rows' names, unchecks one row + edits another row's name via real
DOM events, clicks `.tp-fieldsuggest-apply-btn`, then asserts
`wasm.getFieldList()` has exactly the 5 expected names (edited name applied,
unchecked row excluded) and the list clears after apply. Ran green against
the live dev server (`localhost:7700`, headless Chrome via
`CHROME_PATH="/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
node e2e/field-suggest-panel.test.mjs --mode=headless`).

Manual real-fixture verification (plan's last step) was also completed, but
NOT via the browser extension (Claude-in-Chrome was not connected this
session) -- instead via a temporary ad-hoc puppeteer script (written, run,
then deleted per the established convention, not committed): loaded the
actual `kr-gov-form-harvester/data/files/5555817/5555817.hwpx` bytes
directly into `window.__wasm.loadDocument()` (bypassing the e2e harness's
`/samples`-relative-path restriction), found its one top-level table via
`findNearestControlForward`, placed the cursor in it, clicked "현재 표에서
제안 생성", and confirmed the review list renders **exactly** the 8 expected
names -- `신청인_주소`/`신청인_전화번호`/`신청인_성명`/`신청인_생년월일` and
`법인명_명칭`/`법인명_전화번호`/`법인명_소재지`/`법인명_대표자성명` -- then
clicked "적용" and confirmed `wasm.getFieldList()` has exactly those 8 names,
no duplicates, no extras. Note the real file's actual shape differs from
what earlier scoping notes assumed: it's a 5-column table (two independent
label+blank pairs per row, e.g. row2 has "주소"+blank at col1-2 AND
"전화번호"+blank at col3-4) and 신청인's rowSpan really is 2 (covering
"주소"/"전화번호" at row2 and "성명"/"생년월일" at row3 as two labels-per-row
each), not 4 separate anchor rows -- confirmed via
`rhwp export-tables 5555817.hwpx --json` before writing the check. The
per-cell (not per-row) design of `leafLabelAdjacentBlankRule` handled this
shape correctly with no code changes needed.

Nothing else has been committed -- `git status -s` in `rhwp-code` will show
`HWP-STUDIO-CONTEXT.md`, `mydocs/manual/rhwp_studio_ui_conventions.md`,
`rhwp-studio/src/main.ts`, `rhwp-studio/src/styles/template-panel.css`,
`rhwp-studio/src/ui/template-panel.ts`, and
`rhwp-studio/tests/mutation-routing-guard.test.ts` modified, plus new
`mydocs/manual/field_naming_heuristics.md`,
`rhwp-studio/e2e/field-suggest-panel.test.mjs`,
`rhwp-studio/src/command/commands/field-suggest.ts`,
`rhwp-studio/src/core/field-name-suggest.ts`, and
`rhwp-studio/tests/field-name-suggest.test.ts` -- all staged for the user to
review/commit.