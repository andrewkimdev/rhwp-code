"""중첩 표가 부모 셀 안에서 시작하는지 검사한다 (#3637 `table_layout.rs` 축).

`rhwp export-render-tree` 가 내는 쪽별 render tree JSON 을 훑어 두 가지를 센다.

- **시작 y 가 부모 셀 밖인 중첩 표** — 이 축의 결함 그 자체.
- **쪽 아래로 넘어간 깊이** — 컨테이너가 통째로 셀 밑에 놓였을 때 커지는 값으로,
  `tests/issue_3637_nested_table_starts_inside_parent_cell.rs` 가 계약하는 양이다.

셀보다 *큰* 중첩 표는 시작이 셀 안이어도 바닥이 넘친다. 그건 쪽보다 큰 행의 분할 정책
축(3회 반증)이라 여기서 세지 않는다 — 시작 y 만 본다.

사용:
  python tools/nested_table_containment.py --exe ./target/debug/rhwp.exe 문서.hwpx
  python tools/nested_table_containment.py --tree-dir output/render_trees   # 이미 뽑아둔 것

수정 전후 비교는 바이너리 두 개로 각각 돌려 값을 대조한다.
"""

import argparse
import glob
import json
import os
import subprocess
import sys
import tempfile

sys.stdout.reconfigure(encoding="utf-8", errors="replace")

# 부모 셀 경계를 벗어난 것으로 셀 때 무시할 오차(px).
TOLERANCE_PX = 0.5


def scan_page(tree):
    """(셀 밖에서 시작한 중첩 표 목록, 이 쪽에서 가장 깊은 y, 쪽 높이)."""
    escapes = []
    deepest = 0.0

    def walk(node, cell):
        nonlocal deepest
        box = node.get("bbox") or {}
        if box:
            deepest = max(deepest, box.get("y", 0.0) + box.get("h", 0.0))
        if node.get("type") == "Table" and cell is not None:
            parent = cell["bbox"]
            top = parent["y"]
            bottom = parent["y"] + parent["h"]
            start = box.get("y", 0.0)
            if start > bottom + TOLERANCE_PX:
                escapes.append((start, top, bottom, start - bottom))
            elif start < top - TOLERANCE_PX:
                escapes.append((start, top, bottom, top - start))
        inner = node if node.get("type") == "Cell" else cell
        for child in node.get("children", []):
            walk(child, inner)

    walk(tree, None)
    return escapes, deepest, tree["bbox"]["h"]


def export_trees(exe, doc, out_dir):
    run = subprocess.run(
        [exe, "export-render-tree", doc, "-o", out_dir],
        capture_output=True, text=True, encoding="utf-8", errors="replace", timeout=1800,
    )
    if run.returncode != 0:
        print(f"export-render-tree 실패: {run.stderr[-500:]}")
        return False
    return True


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("document", nargs="?", help="검사할 .hwp/.hwpx")
    ap.add_argument("--exe", default="./target/debug/rhwp.exe")
    ap.add_argument("--tree-dir", default="", help="이미 뽑아둔 render tree JSON 폴더")
    ap.add_argument("--top", type=int, default=8, help="쪽별 상세 출력 개수")
    a = ap.parse_args()

    tmp = None
    tree_dir = a.tree_dir
    if not tree_dir:
        if not a.document:
            ap.error("document 또는 --tree-dir 가 필요하다")
        tmp = tempfile.mkdtemp(prefix="rhwp_rt_")
        if not export_trees(a.exe, a.document, tmp):
            return 1
        tree_dir = tmp

    pages = sorted(glob.glob(os.path.join(tree_dir, "render_tree_*.json")))
    if not pages:
        print(f"render tree JSON 이 없다: {tree_dir}")
        return 1

    total, worst_escape, deepest_overflow, deepest_page = 0, 0.0, 0.0, 0
    rows = []
    for path in pages:
        with open(path, encoding="utf-8") as fh:
            tree = json.load(fh)
        escapes, deepest, height = scan_page(tree)
        overflow = deepest - height
        if overflow > deepest_overflow:
            deepest_overflow, deepest_page = overflow, path
        if escapes:
            total += len(escapes)
            worst = max(e[3] for e in escapes)
            worst_escape = max(worst_escape, worst)
            rows.append((os.path.basename(path), len(escapes), worst, overflow))

    print(f"시작 y 가 부모 셀 밖인 중첩 표  {total}건 / 쪽 {len(rows)}개")
    print(f"최대 이탈                      {worst_escape:.1f}px")
    print(f"쪽 아래로 넘어간 최대 깊이      {deepest_overflow:.1f}px"
          f"  ({os.path.basename(deepest_page) if deepest_page else '-'})")
    if rows:
        print("\n  쪽                      건수  최대이탈   쪽아래깊이")
        for name, n, worst, overflow in rows[: a.top]:
            print(f"  {name:<22} {n:>4}  {worst:>8.1f}  {overflow:>10.1f}")
    if tmp:
        print(f"\n(render tree: {tmp})")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
