"""차등 게이트 오케스트레이터 — 시나리오 전체를 양쪽에 돌리고 판정한다 (P0).

한 번의 호출로 아래를 한다.

1. 시나리오마다 **새 프로세스**로 오라클 러너를 돌린다(COM 규약: 문서당 프로세스 1개).
2. 시간 제한을 걸고, 넘기면 죽인 뒤 남은 `Hwp.exe`/`HwpFrame.exe` 를 정리한다.
3. rhwp 러너를 돌린다(여기는 프로세스 격리가 필요 없다).
4. `compare.py` 로 판정하고 요약을 찍는다.

## 쓰임

    python tools/hwpctrl_compat/run_gate.py --impl legacy
    python tools/hwpctrl_compat/run_gate.py --only field-read --timeout 300

## 왜 직렬인가

COM 판정을 동시에 돌리면 서로의 `Hwp.exe` 를 죽여 "무응답" 오판을 만든다. 병렬화하지 말 것.
"""

from __future__ import annotations

import argparse
import io
import json
import subprocess
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[1]
SCENARIO_DIR = HERE / "scenarios"
OUT_ROOT = REPO / "output" / "poc" / "hwpctrl"

# 오라클은 **한글2022(12.x)** 로 고정한다(계획서 §9-4). 이 머신에는 2024(13.x)도 깔려 있어서
# 고정하지 않으면 두 버전의 정답지가 한 표에 섞인다. 전환 방법은 계획서 §4.5.1.
ORACLE_VERSION_PREFIX = "12,"


def kill_stray() -> None:
    """남은 한글 프로세스를 정리한다. 다음 시나리오가 그 인스턴스에 붙으면 결과가 오염된다."""
    for image in ("Hwp.exe", "HwpFrame.exe"):
        subprocess.run(
            ["taskkill", "/F", "/IM", image],
            capture_output=True,
            check=False,
        )


def run_ocx(scenario: Path, out_dir: Path, timeout: int, expect_version: str | None) -> str:
    cmd = [sys.executable, str(HERE / "runner_ocx.py"), str(scenario), "--out", str(out_dir)]
    if expect_version:
        cmd += ["--expect-version", expect_version]
    try:
        proc = subprocess.run(cmd, capture_output=True, timeout=timeout, check=False)
    except subprocess.TimeoutExpired:
        kill_stray()
        return "STALL"
    sys.stdout.write(proc.stdout.decode("utf-8", "replace"))
    if proc.returncode == 3:
        return "VERSION"
    if proc.returncode != 0:
        sys.stderr.write(proc.stderr.decode("utf-8", "replace")[-2000:])
        return "ERR"
    return "OK"


def run_rhwp(scenario: Path, out_dir: Path, impl: str, timeout: int) -> str:
    cmd = [
        "node",
        str(HERE / "runner_rhwp.mjs"),
        str(scenario),
        "--out",
        str(out_dir),
        "--impl",
        impl,
    ]
    try:
        proc = subprocess.run(cmd, capture_output=True, timeout=timeout, check=False)
    except subprocess.TimeoutExpired:
        return "STALL"
    sys.stdout.write(proc.stdout.decode("utf-8", "replace"))
    if proc.returncode != 0:
        sys.stderr.write(proc.stderr.decode("utf-8", "replace")[-2000:])
        return "ERR"
    return "OK"


def main() -> int:
    sys.stdout.reconfigure(encoding="utf-8")
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--impl", default="legacy", help="rhwp 측 구현 (legacy | 패키지 엔트리 경로)")
    ap.add_argument("--only", help="시나리오 id 하나만")
    ap.add_argument("--timeout", type=int, default=300, help="시나리오당 초 (COM 무응답 대비)")
    ap.add_argument(
        "--expect-version",
        default=ORACLE_VERSION_PREFIX,
        help=(
            f"오라클 버전 접두사 고정 (기본 '{ORACLE_VERSION_PREFIX}' = 한글2022). "
            "빈 문자열을 주면 검사하지 않는다 — 두 버전의 결과가 섞이므로 권하지 않는다."
        ),
    )
    ap.add_argument("--skip-ocx", action="store_true", help="오라클 재실행 없이 기존 정답지 사용")
    args = ap.parse_args()

    scenarios = sorted(SCENARIO_DIR.glob("*.json"))
    if args.only:
        scenarios = [p for p in scenarios if p.stem == args.only]
    if not scenarios:
        print("시나리오 없음")
        return 2

    ocx_dir = OUT_ROOT / "ocx"
    rhwp_dir = OUT_ROOT / "rhwp"
    verdict_dir = OUT_ROOT / "verdict"
    for d in (ocx_dir, rhwp_dir, verdict_dir):
        d.mkdir(parents=True, exist_ok=True)

    status = {}
    for path in scenarios:
        name = path.stem
        if args.skip_ocx:
            status[name] = "SKIPPED" if (ocx_dir / f"{name}.returns.json").exists() else "NO_ORACLE"
        else:
            kill_stray()
            started = time.monotonic()
            status[name] = run_ocx(path, ocx_dir, args.timeout, args.expect_version)
            print(f"  오라클 {name}: {status[name]} ({time.monotonic() - started:.1f}s)")
        rhwp_status = run_rhwp(path, rhwp_dir, args.impl, args.timeout)
        print(f"  rhwp {name}: {rhwp_status}")
        if rhwp_status != "OK":
            status[name] = f"{status[name]}/RHWP_{rhwp_status}"
    kill_stray()

    subprocess.run(
        [
            sys.executable,
            str(HERE / "compare.py"),
            "--ocx",
            str(ocx_dir),
            "--rhwp",
            str(rhwp_dir),
            "--out",
            str(verdict_dir),
        ],
        check=False,
    )

    with io.open(verdict_dir / "run_status.json", "w", encoding="utf-8", newline="\n") as fh:
        json.dump({"impl": args.impl, "status": status}, fh, ensure_ascii=False, indent=2)
        fh.write("\n")
    bad = {k: v for k, v in status.items() if v not in ("OK", "SKIPPED")}
    if bad:
        print(f"실행 문제: {bad}")
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
