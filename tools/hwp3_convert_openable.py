#!/usr/bin/env python
"""HWP3 → HWP5 변환본을 **한글이 열 수 있는가**.

rhwp 는 자기가 쓴 파일을 읽을 수 있으므로 왕복 정합을 rhwp 리더로만 재면
"완벽" 으로 보인다. 실제 판정은 한컴이 여는지다.

한글 COM 은 open 실패 뒤 인스턴스가 죽어 다음 측정을 오염시킨다 —
**문서 1건당 프로세스 1개**로 격리하고, 사이에 프로세스를 정리한다.

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
import time

sys.stdout.reconfigure(encoding="utf-8", errors="replace")


def child(src, dst):
    """한 문서만 열어 결과를 한 줄로 찍는다. 실패해도 부모는 살아남는다."""
    from pyhwpx import Hwp
    hwp = Hwp(visible=False)
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


def kill_hangul():
    for image in ("Hwp.exe", "HwpApp.exe"):
        subprocess.run(["taskkill", "/F", "/IM", image],
                       capture_output=True, shell=False)


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
        kill_hangul()
        time.sleep(9)
        # [함정] 한글이 특정 문서에서 응답 없이 멈춘다(모달 대기 추정). 문서 하나의
        # hang 이 전체 실행을 죽이지 않도록 개별로 잡고 프로세스를 정리한다.
        try:
            p = subprocess.run([sys.executable, __file__, "--child", src, dst],
                               capture_output=True, text=True, encoding="utf-8",
                               errors="replace", timeout=a.child_timeout)
            out = p.stdout
        except subprocess.TimeoutExpired:
            kill_hangul()
            time.sleep(5)
            out = ""
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
