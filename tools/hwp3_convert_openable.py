#!/usr/bin/env python
"""HWP3 → HWP5 변환본을 **한글이 열 수 있는가**.

rhwp 는 자기가 쓴 파일을 읽을 수 있으므로 왕복 정합을 rhwp 리더로만 재면
"완벽" 으로 보인다. 실제 판정은 한컴이 여는지다.

한글 COM 은 open 실패 뒤 인스턴스가 죽어 다음 측정을 오염시킨다 —
**문서 1건당 새 프로세스 1개**로 격리한다. 이 도구는 기존 한글 세션에
접속하거나 다른 한글 프로세스를 종료하지 않는다. 개별 검사 timeout 은
검사 자식 프로세스만 끝내며, COM 서버 정리는 정상 종료 경로의 ``quit()`` 에
맡긴다.

사용:
  python tools/hwp3_convert_openable.py --exe <rhwp.exe> --list <hwp3목록.txt> [--limit N] [--out t.tsv]
  python hwp3_convert_openable.py --child <원본> <출력hwp>      (내부용)
"""
import argparse
import csv
import io
import os
import subprocess
import sys

sys.stdout.reconfigure(encoding="utf-8", errors="replace")


def child(src, dst):
    """한 문서만 열어 결과를 한 줄로 찍는다. 실패해도 부모는 살아남는다."""
    from pyhwpx import Hwp

    # pyhwpx 의 기본값(new=False)은 실행 중인 한글 COM 인스턴스에 연결할 수 있다.
    # 이 작업자는 자신이 만든 인스턴스만 quit 해야 하므로 새 hidden 인스턴스를
    # 명시한다.
    hwp = Hwp(new=True, visible=False)
    try:
        ok = bool(hwp.open(dst))
        pages = hwp.PageCount if ok else -1
        ok_src = bool(hwp.open(src)) if ok else None
        src_pages = hwp.PageCount if ok_src else -1
        print(f"RESULT\t{int(ok)}\t{pages}\t{src_pages}")
        hwp.clear(option=1)
    finally:
        try:
            hwp.quit()
        except Exception:  # noqa: BLE001
            pass


def run_child(src, dst, timeout):
    """분리된 검사 자식을 실행하고, timeout이면 그 자식만 중단한다.

    전역 image-name 종료는 사용자의 열린 한글 문서까지 손상시킬 수 있으므로
    의도적으로 하지 않는다. timeout 뒤 COM 서버가 남는 환경에서는 격리된
    검증 호스트를 정리한 뒤 다시 실행해야 한다.
    """
    try:
        p = subprocess.run([sys.executable, __file__, "--child", src, dst],
                           capture_output=True, text=True, encoding="utf-8",
                           errors="replace", timeout=timeout)
    except subprocess.TimeoutExpired:
        return None
    return p.stdout


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--child", nargs=2)
    ap.add_argument("--exe")
    ap.add_argument("--list")
    ap.add_argument("--limit", type=int, default=30)
    ap.add_argument("--out")
    ap.add_argument("--child-timeout", type=int, default=420)
    a = ap.parse_args()
    if a.child:
        child(*a.child)
        return

    docs = [l.strip() for l in io.open(a.list, encoding="utf-8") if l.strip()][:a.limit]
    tmp = os.path.join(os.environ.get("CLAUDE_JOB_DIR", "."), "tmp", "hwp3rt")
    os.makedirs(tmp, exist_ok=True)
    rows = []
    for i, src in enumerate(docs, 1):
        name = os.path.basename(src)
        dst = os.path.join(tmp, f"rt_{i:03d}.hwp")
        for f in (dst,):
            if os.path.exists(f):
                os.remove(f)
        cv = subprocess.run([a.exe, "convert", src, dst],
                            capture_output=True, text=True, encoding="utf-8",
                            errors="replace", timeout=1800)
        if not os.path.exists(dst):
            rows.append({"sample": name, "convert": 0, "open": "", "pages": "",
                         "src_pages": ""})
            print(f"[{i}/{len(docs)}] {name[:44]:44} 변환실패", flush=True)
            continue
        # [함정] 한글이 특정 문서에서 응답 없이 멈춘다(모달 대기 추정). 문서 하나의
        # hang 이 전체 실행을 죽이지 않도록 개별 자식만 timeout 처리한다. 기존
        # 한글 프로세스에는 접속하거나 종료하지 않는다.
        out = run_child(src, dst, a.child_timeout)
        if out is None:
            print(f"[{i}/{len(docs)}] {name[:44]:44} HANG(무응답)", flush=True)
            rows.append({"sample": name, "convert": 1, "open": "hang",
                         "pages": "", "src_pages": ""})
            continue
        line = next((l for l in out.splitlines() if l.startswith("RESULT")), None)
        if line:
            _, ok, pages, src_pages = line.split("\t")
        else:
            ok, pages, src_pages = "?", "", ""
        rows.append({"sample": name, "convert": 1, "open": ok,
                     "pages": pages, "src_pages": src_pages})
        mark = "OK" if ok == "1" else ("**못 엶**" if ok == "0" else "?")
        print(f"[{i}/{len(docs)}] {name[:44]:44} {mark}  왕복{pages} 원본{src_pages}",
              flush=True)
    if a.out and rows:
        with open(a.out, "w", encoding="utf-8", newline="") as fh:
            w = csv.DictWriter(fh, fieldnames=list(rows[0]), delimiter="\t")
            w.writeheader()
            w.writerows(rows)
        print(f"기록: {a.out}")
    bad = [r for r in rows if r["open"] == "0"]
    print(f"\n=== 변환 {sum(1 for r in rows if r['convert'])}건 중 한글이 못 여는 것 "
          f"{len(bad)}건 ===")
    for r in bad[:15]:
        print("  ", r["sample"][:60])


if __name__ == "__main__":
    main()
