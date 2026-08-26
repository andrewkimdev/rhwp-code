//! tests — json.rs 에서 무변동 이동
//! Task #3315: `imageBytes` 최상위 값과 op 단위 `imageBytesOmitted` 의 관계를 못박는다.
//!
//! 생략은 신원 키를 낼 수 있는 op 만 대상이므로 **한 문서 안에 두 종류가 섞일 수 있다.**
//! 그 상태에서 최상위 값이 무엇을 뜻하는지가 열려 있었다 — 요청 모드인가, 모든 op 의 실제
//! 저장 방식인가. 여기서 **요청 모드**로 확정하고, 그러므로 소비자는 op 단위 표식을 봐야
//! 한다는 것을 고정한다.
//!
//! samples 전수에 키 없는 그림이 섞인 문서가 없어(전 표본 `cacheable:true`) 문서 단위
//! 테스트로는 이 상태를 만들 수 없다. 그래서 트리를 직접 조립한다.

use super::LayerJsonOptions;
use crate::paint::{LayerNode, PageLayerTree, PaintOp};
use crate::renderer::render_tree::{BoundingBox, ImageNode};
use serde_json::Value;

/// 키를 낼 수 있는 그림(`bin_data_id != 0`)과 낼 수 없는 합성 그림(`0`)을 함께 담은 트리.
fn mixed_tree() -> PageLayerTree {
    let png = vec![0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];
    PageLayerTree::new(
        120.0,
        80.0,
        LayerNode::leaf(
            BoundingBox::new(0.0, 0.0, 120.0, 80.0),
            None,
            vec![
                PaintOp::image(
                    BoundingBox::new(0.0, 0.0, 10.0, 10.0),
                    ImageNode::new(7, Some(png.clone())),
                    None,
                ),
                PaintOp::image(
                    BoundingBox::new(20.0, 0.0, 10.0, 10.0),
                    ImageNode::new(0, Some(png)),
                    None,
                ),
            ],
        ),
    )
}

fn image_ops(json: &str) -> Vec<Value> {
    let value: Value = serde_json::from_str(json).expect("valid layer JSON");
    value["root"]["ops"]
        .as_array()
        .expect("ops")
        .iter()
        .filter(|op| op.get("type").and_then(Value::as_str) == Some("image"))
        .cloned()
        .collect()
}

#[test]
fn issue_3315_top_level_image_bytes_is_the_requested_mode_not_a_per_op_guarantee() {
    let json = mixed_tree().to_json_with_options(LayerJsonOptions {
        omit_image_bytes: true,
    });
    let ops = image_ops(&json);
    assert_eq!(ops.len(), 2);

    assert!(
        json.contains("\"imageBytes\":\"byKey\""),
        "요청한 모드를 최상위에 싣는다"
    );

    // 키가 있는 op — 생략됐고 키로 받아야 한다.
    assert!(ops[0].get("base64").is_none(), "키 있는 op 은 생략된다");
    assert_eq!(ops[0]["imageBytesOmitted"].as_bool(), Some(true));
    assert!(ops[0].get("sourceImageKey").is_some());

    // 키가 없는 합성 op — 되찾을 길이 없으므로 base64 를 유지한다.
    assert!(
        ops[1].get("base64").is_some(),
        "키 없는 op 의 바이트를 빼면 소비자가 그 그림을 되찾을 방법이 없다"
    );
    assert!(
        ops[1].get("imageBytesOmitted").is_none(),
        "생략하지 않은 op 에 생략 표식을 달면 소비자가 헛되게 키를 찾는다"
    );
    assert!(ops[1].get("sourceImageKey").is_none());
}

#[test]
fn issue_3315_inline_mode_marks_no_op_as_omitted() {
    let json = mixed_tree().to_json();
    assert!(json.contains("\"imageBytes\":\"inline\""));
    assert!(!json.contains("\"imageBytesOmitted\""));
    for op in image_ops(&json) {
        assert!(op.get("base64").is_some());
    }
}
