"""남은 액션을 **하나씩** 걸어 갈래를 분류한다 — 대화상자·무동작·움직임.

왜 필요한가: 남은 원장의 대부분이 액션인데, 어느 것이 대화상자를 띄우고 어느 것이 관측
가능한지 목록이 없다. 하나씩 짧은 시한으로 걸어 그 지도를 만든다. 멈추면 대화상자이므로
곧바로 한글을 죽이고 다음으로 간다 — 그 이름은 다시 걸지 않는다.

    python tools/hwpctrl_compat/sweep_actions.py [--out 결과.tsv] [--limit N]

결과는 TSV 다: 이름, 갈래(DIALOG/NOOP/MOVED/CHANGED/FAIL), 무엇이 달라졌는지.
"""

from __future__ import annotations

import argparse
import io
import json
import subprocess
import sys
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[1]
LEDGER = REPO / "npm" / "hwpctrl-ocx" / "spec" / "api_ledger.json"
SAMPLE = "samples/para-001.hwp"

# 이미 대화상자로 확인된 것들 — 다시 걸지 않는다(계획서 §4.32).
FORBIDDEN = {
    "PutBullet", "PutParaNumber", "PutOutlineNumber", "ParaNumberBullet",
    "CharShapeHeight", "CharShapeWidth", "CharShapeSpacing",
    "FindDlg", "ReplaceDlg", "ShapeObjDialog", "TablePropertyDialog",
    "PictureInsertDialog", "Print", "PageSetup", "HeaderFooter", "DocSummaryInfo",
    "SpellingCheck", "Hyperlink", "InsertHyperlink", "ModifyHyperlink",
}


def remaining_actions() -> list[str]:
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

    names = []
    for item in walk(doc):
        ident = item["id"]
        if not ident.startswith("Action.") or item.get("status") in ("verified", "substituted"):
            continue
        name = ident.split(".", 1)[1]
        if "." in name or name in FORBIDDEN:
            continue
        names.append(name)
    return sorted(names)


def observables(action: str) -> list:
    """액션 앞뒤로 같은 것을 읽는다 — 무엇이 달라졌는지 갈래를 보려는 것."""
    probe = [
        ["GetPos", []],
        ["SelectionMode", []],
        ["CharShape.Item", ["Height"]],
        ["ParaShape.Item", ["AlignType"]],
        ["SetPos", [0, 0, 10000000]],
        ["GetPos", []],
    ]
    return (
        [["SetPos", [0, 0, 20]]]
        + probe
        + [["SetPos", [0, 0, 20]], ["Run", [action]]]
        + probe
    )


def classify(before: list, after: list) -> tuple[str, str]:
    labels = ["캐럿", "모드", "글자높이", "정렬", "-", "문단끝"]
    diffs = [
        f"{labels[i]}:{json.dumps(b, ensure_ascii=False)}→{json.dumps(a, ensure_ascii=False)}"
        for i, (b, a) in enumerate(zip(before, after))
        if b != a and labels[i] != "-"
    ]
    if not diffs:
        return "NOOP", ""
    only_caret = all(d.startswith("캐럿") for d in diffs)
    return ("MOVED" if only_caret else "CHANGED"), " | ".join(diffs)


def main() -> int:
    sys.stdout.reconfigure(encoding="utf-8")
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--out", type=Path, default=REPO / "output" / "poc" / "hwpctrl" / "sweep_actions.tsv")
    ap.add_argument("--limit", type=int, default=0)
    ap.add_argument("--timeout", type=int, default=70)
    args = ap.parse_args()

    names = remaining_actions()
    if args.limit:
        names = names[: args.limit]
    args.out.parent.mkdir(parents=True, exist_ok=True)
    tmp = Path(tempfile.mkdtemp(prefix="sweep-"))
    rows = []
    print(f"액션 {len(names)}개를 하나씩 건다 (시한 {args.timeout}초)")

    for i, action in enumerate(names, 1):
        scenario = {
            "id": f"sweep-{action}",
            "title": action,
            "ledger": [],
            "open": SAMPLE,
            "calls": observables(action),
        }
        path = tmp / f"sweep-{action}.json"
        with io.open(path, "w", encoding="utf-8", newline="\n") as fh:
            json.dump(scenario, fh, ensure_ascii=False)
        subprocess.run(["taskkill", "/F", "/IM", "Hwp.exe"], capture_output=True, check=False)
        try:
            proc = subprocess.run(
                [sys.executable, str(HERE / "runner_ocx.py"), str(path), "--out", str(tmp),
                 "--expect-version", "12"],
                capture_output=True, timeout=args.timeout, check=False,
            )
        except subprocess.TimeoutExpired:
            rows.append((action, "DIALOG", "시한 안에 안 끝났다 — 대화상자로 본다"))
            subprocess.run(["taskkill", "/F", "/IM", "Hwp.exe"], capture_output=True, check=False)
            print(f"  [{i}/{len(names)}] {action}: DIALOG")
            continue
        if proc.returncode != 0:
            rows.append((action, "FAIL", f"종료코드 {proc.returncode}"))
            print(f"  [{i}/{len(names)}] {action}: FAIL {proc.returncode}")
            continue
        data = json.loads((tmp / f"sweep-{action}.returns.json").read_text(encoding="utf-8"))
        values = [c.get("value") for c in data["calls"] if c["call"] in
                  ("GetPos", "SelectionMode", "CharShape.Item", "ParaShape.Item")]
        # 앞 6, 뒤 6 (SetPos 는 값이 아니라 빠진다)
        half = len(values) // 2
        kind, detail = classify(values[:half], values[half:])
        rows.append((action, kind, detail))
        print(f"  [{i}/{len(names)}] {action}: {kind} {detail[:70]}")

    with io.open(args.out, "w", encoding="utf-8", newline="\n") as fh:
        fh.write("action\tkind\tdetail\n")
        for row in rows:
            fh.write("\t".join(row) + "\n")
    counts: dict[str, int] = {}
    for _, kind, _ in rows:
        counts[kind] = counts.get(kind, 0) + 1
    print(f"\n갈래별: {counts}")
    print(f"→ {args.out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
