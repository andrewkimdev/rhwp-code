"""한컴 COM 오라클 — 문서를 열어 보거나, 한컴에게 저장시켜 정답지를 만든다.

`rhwp` 가 저장한 HWP 를 한컴이 **열지 못하는** 결함(#3565 계열)을 추적할 때 쓴다.
rhwp 자기 파서는 자기가 쓴 파일을 그대로 되읽으므로 `convert --verify` 로는 잡히지
않는다. 판정자는 한컴뿐이다.

## 쓰임

    # 후보 파일이 열리는가 (rc=0 열림 / rc=1 못 엶)
    python tools/hwp_open_bisect/hangul_com.py check-open out.hwp

    # 한컴에게 같은 원본을 HWP5 로 저장시켜 **정답지** 를 만든다
    python tools/hwp_open_bisect/hangul_com.py save-as src.hwpx oracle.hwp

## 주의

- **인스턴스를 한 프로세스에서 재생성하지 말 것.** 두 번째 `Hwp()` 는 `com_error` 로
  죽는다. 파일 하나당 프로세스 하나로 실행한다(이 스크립트가 그 단위다).
- 여러 판정을 **동시에 돌리지 말 것.** 서로의 `Hwp.exe` 를 죽여 "무응답" 오판을 만든다.
- 열리지 않는 문서는 한컴이 멎기도 한다. 호출 측에서 시간 제한을 걸고, 끝난 뒤
  남은 `Hwp.exe`/`HwpFrame.exe` 를 정리한다.
"""

import argparse
import sys


def _hwp():
    from pyhwpx import Hwp

    return Hwp(visible=False)


def check_open(path: str) -> int:
    """한컴으로 열어 본다. 열리면 쪽수를 찍고 0, 못 열면 1."""
    hwp = _hwp()
    try:
        ok = bool(hwp.open(path))
        pages = hwp.PageCount if ok else 0
        print(f"open={ok} pages={pages}")
        return 0 if ok else 1
    except Exception as exc:  # noqa: BLE001 - COM 예외 종류가 다양하다
        print(f"예외 {type(exc).__name__}: {exc}")
        return 2
    finally:
        _quit(hwp)


def save_as(src: str, dst: str) -> int:
    """한컴에게 `src` 를 열어 `dst`(HWP5) 로 저장시킨다 — 대조용 정답지."""
    hwp = _hwp()
    try:
        if not hwp.open(src):
            print(f"open=False — 원본을 한컴이 열지 못한다: {src}")
            return 1
        print(f"open=True pages={hwp.PageCount}")
        ok = bool(hwp.save_as(dst, "HWP"))
        print(f"saveas={ok}")
        return 0 if ok else 1
    except Exception as exc:  # noqa: BLE001
        print(f"예외 {type(exc).__name__}: {exc}")
        return 2
    finally:
        _quit(hwp)


def _quit(hwp) -> None:
    for call in ("clear", "quit"):
        try:
            getattr(hwp, call)()
        except Exception:  # noqa: BLE001 - 정리 실패는 판정에 영향 없다
            pass


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    sub = parser.add_subparsers(dest="cmd", required=True)

    p_open = sub.add_parser("check-open", help="한컴으로 열리는지 판정한다")
    p_open.add_argument("path")

    p_save = sub.add_parser("save-as", help="한컴에게 저장시켜 정답지를 만든다")
    p_save.add_argument("src")
    p_save.add_argument("dst")

    args = parser.parse_args(argv)
    if args.cmd == "check-open":
        return check_open(args.path)
    return save_as(args.src, args.dst)


if __name__ == "__main__":
    sys.exit(main())
