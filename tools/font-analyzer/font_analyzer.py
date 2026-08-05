#!/usr/bin/env python3
"""font-analyzer — rhwp `info --json` 계약을 재사용하는 HWP/HWPX 글꼴 분석 도구.

HWP(CFB)/HWPX(OWPML ZIP) 컨테이너를 직접 파싱하지 않는다. 글꼴 목록의 유일한
신뢰 소스는 rhwp CLI의 `info --json` 출력이며(`fonts[]`, schemaVersion 1.0),
이 도구는 그 계약 위에서 단일 파일 조회와 디렉터리 일괄 집계만 담당한다.

사용 예:
    python tools/font-analyzer/font_analyzer.py samples/field-01.hwp
    python tools/font-analyzer/font_analyzer.py samples --format md
    RHWP_BIN=target/debug/rhwp python tools/font-analyzer/font_analyzer.py samples/field-01.hwp --format json
"""

from __future__ import annotations

import argparse
import json
import os
import shutil
import subprocess
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
SUPPORTED_SUFFIXES = (".hwp", ".hwpx")
DEFAULT_MAX_FILES = 10_000
MAX_FILES_LIMIT = 100_000
DEFAULT_RHWP_TIMEOUT_SECONDS = 120
MAX_RHWP_TIMEOUT_SECONDS = 1_800


class ToolError(RuntimeError):
    """사용자에게 그대로 보여줄 실행 오류."""


def bounded_positive_int(raw: str, argument: str, maximum: int) -> int:
    """CLI 자원 상한 인자를 양의 정수 범위로 제한한다."""
    try:
        value = int(raw)
    except ValueError as exc:
        raise argparse.ArgumentTypeError(f"{argument} 값은 정수여야 합니다: {raw}") from exc
    if not 1 <= value <= maximum:
        raise argparse.ArgumentTypeError(
            f"{argument} 값은 1 이상 {maximum} 이하여야 합니다: {raw}"
        )
    return value


def _reconfigure_utf8() -> None:
    """Windows 콘솔/파이프의 cp949 기본 인코딩으로 인한 한글 깨짐을 막는다."""
    for stream in (sys.stdout, sys.stderr):
        try:
            stream.reconfigure(encoding="utf-8", errors="replace")
        except (AttributeError, ValueError):
            pass


def resolve_rhwp_bin(cli_value: str | None) -> str:
    """rhwp 실행 파일 결정: --rhwp-bin > RHWP_BIN > PATH > 저장소 target/.

    명시적으로 지정한 경로(--rhwp-bin, RHWP_BIN)가 잘못됐으면 다음 후보로
    넘어가지 않고 즉시 오류를 낸다. 조용한 대체는 회귀를 숨기기 때문이다.
    """
    if cli_value:
        if not Path(cli_value).is_file():
            raise ToolError(f"--rhwp-bin 경로에 실행 파일이 없습니다: {cli_value}")
        return cli_value

    env_value = os.environ.get("RHWP_BIN")
    if env_value:
        if not Path(env_value).is_file():
            raise ToolError(f"RHWP_BIN 경로에 실행 파일이 없습니다: {env_value}")
        return env_value

    found = shutil.which("rhwp")
    if found:
        return found

    exe = "rhwp.exe" if os.name == "nt" else "rhwp"
    for profile in ("release-test", "release", "debug"):
        candidate = REPO / "target" / profile / exe
        if candidate.is_file():
            return str(candidate)

    raise ToolError(
        "rhwp 실행 파일을 찾지 못했습니다. --rhwp-bin 인자 또는 RHWP_BIN 환경변수로 "
        "경로를 지정하거나 rhwp를 PATH에 추가하세요 (빌드: cargo build --bin rhwp)."
    )


def rhwp_info(rhwp_bin: str, path: Path, timeout_seconds: int) -> dict:
    """`rhwp info --json <path>`를 실행해 JSON 객체를 반환한다."""
    try:
        proc = subprocess.run(
            [rhwp_bin, "info", "--json", str(path)],
            capture_output=True,
            text=True,
            encoding="utf-8",
            errors="replace",
            timeout=timeout_seconds,
        )
    except subprocess.TimeoutExpired as exc:
        raise ToolError(
            f"rhwp info가 {timeout_seconds}초 안에 끝나지 않았습니다: {path}"
        ) from exc
    except OSError as exc:
        raise ToolError(f"rhwp 실행 실패: {rhwp_bin}: {exc}") from exc

    if proc.returncode != 0:
        detail = proc.stderr.strip() or proc.stdout.strip() or "원인 미상"
        raise ToolError(f"rhwp info 실패 (exit {proc.returncode}): {path}: {detail}")

    try:
        data = json.loads(proc.stdout)
    except json.JSONDecodeError as exc:
        raise ToolError(f"rhwp info 출력이 JSON이 아닙니다: {path}: {exc}") from exc
    if not isinstance(data, dict):
        raise ToolError(f"rhwp info JSON 최상위가 객체가 아닙니다: {path}")
    return data


def fonts_from_info(info: dict, path: Path) -> list[str]:
    """info JSON에서 `fonts[]`를 검증해 순서를 유지한 채 중복만 제거한다."""
    fonts = info.get("fonts")
    if fonts is None:
        raise ToolError(
            f"rhwp info JSON에 fonts 필드가 없습니다 (스키마 변경 의심, "
            f"schemaVersion={info.get('schemaVersion')!r}): {path}"
        )
    if not isinstance(fonts, list) or not all(isinstance(f, str) for f in fonts):
        raise ToolError(f"rhwp info의 fonts 필드가 문자열 배열이 아닙니다: {path}")
    seen: dict[str, None] = {}
    for name in fonts:
        seen.setdefault(name)
    return list(seen)


def analyze_file(rhwp_bin: str, path: Path, timeout_seconds: int) -> dict:
    """단일 파일의 글꼴 분석 결과를 반환한다."""
    info = rhwp_info(rhwp_bin, path, timeout_seconds)
    fonts = fonts_from_info(info, path)
    return {
        "source": str(path),
        "format": info.get("format"),
        "fonts": fonts,
        "fontCount": len(fonts),
    }


def collect_targets(root: Path, recursive: bool, max_files: int) -> list[Path]:
    """디렉터리에서 지원 확장자(.hwp/.hwpx) 파일을 정렬해 모은다."""
    it = root.rglob("*") if recursive else root.glob("*")
    targets: list[Path] = []
    for path in it:
        # 심볼릭 링크는 분석 대상의 실제 위치를 사용자가 예측하기 어렵기 때문에 제외한다.
        if path.is_symlink() or not path.is_file():
            continue
        if path.suffix.lower() not in SUPPORTED_SUFFIXES:
            continue
        targets.append(path)
        if len(targets) > max_files:
            raise ToolError(
                f"분석 대상이 --max-files 상한({max_files}개)을 초과했습니다: {root}"
            )
    return sorted(targets)


def analyze_dir(
    rhwp_bin: str,
    root: Path,
    recursive: bool,
    targets: list[Path],
    timeout_seconds: int,
) -> dict:
    """디렉터리 일괄 분석: 파일별 결과 + 글꼴별 사용 파일 집계 + 실패 목록."""
    if not targets:
        raise ToolError(f"디렉터리에 .hwp/.hwpx 파일이 없습니다: {root}")

    files: list[dict] = []
    errors: list[dict] = []
    usage: dict[str, list[str]] = {}
    for target in targets:
        try:
            result = analyze_file(rhwp_bin, target, timeout_seconds)
        except ToolError as exc:
            errors.append({"source": str(target), "error": str(exc)})
            continue
        files.append(result)
        for font in result["fonts"]:
            usage.setdefault(font, []).append(result["source"])

    aggregated = [
        {"name": name, "fileCount": len(sources), "files": sources}
        for name, sources in usage.items()
    ]
    aggregated.sort(key=lambda item: (-item["fileCount"], item["name"]))

    return {
        "root": str(root),
        "recursive": recursive,
        "fileCount": len(targets),
        "okCount": len(files),
        "errorCount": len(errors),
        "uniqueFontCount": len(aggregated),
        "fonts": aggregated,
        "files": files,
        "errors": errors,
    }


def paths_alias(left: Path, right: Path) -> bool:
    """하드링크와 정규화된 경로를 포함해 두 경로가 같은 파일인지 확인한다."""
    try:
        return os.path.samefile(left, right)
    except OSError:
        return left.resolve() == right.resolve()


def ensure_safe_output(
    output_path: Path, source_paths: list[Path], overwrite: bool
) -> None:
    """분석 보고서가 원본을 덮어쓰지 않도록 출력 경로를 검사한다."""
    if output_path.is_symlink():
        raise ToolError(f"출력 경로는 심볼릭 링크일 수 없습니다: {output_path}")
    if any(paths_alias(output_path, source_path) for source_path in source_paths):
        raise ToolError(f"출력 경로가 분석 원본과 같습니다: {output_path}")
    if not output_path.exists():
        return
    if not output_path.is_file():
        raise ToolError(f"출력 경로가 일반 파일이 아닙니다: {output_path}")
    if not overwrite:
        raise ToolError(
            f"출력 파일이 이미 있습니다: {output_path} (덮어쓰려면 --overwrite 사용)"
        )


def format_text(result: dict) -> str:
    lines: list[str] = []
    if "root" in result:
        lines.append(f"디렉터리: {result['root']} (재귀: {result['recursive']})")
        lines.append(
            f"파일 {result['fileCount']}개 중 성공 {result['okCount']}개, "
            f"실패 {result['errorCount']}개, 고유 글꼴 {result['uniqueFontCount']}종"
        )
        lines.append("")
        lines.append("글꼴별 사용 파일 수:")
        for font in result["fonts"]:
            lines.append(f"  {font['fileCount']:4d}  {font['name']}")
        if result["errors"]:
            lines.append("")
            lines.append("실패한 파일:")
            for err in result["errors"]:
                lines.append(f"  {err['source']}: {err['error']}")
    else:
        lines.append(f"파일: {result['source']} (형식: {result['format']})")
        lines.append(f"글꼴 {result['fontCount']}종:")
        for font in result["fonts"]:
            lines.append(f"  - {font}")
    return "\n".join(lines)


def format_markdown(result: dict) -> str:
    lines: list[str] = []
    if "root" in result:
        lines.append(f"# 글꼴 집계: `{result['root']}`")
        lines.append("")
        lines.append(
            f"- 파일: {result['fileCount']}개 (성공 {result['okCount']}, "
            f"실패 {result['errorCount']})"
        )
        lines.append(f"- 고유 글꼴: {result['uniqueFontCount']}종")
        lines.append("")
        lines.append("| 글꼴 | 사용 파일 수 |")
        lines.append("| --- | ---: |")
        for font in result["fonts"]:
            lines.append(f"| {font['name']} | {font['fileCount']} |")
        if result["errors"]:
            lines.append("")
            lines.append("## 실패한 파일")
            lines.append("")
            lines.append("| 파일 | 오류 |")
            lines.append("| --- | --- |")
            for err in result["errors"]:
                message = err["error"].replace("|", "\\|")
                lines.append(f"| {err['source']} | {message} |")
    else:
        lines.append(f"# 글꼴 목록: `{result['source']}`")
        lines.append("")
        lines.append(f"- 형식: {result['format']}")
        lines.append(f"- 글꼴 수: {result['fontCount']}")
        lines.append("")
        lines.append("| 글꼴 |")
        lines.append("| --- |")
        for font in result["fonts"]:
            lines.append(f"| {font} |")
    return "\n".join(lines)


def render(result: dict, fmt: str) -> str:
    if fmt == "json":
        return json.dumps(result, ensure_ascii=False, indent=2)
    if fmt == "md":
        return format_markdown(result)
    return format_text(result)


def main(argv: list[str] | None = None) -> int:
    _reconfigure_utf8()
    parser = argparse.ArgumentParser(
        prog="font_analyzer",
        description="rhwp `info --json` 계약을 재사용하는 HWP/HWPX 글꼴 분석 도구",
    )
    parser.add_argument("input", help="분석할 .hwp/.hwpx 파일 또는 디렉터리")
    parser.add_argument(
        "--recursive",
        "-r",
        action="store_true",
        help="디렉터리 입력일 때 하위 디렉터리까지 재귀 탐색",
    )
    parser.add_argument(
        "--format",
        choices=("text", "json", "md"),
        default="text",
        help="출력 형식 (기본: text)",
    )
    parser.add_argument("--output", "-o", help="결과를 저장할 파일 (기본: 표준 출력)")
    parser.add_argument(
        "--rhwp-bin",
        help="rhwp 실행 파일 경로 (기본: RHWP_BIN 환경변수 → PATH → 저장소 target/)",
    )
    parser.add_argument(
        "--strict",
        action="store_true",
        help="디렉터리 분석에서 실패 파일이 하나라도 있으면 종료 코드 1",
    )
    parser.add_argument(
        "--overwrite",
        action="store_true",
        help="기존 출력 파일을 명시적으로 덮어쓴다",
    )
    parser.add_argument(
        "--max-files",
        type=lambda raw: bounded_positive_int(raw, "--max-files", MAX_FILES_LIMIT),
        default=DEFAULT_MAX_FILES,
        help=f"디렉터리 분석 최대 파일 수 (기본: {DEFAULT_MAX_FILES})",
    )
    parser.add_argument(
        "--timeout-seconds",
        type=lambda raw: bounded_positive_int(
            raw, "--timeout-seconds", MAX_RHWP_TIMEOUT_SECONDS
        ),
        default=DEFAULT_RHWP_TIMEOUT_SECONDS,
        help=(
            "파일별 rhwp info 제한 시간(초) "
            f"(기본: {DEFAULT_RHWP_TIMEOUT_SECONDS}, 최대: {MAX_RHWP_TIMEOUT_SECONDS})"
        ),
    )
    args = parser.parse_args(argv)

    try:
        rhwp_bin = resolve_rhwp_bin(args.rhwp_bin)
        target = Path(args.input)
        if target.is_symlink():
            raise ToolError(f"입력 경로는 심볼릭 링크일 수 없습니다: {target}")
        if target.is_dir():
            targets = collect_targets(target, args.recursive, args.max_files)
            source_paths = targets
            result = analyze_dir(
                rhwp_bin,
                target,
                args.recursive,
                targets,
                args.timeout_seconds,
            )
        elif target.is_file():
            source_paths = [target]
            result = analyze_file(rhwp_bin, target, args.timeout_seconds)
        else:
            raise ToolError(f"입력 경로가 존재하지 않습니다: {target}")

        if args.output:
            ensure_safe_output(Path(args.output), source_paths, args.overwrite)
    except ToolError as exc:
        print(f"오류: {exc}", file=sys.stderr)
        return 1

    rendered = render(result, args.format)
    if args.output:
        out_path = Path(args.output)
        out_path.parent.mkdir(parents=True, exist_ok=True)
        out_path.write_text(rendered + "\n", encoding="utf-8")
        print(f"저장됨: {out_path}", file=sys.stderr)
    else:
        print(rendered)

    if args.strict and result.get("errorCount", 0) > 0:
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
