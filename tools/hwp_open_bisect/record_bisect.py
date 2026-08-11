"""HWP5 레코드 대조·이식 — "저장했는데 한컴이 못 여는" 결함을 바이트까지 좁힌다.

한컴이 같은 원본을 저장한 파일을 **정답지**로 두고, rhwp 산출물의 일부만 정답지에
이식해 개방 여부를 본다. 열리면 그 부분은 무죄, 못 열면 그 안에 원인이 있다.
코드를 고치지 않고 결함 위치를 짚을 수 있다.

## 쓰임

    O=oracle.hwp; C=rhwp_out.hwp

    # 1) 구조부터 본다 — 레코드 수·태그·레벨·크기가 어디서 갈리는가
    python tools/hwp_open_bisect/record_bisect.py diff $O $C

    # 2) 스트림 통째로: 후보의 Section0 만 정답지에 넣는다
    python tools/hwp_open_bisect/record_bisect.py hybrid $O $C out.hwp --stream BodyText/Section0

    # 3) 레코드 종류로: 후보의 SHAPE_COMPONENT(76) 만 넣는다 (--invert 로 여집합)
    python tools/hwp_open_bisect/record_bisect.py hybrid $O $C out.hwp --section 0 --tag 76

    # 4) 레코드 번호 범위로 이분한다
    python tools/hwp_open_bisect/record_bisect.py hybrid $O $C out.hwp --section 0 --records 0-268

    # 5) 특정 태그의 특정 바이트 구간만 정답지 값으로 되돌린다
    python tools/hwp_open_bisect/record_bisect.py hybrid $O $C out.hwp \
        --section 0 --tag 76 --restore-bytes 190-220

    # 각 산출물을 hangul_com.py check-open 으로 판정한다

## 함정 (겪은 것들)

- **ID 공간을 맞출 것.** 본문 레코드만 옮기고 DocInfo 는 정답지 것을 두면, 글자모양·
  테두리 ID 가 어긋나 내용과 무관하게 깨진다. `--with-docinfo`(기본값)가 후보의
  DocInfo 를 함께 넣어 이 교란을 없앤다.
- **부분 이식은 새 모순을 만든다.** 예컨대 `PARA_HEADER` 만 후보 것으로 바꾸면 "영역
  태그 0개" 선언과 정답지의 `PARA_RANGE_TAG` 레코드가 공존해 실패한다. 이건 원래
  결함이 아니라 실험이 만든 결함이다. **매 라운드 대조군**(`--tag` 없이 전량/무이식)을
  함께 돌려 걸러야 한다.
- **레코드 수가 다르면** 번호 대응이 깨진다. 이 도구는 `(태그, 레벨, 크기)` 로 정렬해
  일치 구간에서만 이식하므로 수가 달라도 쓸 수 있다.
- 스트림 슬롯 크기를 넘으면 쓸 수 없다. 보통 rhwp 쪽이 작아 문제되지 않는다.
"""

import argparse
import shutil
import struct
import sys
import zlib
from difflib import SequenceMatcher

import olefile

TAG_NAMES = {
    66: "PARA_HEADER",
    67: "PARA_TEXT",
    68: "PARA_CHAR_SHAPE",
    69: "PARA_LINE_SEG",
    70: "PARA_RANGE_TAG",
    71: "CTRL_HEADER",
    72: "LIST_HEADER",
    76: "SHAPE_COMPONENT",
    77: "TABLE",
    78: "SC_LINE",
    79: "SC_RECTANGLE",
    85: "SC_PICTURE",
    86: "SC_CONTAINER",
    87: "CTRL_DATA",
}


def read_stream(path, name):
    ole = olefile.OleFileIO(path)
    try:
        return ole.openstream(name).read()
    finally:
        ole.close()


def stream_names(path, prefix="BodyText/Section"):
    ole = olefile.OleFileIO(path)
    try:
        names = ["/".join(p) for p in ole.listdir()]
    finally:
        ole.close()
    return sorted(
        (n for n in names if n.startswith(prefix)),
        key=lambda n: int(n.rsplit("Section", 1)[1]) if "Section" in n else 0,
    )


def inflate(raw):
    try:
        return zlib.decompress(raw, -15)
    except zlib.error:
        return raw


def parse_records(buf):
    """(태그, 레벨, 데이터) 목록. HWP5 레코드 헤더는 32비트(태그10/레벨10/크기12)."""
    out, pos, end = [], 0, len(buf)
    while pos + 4 <= end:
        header = struct.unpack_from("<I", buf, pos)[0]
        tag, level, size = header & 0x3FF, (header >> 10) & 0x3FF, (header >> 20) & 0xFFF
        pos += 4
        if size == 0xFFF:  # 확장 크기
            if pos + 4 > end:
                break
            size = struct.unpack_from("<I", buf, pos)[0]
            pos += 4
        if pos + size > end:
            break
        out.append((tag, level, buf[pos : pos + size]))
        pos += size
    return out


def emit_records(records):
    out = bytearray()
    for tag, level, data in records:
        n = len(data)
        if n < 0xFFF:
            out += struct.pack("<I", (tag & 0x3FF) | ((level & 0x3FF) << 10) | (n << 20))
        else:
            out += struct.pack("<I", (tag & 0x3FF) | ((level & 0x3FF) << 10) | (0xFFF << 20))
            out += struct.pack("<I", n)
        out += data
    return bytes(out)


def section_records(path, section):
    name = f"BodyText/Section{section}"
    return parse_records(inflate(read_stream(path, name))), name


def cmd_diff(args):
    """두 파일의 레코드 구조·내용 차이를 요약한다."""
    a_names = stream_names(args.oracle)
    b_names = stream_names(args.candidate)
    print(f"구역: 정답지 {len(a_names)} / 후보 {len(b_names)}")
    for name in a_names:
        if name not in b_names:
            print(f"  {name}: 후보에 없음")
            continue
        a = parse_records(inflate(read_stream(args.oracle, name)))
        b = parse_records(inflate(read_stream(args.candidate, name)))
        ka = [(t, lv, len(d)) for t, lv, d in a]
        kb = [(t, lv, len(d)) for t, lv, d in b]
        unmatched, content = {}, {}
        for op, i1, i2, j1, j2 in SequenceMatcher(a=ka, b=kb, autojunk=False).get_opcodes():
            if op != "equal":
                for i in range(i1, i2):
                    unmatched.setdefault(("정답지만", a[i][0]), 0)
                    unmatched[("정답지만", a[i][0])] += 1
                for j in range(j1, j2):
                    unmatched.setdefault(("후보만", b[j][0]), 0)
                    unmatched[("후보만", b[j][0])] += 1
                continue
            for k in range(i2 - i1):
                i, j = i1 + k, j1 + k
                if a[i][2] != b[j][2]:
                    content[a[i][0]] = content.get(a[i][0], 0) + 1
        head = f"  {name}: 레코드 {len(a)} vs {len(b)}"
        if not unmatched and not content:
            print(head + "  — 동일")
            continue
        print(head)
        for (side, tag), n in sorted(unmatched.items(), key=lambda x: -x[1]):
            print(f"      정렬 불일치 {side} {TAG_NAMES.get(tag, tag)} {n}건")
        for tag, n in sorted(content.items(), key=lambda x: -x[1]):
            print(f"      내용 차이 {TAG_NAMES.get(tag, tag)} {n}건")
    return 0


def _parse_range(spec):
    lo, hi = spec.split("-")
    return int(lo), int(hi)


def cmd_hybrid(args):
    """정답지를 바탕으로 후보의 일부를 이식한 파일을 만든다."""
    shutil.copy(args.oracle, args.out)
    ole = olefile.OleFileIO(args.out, write_mode=True)
    try:
        if args.with_docinfo:
            _write_padded(ole, "DocInfo", read_stream(args.candidate, "DocInfo"))

        if args.stream:
            _write_padded(ole, args.stream, read_stream(args.candidate, args.stream))
            print(f"이식: {args.stream} 통째")
            return 0

        a, name = section_records(args.oracle, args.section)
        b, _ = section_records(args.candidate, args.section)
        mixed, swapped = list(a), 0
        rec_lo, rec_hi = _parse_range(args.records) if args.records else (0, len(a))
        restore = _parse_range(args.restore_bytes) if args.restore_bytes else None

        for op, i1, i2, j1, j2 in SequenceMatcher(
            a=[(t, lv, len(d)) for t, lv, d in a],
            b=[(t, lv, len(d)) for t, lv, d in b],
            autojunk=False,
        ).get_opcodes():
            if op != "equal":
                continue
            for k in range(i2 - i1):
                i, j = i1 + k, j1 + k
                if not rec_lo <= i < rec_hi:
                    continue
                if args.tag is not None:
                    match = a[i][0] == args.tag
                    take = (not match) if args.invert else match
                    if not take:
                        continue
                data = b[j][2]
                if restore and len(data) == len(a[i][2]):
                    buf = bytearray(data)
                    lo, hi = restore
                    buf[lo:hi] = a[i][2][lo:hi]
                    data = bytes(buf)
                mixed[i] = (b[j][0], b[j][1], data)
                swapped += 1

        _write_padded(ole, name, zlib.compress(emit_records(mixed), 9)[2:-4])
        print(f"이식: {name} 레코드 {swapped}건")
        return 0
    finally:
        ole.close()


def _write_padded(ole, name, data):
    size = ole.get_size(name)
    if len(data) > size:
        raise SystemExit(f"슬롯 초과: {name} 은 {size}B 인데 {len(data)}B 를 쓰려 한다")
    # deflate 스트림 뒤의 잉여 바이트는 읽는 쪽이 무시하므로 0 으로 채워 크기를 맞춘다.
    ole.write_stream(name, data + b"\x00" * (size - len(data)))


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    sub = parser.add_subparsers(dest="cmd", required=True)

    p_diff = sub.add_parser("diff", help="레코드 구조·내용 차이 요약")
    p_diff.add_argument("oracle")
    p_diff.add_argument("candidate")
    p_diff.set_defaults(func=cmd_diff)

    p_hy = sub.add_parser("hybrid", help="정답지에 후보 일부를 이식한다")
    p_hy.add_argument("oracle")
    p_hy.add_argument("candidate")
    p_hy.add_argument("out")
    p_hy.add_argument("--stream", help="스트림 통째 이식 (예: BodyText/Section0)")
    p_hy.add_argument("--section", type=int, default=0, help="레코드 단위 이식 대상 구역")
    p_hy.add_argument("--tag", type=int, help="이 태그의 레코드만 이식")
    p_hy.add_argument("--invert", action="store_true", help="--tag 의 여집합을 이식")
    p_hy.add_argument("--records", help="레코드 번호 범위 (예: 0-268)")
    p_hy.add_argument(
        "--restore-bytes",
        help="이식하되 이 바이트 구간만 정답지 값으로 되돌린다 (예: 190-220)",
    )
    p_hy.add_argument(
        "--no-docinfo",
        dest="with_docinfo",
        action="store_false",
        help="후보 DocInfo 를 함께 넣지 않는다 (ID 공간이 어긋날 수 있다)",
    )
    p_hy.set_defaults(func=cmd_hybrid, with_docinfo=True)

    args = parser.parse_args(argv)
    return args.func(args)


if __name__ == "__main__":
    sys.exit(main())
