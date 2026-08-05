#!/usr/bin/env python
"""문서별 PI→쪽 지도의 해시를 뽑는다 (COM 불필요).

두 바이너리의 해시를 비교하면 **PI 판정이 바뀔 수 있는 문서**만 골라낼 수 있다.
해시가 같으면 그 문서의 PI 오라클 결과는 이전 실행 그대로다.

사용: python pi_map_hash.py <chunks_dir> <out_tsv> --exe <rhwp.exe> [--jobs N]
"""
import argparse, hashlib, os, re, subprocess, time
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

PG = re.compile(r"=== 페이지 (\d+) \(global_idx=\d+, section=(\d+)")
PI = re.compile(r"\bpi=(\d+)")


def run_one(exe, path, timeout):
    try:
        p = subprocess.run([exe, "dump-pages", path], capture_output=True,
                           timeout=timeout, stdin=subprocess.DEVNULL)
    except (subprocess.TimeoutExpired, OSError):
        return None, None
    if p.returncode != 0:
        return None, None
    page = sec = None
    seen = {}
    pages = 0
    for line in p.stdout.decode("utf-8", "replace").splitlines():
        m = PG.search(line)
        if m:
            page = int(m.group(1)); sec = int(m.group(2))
            pages = max(pages, page)
            continue
        if page is None:
            continue
        for x in PI.findall(line):
            k = (sec, int(x))
            if k not in seen:
                seen[k] = page
    blob = ";".join(f"{s}.{p}={v}" for (s, p), v in sorted(seen.items()))
    return hashlib.sha1(blob.encode()).hexdigest()[:16], pages


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("chunks_dir", type=Path)
    ap.add_argument("out_tsv", type=Path)
    ap.add_argument("--exe", required=True)
    ap.add_argument("--jobs", type=int, default=12)
    ap.add_argument("--timeout", type=int, default=300)
    a = ap.parse_args()

    files, seen = [], set()
    for ch in sorted(a.chunks_dir.glob("chunk_*.txt")):
        for line in ch.read_text(encoding="utf-8").splitlines():
            f = line.strip()
            if f and f not in seen:
                seen.add(f); files.append(f)
    print(f"문서 {len(files)}건", flush=True)
    t0 = time.time()
    with open(a.out_tsv, "w", encoding="utf-8", newline="") as fh:
        fh.write("doc\tpi_hash\tpages\n")
        with ThreadPoolExecutor(max_workers=a.jobs) as ex:
            futs = [(ex.submit(run_one, a.exe, f, a.timeout), f) for f in files]
            for i, (fut, f) in enumerate(futs, 1):
                h, pages = fut.result()
                fh.write(f"{Path(f).name}\t{h or ''}\t{pages or ''}\n")
                if i % 2000 == 0:
                    fh.flush(); print(f"[{i}/{len(files)}] {(time.time()-t0)/60:.1f}m", flush=True)
    print(f"=== 완료 {(time.time()-t0)/60:.1f}m ===", flush=True)


if __name__ == "__main__":
    main()
