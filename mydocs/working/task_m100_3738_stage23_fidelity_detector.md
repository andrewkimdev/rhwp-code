---
kind: investigation
status: active
canonical: mydocs/manual/bug_hunting_playbook.md
last_verified: 2026-08-02
---

# Task #3738 Stage 23 fidelity detector — Square 그림·본문 교차의 전수 후보화

## 문제 재현

`fidelity_compare.py`의 기존 빠른 direct-pair 경로를 native HWP p126–128(0-based 125–127)에 실행하면,
사용자가 지적한 human p127은 다음처럼 결함 후보가 아니다.

| 산출물 | p127 결과 | 놓치는 이유 |
| --- | --- | --- |
| `text-report.tsv` | `reference_only=0`, `svg_only=0` | 같은 문자를 다른 폭·좌표에 그려 문자 multiset이 같다. |
| `layout-candidates.tsv` | `0,0,0,0` | 기존 네 열은 body/각주, table/footer, table/frame, image/frame만 센다. |
| pixel mode | raw diff score 16.12% | 순위용 값일 뿐 구조 flag가 없다. 현재 Mac에는 `pypdfium2`가 없어 CLI pixel mode도 실행되지 않는다. |

즉, fidelity의 text-only 전수 경로는 p127처럼 page 안에서 Square 그림과 본문이 교차하는 결함을 자동으로
구분하지 못한다. 임의 pixel 합격선은 폰트 차이도 크게 반응하므로 해결책이 아니다.

## 구현 계약

1. `export-render-tree` JSON의 Image에 source owner `pi`/`ci`와 `textWrap`을 노출한다. 기존 `ImageNode`가
   이미 가진 정보를 외부 후보 도구가 읽을 수 있게 하는 호환 확장이다.
2. `fidelity_compare.square_wrap_text_overlap_candidates`는 Body의 `Square`/`Tight`/`Through` 이미지 중
   80px 이상인 대상에 대해, 이미지 폭의 절반 이상을 가로지르는 TextLine이 3행 이상이면 후보로 센다.
3. `BehindText`/`InFrontOfText`는 의도된 overlay일 수 있으므로 제외한다. 이 값은 결함 확정이 아니라
   `layout-candidates.tsv`의 `square_wrap_text_overlap` column으로 남는 high-priority review 후보이다.
4. native binary를 정확한 detector code revision으로 빌드한 뒤 p127 direct text-only + layout ledger를
   재실행해 해당 열이 양수인지 고정한다. 이어 p127 renderer fix 이후에도 후보가 0으로 내려가는지 확인한다.

## 코드 회귀

- Rust unit: `render_tree_json_exposes_image_owner_and_text_wrap`
- Python positive: Square 이미지와 3개 Body line의 교차를 1 candidate로 판정
- Python negative: `InFrontOfText` 이미지의 같은 기하는 후보로 판정하지 않음

다음 evidence 단계 전까지 이 문서는 `active`다. p127 renderer 자체의 narrow wrap anchor 복원은
`task_m100_3738_stage23.md`의 별도 code stage로 진행하며, 이 detector는 그 전후의 빠른 전수 triage를
제공한다.
