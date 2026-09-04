//! [Issue #6495] HWPX 파서가 note 내부 후속 줄의 `vertpos = 0` 을 **단조값으로 덮어**
//! 되감김(= 한컴이 거기서 단/쪽을 끊었다는 신호)을 지우던 결함의 가드.
//!
//! `#1692` 는 그 `0` 을 "연속줄 아티팩트"로 보고 `prev + line_height + line_spacing` 으로
//! 복원했다. 그런데 같은 내용의 두 판을 맞대면 그 `0` 이 실제 경계인 문단이 있다.
//!
//! ```text
//! 3-09월_교육_통합_2023  pi=512
//!   .hwp    vpos = 65968 / 61499(되감김) / 63001
//!   .hwpx   vpos = 65968 /      0        / 63001   ← 0 아닌 값이 이미 되감긴다
//!   정규화  vpos = 65968 /  67320        / 63001   ← 되감김이 idx=1 → idx=2 로 한 칸 밀림
//! ```
//!
//! 한 칸 밀리면 split 이 한 줄 늦어져 단 아래끝을 넘긴 줄이 계속 그려진다.
//!
//! 판별자는 **그 문단의 `0` 아닌 값들 사이에 되감김이 있는가**다.
//! `#1692`(`SO-SUEOP.hwpx` endnote 161)는 `[v, 0]` 이라 되감김이 없으므로 종전대로
//! 복원된다.
//!
//! upstream(edwardkim/rhwp) 커밋 `194ab18188`(#6495)의 통합 테스트를 이식했다. upstream은
//! `#4882`(HWP5-origin 보존 갈래) 위에 이 수정을 얹었지만, rhwp-code의
//! `normalize_hwpx_note_line_vpos`는 아직 `#4882`를 포팅하지 않아 그 갈래가 없다 — 이
//! 테스트는 `#6495`의 되감김 판별자만 검증한다.

#![cfg(not(target_arch = "wasm32"))]

use rhwp::model::control::Control;
use rhwp::model::paragraph::Paragraph;
use rhwp::parser::parse_document;

const SAMPLE: &str = "samples/3-09월_교육_통합_2022.hwpx";

fn collect_note_paragraphs<'a>(paragraphs: &'a [Paragraph], out: &mut Vec<&'a Paragraph>) {
    for paragraph in paragraphs {
        for control in &paragraph.controls {
            match control {
                Control::Endnote(note) => {
                    for p in &note.paragraphs {
                        out.push(p);
                    }
                    collect_note_paragraphs(&note.paragraphs, out);
                }
                Control::Footnote(note) => {
                    for p in &note.paragraphs {
                        out.push(p);
                    }
                    collect_note_paragraphs(&note.paragraphs, out);
                }
                Control::Table(table) => {
                    for cell in &table.cells {
                        collect_note_paragraphs(&cell.paragraphs, out);
                    }
                }
                _ => {}
            }
        }
    }
}

/// `0` 이 아닌 `vertical_pos` 들이 한 번이라도 되감기는가.
fn nonzero_vpos_rewinds(para: &Paragraph) -> bool {
    let mut prev = None;
    para.line_segs.iter().any(|seg| {
        if seg.vertical_pos <= 0 {
            return false;
        }
        let rewound = prev.is_some_and(|p| seg.vertical_pos < p);
        prev = Some(seg.vertical_pos);
        rewound
    })
}

#[test]
fn note_vpos_zero_survives_when_the_paragraph_already_rewinds() {
    let bytes = std::fs::read(SAMPLE).unwrap_or_else(|e| panic!("read {SAMPLE}: {e}"));
    let document = parse_document(&bytes).unwrap_or_else(|e| panic!("parse {SAMPLE}: {e:?}"));

    let mut notes = Vec::new();
    for section in &document.sections {
        collect_note_paragraphs(&section.paragraphs, &mut notes);
    }
    assert!(!notes.is_empty(), "이 표본에는 미주 문단이 있어야 한다");

    let preserved = notes
        .iter()
        .filter(|para| {
            nonzero_vpos_rewinds(para)
                && para
                    .line_segs
                    .iter()
                    .skip(1)
                    .any(|seg| seg.vertical_pos == 0)
        })
        .count();

    assert!(
        preserved > 0,
        "되감김이 있는 미주 문단의 `vertpos=0` 이 하나도 남지 않았다 — #6495 회귀. \
         정규화가 그 0 을 단조값으로 덮으면 되감김이 한 칸 밀려 split 이 한 줄 늦어지고, \
         9쪽 오른쪽 단이 용지 끝(841.17pt)까지 흐른다"
    );
}
