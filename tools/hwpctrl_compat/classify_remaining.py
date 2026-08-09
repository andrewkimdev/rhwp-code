"""남은 원장 항목을 **왜 못 올렸는지**로 갈래 지어 도달 가능한 상한을 낸다.

원장 484 가 도달 가능한 상한이 아니다 — 대화상자를 띄우는 액션은 COM 이 답을 못 주고, 이
빌드에 아예 없는 API 도 있다. 그 선을 숫자로 그어 두면 다음 사람이 "남은 것을 다 할 수 있다"는
잘못된 기대로 시간을 태우지 않는다.

    python tools/hwpctrl_compat/classify_remaining.py
"""

from __future__ import annotations

import io
import json
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[1]
LEDGER = REPO / "npm" / "hwpctrl-ocx" / "spec" / "api_ledger.json"
SWEEPS = [
    REPO / "output" / "poc" / "hwpctrl" / "sweep_actions.tsv",
    REPO / "output" / "poc" / "hwpctrl" / "sweep_shapeobj.tsv",
    REPO / "output" / "poc" / "hwpctrl" / "sweep_table.tsv",
    REPO / "output" / "poc" / "hwpctrl" / "sweep_cellblock.tsv",
]

# 이 COM 개체에 **아예 없는** 것들(AttributeError 실측).
ABSENT = {
    "HwpCtrl.method.GetTableCellAddr", "HwpCtrl.method.GetViewStatus",
    "HwpCtrl.property.ScrollPosInfo", "HwpCtrl.property.ReadOnlyMode",
    "HwpCtrl.method.GetCtrlHorizontalOffset", "HwpCtrl.method.GetCtrlVerticalOffset",
    "HwpCtrl.method.GetTextBySet", "HwpCtrl.method.SaveDocument",
    "HwpCtrl.method.MoveToFieldEx", "HwpCtrl.method.OpenDocument",
    "ParameterSet.method.GetInterSection", "ParameterSet.DrawLayout",
}

# 반환값 말고는 관측할 것이 없는 UI 계열.
UI_ONLY = {
    "HwpCtrl.method.SetToolBar", "HwpCtrl.method.ShowCaret", "HwpCtrl.method.ShowRibbon",
    "HwpCtrl.method.ShowStatusBar", "HwpCtrl.method.ShowToolBar",
    "HwpCtrl.method.ShowHorizontalScroll", "HwpCtrl.method.ShowVerticalScroll",
    "HwpCtrl.method.GetMousePos", "HwpCtrl.method.PrintDocument",
    "HwpCtrl.method.IsSpellCheckCompleted", "HwpCtrl.method.CreatePageImage",
    "HwpCtrl.method.CreatePageImageEx",
    "Action.ViewZoomNormal", "Action.ViewZoomFitPage", "Action.ViewZoomFitWidth",
    "Action.ToggleOverwrite",
}

# 조판(쪽·줄)이 맞아야 잴 수 있는 것들.
LAYOUT = {
    "HwpCtrl.method.KeyIndicator", "HwpCtrl.method.GetPageText",
    "Action.TableColPageDown", "Action.TableColPageUp",
    "Action.MoveLineBegin", "Action.MoveLineEnd", "Action.MoveLineUp", "Action.MoveLineDown",
    "Action.MoveUp", "Action.MoveDown", "Action.MovePageBegin", "Action.MovePageEnd",
    "Action.MovePageUp", "Action.MovePageDown", "Action.MoveViewBegin", "Action.MoveViewEnd",
    "Action.MoveViewUp", "Action.MoveViewDown", "Action.MoveScrollUp", "Action.MoveScrollDown",
    "Action.MoveScrollNext", "Action.MoveScrollPrev", "Action.DeleteLine",
    "Action.DeleteLineEnd", "Action.MoveSelLineBegin", "Action.MoveSelLineEnd",
    "Action.MoveSelLineUp", "Action.MoveSelLineDown", "Action.MoveSelUp", "Action.MoveSelDown",
    "Action.MoveSelPageUp", "Action.MoveSelPageDown", "Action.MoveSelViewUp",
    "Action.MoveSelViewDown", "Action.ParagraphShapeIndentAtCaret",
}

# 머신(설치 글꼴·음력 자료)에 달린 것들.
MACHINE = {
    "Action.CharShapeNextFaceName", "Action.CharShapePrevFaceName",
    "HwpCtrl.method.LunarToSolar", "HwpCtrl.method.LunarToSolarBySet",
    "HwpCtrl.method.SolarToLunar", "HwpCtrl.method.SolarToLunarBySet",
}


def sweep_kinds() -> dict[str, str]:
    # 스윕이 아예 안 건 것들 — 이미 대화상자로 확인해 금지 목록에 넣은 이름이다.
    from sweep_actions import FORBIDDEN

    kinds: dict[str, str] = {name: "DIALOG" for name in FORBIDDEN}
    for path in SWEEPS:
        if not path.exists():
            continue
        for line in io.open(path, encoding="utf-8").read().splitlines()[1:]:
            parts = line.split("\t")
            if len(parts) < 2:
                continue
            name, kind = parts[0], parts[1]
            # 맥락을 붙여 살아난 것이 있으면 그 결과를 우선한다.
            if kinds.get(name) in ("CHANGED", "MOVED"):
                continue
            kinds[name] = kind
    return kinds


def main() -> int:
    sys.stdout.reconfigure(encoding="utf-8")
    doc = json.loads(LEDGER.read_text(encoding="utf-8"))

    def walk(node):
        if isinstance(node, dict):
            if "id" in node and "status" in node:
                yield node
            for value in node.values():
                yield from walk(value)
        elif isinstance(node, list):
            for value in node:
                yield from walk(value)

    items = list(walk(doc))
    done = [i for i in items if i.get("status") in ("verified", "substituted")]
    rest = [i for i in items if i not in done]
    kinds = sweep_kinds()

    buckets: dict[str, list[str]] = {}
    for item in rest:
        ident = item["id"]
        action = ident.split(".", 1)[1] if ident.startswith("Action.") else None
        if ident in ABSENT:
            key = "없는 API"
        elif ident in UI_ONLY:
            key = "UI 전용(관측 불가)"
        elif ident in MACHINE:
            key = "머신 의존"
        elif ident in LAYOUT:
            key = "조판 의존"
        elif action and kinds.get(action) == "DIALOG":
            key = "대화상자"
        elif action and kinds.get(action) in ("CHANGED", "MOVED"):
            key = "관측됨 — 다음 후보"
        elif action and kinds.get(action) == "NOOP":
            key = "무동작(맥락 더 필요)"
        else:
            key = "아직 안 잼"
        buckets.setdefault(key, []).append(ident)

    print(f"원장 {len(items)} · 완료 {len(done)} · 남은 {len(rest)}\n")
    blocked = 0
    for key in sorted(buckets, key=lambda k: -len(buckets[k])):
        names = buckets[key]
        if key in ("없는 API", "UI 전용(관측 불가)", "머신 의존", "대화상자"):
            blocked += len(names)
        print(f"  {key:<22} {len(names):>4}")
        if key in ("관측됨 — 다음 후보", "아직 안 잼"):
            for j in range(0, len(names), 3):
                print("      " + "  ".join(f"{n:<34}" for n in names[j : j + 3]))
    reachable = len(items) - blocked
    print(f"\n구조적으로 막힌 것 {blocked} → **도달 가능한 상한 {reachable}/{len(items)}**")
    print(f"지금 {len(done)} — 상한까지 {reachable - len(done)} 남음")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
