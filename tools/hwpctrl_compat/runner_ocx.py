"""오라클 러너 — 설치된 한글(COM)에 시나리오를 실행시킨다 (P0).

판정자는 문서가 아니라 **설치된 한글**이다. 이 러너가 정답지를 만든다.

## 쓰임

    python tools/hwpctrl_compat/runner_ocx.py scenarios/field-read.json --out output/poc/hwpctrl/ocx

## COM 규약 (어기면 오판이 난다)

- **문서 하나당 프로세스 하나.** 한 프로세스에서 `Hwp()` 를 두 번 만들면 `com_error` 로 죽는다.
  이 스크립트가 그 단위다 — 호출 측(`run_gate.py`)이 프로세스를 띄우고 시간 제한을 건다.
- **동시에 여러 판정을 돌리지 말 것.** 서로의 `Hwp.exe` 를 죽여 "무응답" 오판을 만든다.
- 보안 모듈(`FilePathCheckerModule.dll`)이 등록돼 있어야 파일 접근 다이얼로그가 뜨지 않는다.
  `pyhwpx` 가 `register_module=True` 로 처리한다.

## WebHwpCtrl ↔ COM 의미 차이

웹한글컨트롤(v2.4 §2.2)은 ActiveX 와 **호출 규약이 다르다**. 포인터로 받던 값을 객체로
돌려주고, 서버 접근이 필요한 API 는 콜백을 받는다. 대조가 성립하려면 COM 쪽 반환을
**웹 쪽 형태로 정규화**해야 한다. 그 변환이 `ADAPTERS` 다. 여기에 없는 API 는 COM 반환을
그대로 쓴다(스칼라는 두 규약이 같다).
"""

from __future__ import annotations

import argparse
import io
import json
import sys
import traceback
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]


def normalize(value):
    """COM VARIANT → JSON 으로 실을 수 있는 값.

    객체는 값을 못 뽑으므로 **타입 이름만** 남긴다. 양쪽 러너가 같은 규칙을 쓰므로
    "객체가 돌아왔다"는 사실 자체는 대조된다.
    """
    if value is None or isinstance(value, (bool, int, float, str)):
        return value
    if isinstance(value, (list, tuple)):
        return [normalize(v) for v in value]
    if isinstance(value, dict):
        return {k: normalize(v) for k, v in value.items()}
    return {"__type": type(value).__name__}


# 웹 규약으로 되돌리는 변환. `com` 은 raw COM 객체다.
ADAPTERS = {
    # v2.4 §8.3.12 — 웹은 {list, para, pos} 객체를 리턴한다.
    "GetPos": lambda com, args: dict(zip(("list", "para", "pos"), com.GetPos())),
    # v2.4 §8.3.14 — 웹은 {slist, spara, spos, elist, epara, epos} 객체를 리턴한다.
    "GetSelectedPos": lambda com, args: dict(
        zip(("result", "slist", "spara", "spos", "elist", "epara", "epos"), com.GetSelectedPos())
    ),
    # v2.4 §8.3.27 — 웹은 {secno, prnpageno, colno, line, pos, over, ctrlname} 객체.
    "KeyIndicator": lambda com, args: dict(
        zip(
            ("result", "secno", "prnpageno", "colno", "line", "pos", "over", "ctrlname"),
            com.KeyIndicator(),
        )
    ),
}


def call_one(com, name: str, args: list):
    """메서드면 호출하고, 속성이면 읽는다. 반환은 정규화한다."""
    adapter = ADAPTERS.get(name)
    if adapter:
        return normalize(adapter(com, args))
    attr = getattr(com, name)
    if callable(attr):
        return normalize(attr(*args))
    return normalize(attr)


def run(scenario: dict, out_dir: Path, expect_version: str | None = None) -> dict:
    from pyhwpx import Hwp

    result = {
        "scenario": scenario["id"],
        "runner": "ocx",
        "oracle": None,
        "calls": [],
        "saved": None,
        "fatal": None,
    }
    hwp = Hwp(new=True, visible=False)
    com = hwp.hwp
    # 어느 한글이 답했는지 **매 실행 기록한다**. 이 머신에는 2022 와 2024 가 함께 깔려 있고
    # ProgID `HWPFrame.HwpObject` 가 어느 쪽으로 붙는지는 등록 상태에 달렸다. 기록하지 않으면
    # 서로 다른 오라클의 결과를 같은 표에 섞게 된다.
    try:
        result["oracle"] = {"version": normalize(com.Version)}
    except Exception as exc:  # noqa: BLE001
        result["oracle"] = {"version": None, "error": f"{type(exc).__name__}: {exc}"}

    # 버전이 어긋나면 **시나리오를 아예 돌리지 않는다.** 돌린 뒤 거부하면 잘못된 버전의
    # 정답지가 이미 디스크에 남아, 다음 사람이 그것을 정답으로 쓴다.
    version = (result.get("oracle") or {}).get("version")
    if expect_version and not str(version or "").startswith(expect_version):
        result["rejected"] = f"기대 '{expect_version}…' 실제 '{version}'"
        try:
            com.Quit()
        except Exception:  # noqa: BLE001
            pass
        return result

    try:
        if scenario.get("open"):
            src = (REPO / scenario["open"]).resolve()
            opened = com.Open(str(src), "", "")
            result["calls"].append({"call": "Open", "args": [scenario["open"]], "value": normalize(opened)})

        for idx, call in enumerate(scenario.get("calls", [])):
            name, args = call[0], (call[1] if len(call) > 1 else [])
            record = {"call": name, "args": args}
            try:
                record["value"] = call_one(com, name, args)
            except Exception as exc:  # noqa: BLE001 — COM 예외 종류가 다양하다
                record["error"] = f"{type(exc).__name__}: {exc}"
            result["calls"].append(record)

        if scenario.get("saveAs"):
            dst = (out_dir / scenario["saveAs"]).resolve()
            dst.parent.mkdir(parents=True, exist_ok=True)
            ok = com.SaveAs(str(dst), "HWP", "")
            result["saved"] = {"path": str(dst.relative_to(REPO)) if REPO in dst.parents else str(dst), "ok": bool(ok)}
    except Exception:  # noqa: BLE001
        result["fatal"] = traceback.format_exc(limit=3)
    finally:
        try:
            com.Quit()
        except Exception:  # noqa: BLE001
            pass
    return result


def main() -> int:
    sys.stdout.reconfigure(encoding="utf-8")
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("scenario", help="시나리오 JSON 경로")
    ap.add_argument("--out", required=True, help="산출물 디렉터리")
    ap.add_argument(
        "--expect-version",
        help="오라클 버전 접두사(예: '12,' = 한글2022). 어긋나면 실행하지 않고 exit 3.",
    )
    args = ap.parse_args()

    with io.open(args.scenario, encoding="utf-8") as fh:
        scenario = json.load(fh)

    out_dir = Path(args.out)
    out_dir.mkdir(parents=True, exist_ok=True)
    result = run(scenario, out_dir, args.expect_version)

    version = (result.get("oracle") or {}).get("version")
    if result.get("rejected"):
        # 정답지 자리에 쓰지 않는다. 증거는 남기되 이름으로 구분한다.
        dst = out_dir / f"{scenario['id']}.rejected.json"
        with io.open(dst, "w", encoding="utf-8", newline="\n") as fh:
            json.dump(result, fh, ensure_ascii=False, indent=2)
            fh.write("\n")
        print(
            f"{scenario['id']}: 오라클 버전 불일치로 **실행하지 않음** — {result['rejected']}\n"
            "이 머신에는 한글2022(12.x)와 2024(13.x)가 함께 있다. 전환은 계획서 §4.5.1.",
        )
        return 3

    dst = out_dir / f"{scenario['id']}.returns.json"
    with io.open(dst, "w", encoding="utf-8", newline="\n") as fh:
        json.dump(result, fh, ensure_ascii=False, indent=2)
        fh.write("\n")
    print(f"{scenario['id']}: 호출 {len(result['calls'])}건 · 오라클 {version} → {dst}")
    return 1 if result["fatal"] else 0


if __name__ == "__main__":
    raise SystemExit(main())
