from __future__ import annotations

import os
import re
import subprocess
import textwrap
import unittest
from pathlib import Path


WORKFLOW_PATH = Path(__file__).resolve().parents[2] / ".github/workflows/ci.yml"
CLASSIFIER_PATH = Path(__file__).resolve().parents[1] / "ci-impact-classifier.cjs"
TESTS_DIR = Path(__file__).resolve().parents[2] / "tests"
WORKER_MARKER = "  # [#2393] 기본 테스트 병렬화"

# [#4040] 파일 전체가 native-skia 로 게이트된 integration test.
#
# default-feature worker 는 이 파일을 통째로 cfg-out 하므로, Native Skia job 이
# 명시적으로 실행하지 않으면 **어디에서도 돌지 않는다.**
#
# 판별은 양쪽 방향의 오탐을 모두 막아야 한다. 한쪽으로 좁으면 부류를 놓치고,
# 반대쪽으로 넓으면 배선할 이유가 없는 파일을 배선하라고 요구한다.
#
# - 게이트는 중첩된다 — `render_p37_direct_pdf_export.rs` 는
#   `#![cfg(all(not(target_arch = "wasm32"), feature = "native-skia"))]` 형태라
#   정확 일치로 좁히면 놓친다. 그래서 괄호 균형으로 술어를 잘라 본다.
# - `not(feature = "native-skia")` 는 **정반대 조건**이다. 그 파일은 native-skia
#   빌드에서 오히려 cfg-out 되므로 job 에 배선하면 0건짜리 target 이 된다.
#   부정 문맥 안의 언급은 세지 않는다.
# - 이 저장소는 한국어 `//!` 문서에 cfg 속성을 자주 인용한다. 인용은 게이트가
#   아니므로 줄 주석을 먼저 지운다.
_INNER_CFG_OPEN = re.compile(r"#!\[\s*cfg\s*\(")
_FEATURE_NATIVE_SKIA = re.compile(r'feature\s*=\s*"native-skia"')
_CALL_NAME_BEFORE_PAREN = re.compile(r"(\w+)\s*$")


def _strip_line_comments(source: str) -> str:
    """`//`·`///`·`//!` 로 시작하는 줄을 지운다."""
    return "\n".join(
        "" if line.lstrip().startswith("//") else line
        for line in source.splitlines()
    )


def _inner_cfg_predicates(source: str) -> list[str]:
    """inner attribute `#![cfg(...)]` 의 술어를 괄호 균형으로 잘라낸다."""
    predicates = []
    for opened in _INNER_CFG_OPEN.finditer(source):
        depth = 1
        index = opened.end()
        while index < len(source) and depth > 0:
            if source[index] == "(":
                depth += 1
            elif source[index] == ")":
                depth -= 1
            index += 1
        if depth == 0:
            predicates.append(source[opened.end():index - 1])
    return predicates


def _requires_native_skia_enabled(predicate: str) -> bool:
    """술어가 native-skia 를 **켠** 상태로 요구하는가. `not(...)` 안이면 아니다."""
    enclosing: list[str] = []
    index = 0
    while index < len(predicate):
        found = _FEATURE_NATIVE_SKIA.match(predicate, index)
        if found:
            if "not" not in enclosing:
                return True
            index = found.end()
            continue
        char = predicate[index]
        if char == "(":
            name = _CALL_NAME_BEFORE_PAREN.search(predicate[:index])
            enclosing.append(name.group(1) if name else "")
        elif char == ")" and enclosing:
            enclosing.pop()
        index += 1
    return False


def source_is_file_gated_native_skia(source: str) -> bool:
    """소스 텍스트가 파일 전체를 native-skia **활성** 조건으로 게이트하는가."""
    return any(
        _requires_native_skia_enabled(predicate)
        for predicate in _inner_cfg_predicates(_strip_line_comments(source))
    )


def file_gated_native_skia_tests() -> list[str]:
    """`tests/*.rs` 중 파일 게이트된 native-skia test 의 stem 목록."""
    return sorted(
        path.stem
        for path in TESTS_DIR.glob("*.rs")
        if source_is_file_gated_native_skia(path.read_text(encoding="utf-8"))
    )


class CiImpactWorkflowTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.workflow = WORKFLOW_PATH.read_text(encoding="utf-8")
        cls.preflight, cls.workers = cls.workflow.split(WORKER_MARKER, maxsplit=1)

    def _step(self, name: str, source: str | None = None) -> str:
        workflow = source or self.workflow
        step = workflow.split(f"      - name: {name}", maxsplit=1)[1]
        boundary = re.search(r"(?m)^(?:      - name:|  [A-Za-z0-9_-]+:)\s*", step)
        return step[: boundary.start()] if boundary else step

    def _job(self, name: str) -> str:
        match = re.search(
            rf"(?ms)^  {re.escape(name)}:\n.*?(?=^  [A-Za-z0-9_-]+:\n|\Z)",
            self.workflow,
        )
        self.assertIsNotNone(match, name)
        return match.group(0) if match else ""

    def _run_aggregate(self, **overrides: str) -> subprocess.CompletedProcess[str]:
        step = self._step("Check Build & Test worker results")
        script = textwrap.dedent(step.split("        run: |\n", maxsplit=1)[1])
        env = {
            **os.environ,
            "PREFLIGHT_RESULT": "success",
            "FAST_PASS": "false",
            "RUST_REQUIRED": "false",
            "NATIVE_SKIA_REQUIRED": "false",
            "FRONTEND_MODE": "unit",
            "IMPACT_REASON": "classified:studio-unit",
            "BUILD_SLOW_RESULT": "skipped",
            "BUILD_A_RESULT": "skipped",
            "BUILD_B_RESULT": "skipped",
            "TEST_SLOW_RESULT": "skipped",
            "TEST_REGULAR_1_RESULT": "skipped",
            "TEST_REGULAR_2_RESULT": "skipped",
            "TEST_REGULAR_3_RESULT": "skipped",
            "LINT_RESULT": "skipped",
            "NATIVE_SKIA_RESULT": "skipped",
            "FRONTEND_UNIT_RESULT": "success",
            "FRONTEND_PACKAGE_RESULT": "skipped",
            **overrides,
        }
        return subprocess.run(
            ["bash", "-e", "-o", "pipefail", "-c", script],
            check=False,
            capture_output=True,
            env=env,
            text=True,
        )

    def test_preflight_exposes_every_axis_with_fail_closed_defaults(self) -> None:
        expected_defaults = {
            "rust_required": "'true'",
            "frontend_mode": "'package'",
            "render_required": "'true'",
            "native_skia_required": "'true'",
            "codeql_languages": "'javascript-typescript,python,rust'",
            "classification_status": "'full'",
            "classifier_version": "'unavailable'",
            "impact_reason": "'fail-closed:impact-unavailable'",
            "impact_authority": "'unavailable'",
        }
        for output, default in expected_defaults.items():
            with self.subTest(output=output):
                self.assertIn(f"      {output}:", self.preflight)
                self.assertIn(default, self.preflight)

    def test_classifier_uses_pr_base_sha_without_checkout_credentials(self) -> None:
        step = self._step("Check out trusted CI impact classifier", self.preflight)
        self.assertIn(
            "ref: ${{ github.event_name == 'pull_request' "
            "&& github.event.pull_request.base.sha || github.sha }}",
            step,
        )
        self.assertIn("persist-credentials: false", step)
        self.assertIn("sparse-checkout: scripts/ci-impact-classifier.cjs", step)
        self.assertIn("sparse-checkout-cone-mode: false", step)
        self.assertIn("id: checkout-impact-classifier", step)
        self.assertIn("Classify CI impact", self.preflight)
        self.assertIn(
            "Stage 4 activates frontend_mode, render_required, rust_required, "
            "and native_skia_required",
            self.preflight,
        )
        self.assertIn("pr-base-trusted", self.preflight)
        self.assertNotIn("pr-base-trusted-shadow", self.preflight)

    def test_missing_classifier_checkout_cannot_claim_trusted_authority(self) -> None:
        self.assertIn(
            "const classifierPath = path.join(\n"
            "              workspace,\n"
            "              'scripts',\n"
            "              'ci-impact-classifier.cjs',",
            self.preflight,
        )
        self.assertIn(
            "const checkoutSucceeded = "
            "process.env.CLASSIFIER_CHECKOUT_OUTCOME === 'success'\n"
            "              && fs.existsSync(classifierPath);",
            self.preflight,
        )
        self.assertIn(
            "const authority = !checkoutSucceeded\n"
            "              ? 'unavailable'",
            self.preflight,
        )

    def test_review_only_fast_pass_does_not_pay_classifier_cost(self) -> None:
        for step_name in (
            "Check out trusted CI impact classifier",
            "Collect CI impact input",
            "Classify CI impact",
        ):
            with self.subTest(step=step_name):
                self.assertIn(
                    "if: ${{ steps.finalize.outputs.fast_pass != 'true' }}",
                    self._step(step_name, self.preflight),
                )

    def test_label_events_do_not_restart_ci_and_manual_dispatch_forces_full(self) -> None:
        self.assertIn(
            "types: [opened, reopened, synchronize]",
            self.workflow,
        )
        self.assertNotIn("labeled, unlabeled", self.workflow)
        collect = self._step("Collect CI impact input", self.preflight)
        self.assertNotIn("label.name === 'ci:full'", collect)
        self.assertIn("context.eventName === 'workflow_dispatch'", collect)
        self.assertIn("? 'manual-or-tag'", collect)

    def test_stage4_consumes_frontend_rust_and_native_axes_but_defers_codeql(self) -> None:
        self.assertIn("needs.preflight.outputs.frontend_mode", self.workers)
        for active_axis in (
            "needs.preflight.outputs.rust_required",
            "needs.preflight.outputs.native_skia_required",
        ):
            with self.subTest(axis=active_axis):
                self.assertIn(active_axis, self.workers)
        self.assertNotIn("needs.preflight.outputs.codeql_languages", self.workers)

    def test_unit_and_package_jobs_are_mutually_exclusive(self) -> None:
        unit = self._job("frontend-unit-gates")
        package = self._job("frontend-package-gates")
        self.assertIn("needs.preflight.outputs.frontend_mode == 'unit'", unit)
        self.assertIn("npx tsc --project tsconfig.ci-unit.json --noEmit", unit)
        self.assertIn("npm --prefix rhwp-studio run test", unit)
        self.assertNotIn("wasm-pack build", unit)
        self.assertIn("needs.preflight.outputs.frontend_mode == 'package'", package)
        self.assertIn("wasm-pack build --target web --dev", package)
        self.assertIn("npm --prefix rhwp-studio run test", package)
        self.assertIn("npm --prefix rhwp-studio run build", package)

    def test_rust_lint_and_archive_builders_require_rust_axis(self) -> None:
        lint = self._job("lint")
        self.assertIn("needs.preflight.outputs.rust_required == 'true'", lint)

        for job_name in (
            "build-test-archive-slow",
            "build-test-archive-a",
            "build-test-archive-b",
        ):
            with self.subTest(job=job_name):
                job = self._job(job_name)
                self.assertIn("needs.preflight.outputs.rust_required == 'true'", job)
                self.assertIn("needs.lint.result == 'success'", job)
                self.assertIn("frontend-unit-gates", job)
                self.assertIn("frontend-package-gates", job)
                self.assertIn("frontend_mode == 'none'", job)
                self.assertIn("frontend_mode == 'unit'", job)
                self.assertIn("frontend_mode == 'package'", job)

    def test_native_skia_accepts_expected_lint_state_for_each_rust_lane(self) -> None:
        native = self._job("native-skia-tests")
        self.assertIn("needs.preflight.outputs.native_skia_required == 'true'", native)
        self.assertIn("needs.preflight.outputs.rust_required == 'true'", native)
        self.assertIn("needs.lint.result == 'success'", native)
        self.assertIn("needs.preflight.outputs.rust_required == 'false'", native)
        self.assertIn("needs.lint.result == 'skipped'", native)
        self.assertIn("frontend-unit-gates", native)
        self.assertIn("frontend-package-gates", native)
        self.assertIn("frontend_mode == 'none'", native)
        self.assertIn("frontend_mode == 'unit'", native)
        self.assertIn("frontend_mode == 'package'", native)
        self.assertNotIn("build-test-archive-", native)
        self.assertNotIn("test-regular-shard", native)
        self.assertNotIn("test-slow-shard", native)

    def test_aggregate_harness_stops_at_the_next_job_boundary(self) -> None:
        step = self._step("Check Build & Test worker results")
        script = textwrap.dedent(step.split("        run: |\n", maxsplit=1)[1])
        self.assertNotIn("wasm-build:", script)
        self.assertNotIn("startsWith(github.ref", script)

    def test_native_skia_integration_targets_are_classifier_inputs(self) -> None:
        # 역방향 감시: job 이 실행하는 target 은 classifier 소유여야 한다.
        native_step = self._step("Native Skia tests")
        classifier = CLASSIFIER_PATH.read_text(encoding="utf-8")
        targets = set(re.findall(r"--test ([A-Za-z0-9_]+)", native_step))
        self.assertTrue(targets)
        for target in targets:
            with self.subTest(target=target):
                self.assertIn(f"'tests/{target}.rs'", classifier)

    def test_discovery_finds_the_known_file_gated_native_skia_tests(self) -> None:
        """발견 패턴이 망가지면 아래 테스트가 조용히 무의미해진다.

        `render_p37_direct_pdf_export` 는 `all(...)` 중첩 게이트라, 정확 일치
        패턴으로는 잡히지 않는다. 이 단언이 그 회귀를 막는다.
        """
        found = file_gated_native_skia_tests()
        for expected in [
            "issue_2083_hide_fill_page_background",
            "issue_2292_chart_png_clip",
            "issue_2293_chart_png_text",
            "render_p37_direct_pdf_export",
        ]:
            self.assertIn(expected, found)
        # 함수 게이트 파일은 이 부류가 아니다 — 별도 축(#4132)이다.
        self.assertNotIn("issue_2225_missing_picture_placeholder", found)
        self.assertNotIn("cli_exit_codes", found)

    def test_discovery_rejects_negated_gates_and_quoted_attributes(self) -> None:
        """[PR #4170 리뷰] 발견 패턴의 **반대 방향** 오탐도 막는다.

        위 테스트는 "놓치지 않는가" 만 본다. 넓은 쪽 오탐은 저장소에 해당 파일이
        생기기 전까지 드러나지 않으므로 합성 입력으로 고정한다.

        - `not(feature = "native-skia")` 는 native-skia 빌드에서 오히려 cfg-out
          되므로, 배선을 요구하면 0건짜리 target 이 생긴다.
        - 이 저장소는 한국어 `//!` 문서에 cfg 속성을 자주 인용한다. 인용은
          게이트가 아니다.
        """
        for source in [
            '#![cfg(feature = "native-skia")]',
            '#![cfg(all(not(target_arch = "wasm32"), feature = "native-skia"))]',
            '#![cfg(all(\n    not(target_arch = "wasm32"),\n    feature = "native-skia"\n))]',
        ]:
            with self.subTest(gated=source):
                self.assertTrue(source_is_file_gated_native_skia(source))

        for source in [
            '#![cfg(not(feature = "native-skia"))]',
            '#![cfg(all(not(target_arch = "wasm32"), not(feature = "native-skia")))]',
            '//! `#![cfg(feature = "native-skia")]` 로 파일을 게이트한다',
            '// #![cfg(feature = "native-skia")]',
            '#![cfg(not(target_arch = "wasm32"))]',
        ]:
            with self.subTest(not_gated=source):
                self.assertFalse(source_is_file_gated_native_skia(source))

    def test_every_file_gated_native_skia_test_is_wired(self) -> None:
        """[#4040] 파일 게이트된 native-skia test 는 job·classifier 양쪽에 있어야 한다.

        기존 `test_native_skia_integration_targets_are_classifier_inputs` 는
        **job 이 실행하는 target** 만 순회하므로, 양쪽 어디에도 없는 파일은 대조
        대상 자체가 아니라 조용히 빠진다. `issue_2083`·`issue_2292`·`issue_2293`
        이 정확히 그 경로로 새어 나갔다 — 파일 전체가 cfg-out 되어 default worker
        에서도 돌지 않고, Native job 도 실행하지 않는 상태였다.

        저장소를 직접 훑어 부류 자체를 강제한다.
        """
        native_step = self._step("Native Skia tests")
        classifier = CLASSIFIER_PATH.read_text(encoding="utf-8")
        targets = set(re.findall(r"--test ([A-Za-z0-9_]+)", native_step))

        missing_from_job = []
        missing_from_classifier = []
        for stem in file_gated_native_skia_tests():
            if stem not in targets:
                missing_from_job.append(stem)
            if f"'tests/{stem}.rs'" not in classifier:
                missing_from_classifier.append(stem)

        self.assertEqual(
            missing_from_job,
            [],
            "Native Skia job 이 실행하지 않는 파일 게이트 test 가 있다. "
            "`--test <name>` 을 release-test·release 두 경로에 추가한다.",
        )
        self.assertEqual(
            missing_from_classifier,
            [],
            "classifier 의 NATIVE_SKIA_RUST_FILES 에 없는 파일 게이트 test 가 있다. "
            "빠지면 그 파일을 고치는 PR 에서 Native Skia job 이 skip 된다.",
        )

    def test_native_skia_targets_run_in_both_profiles(self) -> None:
        """[#4040] release-test 와 release 두 경로가 같은 target 집합을 실행한다."""
        native_step = self._step("Native Skia tests")
        release_test = set(
            re.findall(r"--profile release-test --features native-skia --test ([A-Za-z0-9_]+)", native_step)
        )
        release = set(
            re.findall(r"--release --features native-skia --test ([A-Za-z0-9_]+)", native_step)
        )
        self.assertTrue(release_test)
        self.assertEqual(release_test, release)

    def test_rust_workers_wait_only_for_their_test_archive(self) -> None:
        expected_archives = {
            "test-slow-shard": "build-test-archive-slow",
            "test-regular-shard-1": "build-test-archive-a",
            "test-regular-shard-2": "build-test-archive-slow",
            "test-regular-shard-3": "build-test-archive-b",
        }
        for job_name, archive in expected_archives.items():
            with self.subTest(job=job_name):
                job = self._job(job_name)
                self.assertIn("needs.preflight.outputs.rust_required == 'true'", job)
                self.assertIn(f"needs: [preflight, {archive}]", job)
                self.assertIn(f"needs['{archive}'].result == 'success'", job)
                self.assertNotIn("native-skia-tests", job)
                self.assertNotIn("native_skia_required", job)

    def test_aggregate_validates_expected_success_and_skipped_states(self) -> None:
        aggregate = self._job("build-and-test")
        self.assertIn("- frontend-unit-gates", aggregate)
        self.assertIn("- frontend-package-gates", aggregate)
        self.assertIn("- native-skia-tests", aggregate)
        self.assertIn("RUST_REQUIRED:", aggregate)
        self.assertIn("NATIVE_SKIA_REQUIRED:", aggregate)
        self.assertIn("Rust lane expected success", aggregate)
        self.assertIn("Rust lane expected skipped", aggregate)
        self.assertIn("Native Skia lane expected success", aggregate)
        self.assertIn("Native Skia lane expected skipped", aggregate)
        self.assertIn("Unknown rust_required", aggregate)
        self.assertIn("Unknown native_skia_required", aggregate)
        self.assertIn("Frontend none lane expected skipped/skipped", aggregate)
        self.assertIn("Frontend unit lane expected success/skipped", aggregate)
        self.assertIn("Frontend package lane expected skipped/success", aggregate)
        self.assertIn("Unknown frontend mode", aggregate)

    def test_shard_count_artifacts_are_downloaded_only_for_rust_lane(self) -> None:
        aggregate = self._job("build-and-test")
        for step_name in (
            "Download shard counts",
            "Download archive expected counts",
            "Verify shard totals",
        ):
            with self.subTest(step=step_name):
                self.assertIn(
                    "needs.preflight.outputs.rust_required == 'true'",
                    self._step(step_name, aggregate),
                )

    def test_aggregate_accepts_every_supported_stage4_lane(self) -> None:
        rust_success = {
            "RUST_REQUIRED": "true",
            "LINT_RESULT": "success",
            "BUILD_SLOW_RESULT": "success",
            "BUILD_A_RESULT": "success",
            "BUILD_B_RESULT": "success",
            "TEST_SLOW_RESULT": "success",
            "TEST_REGULAR_1_RESULT": "success",
            "TEST_REGULAR_2_RESULT": "success",
            "TEST_REGULAR_3_RESULT": "success",
        }
        cases = {
            "frontend-only": {},
            "rust-non-render": {
                **rust_success,
                "FRONTEND_MODE": "none",
                "FRONTEND_UNIT_RESULT": "skipped",
            },
            "rust-render": {
                **rust_success,
                "NATIVE_SKIA_REQUIRED": "true",
                "NATIVE_SKIA_RESULT": "success",
                "FRONTEND_MODE": "none",
                "FRONTEND_UNIT_RESULT": "skipped",
            },
            "non-rust-native-input": {
                "NATIVE_SKIA_REQUIRED": "true",
                "NATIVE_SKIA_RESULT": "success",
                "FRONTEND_MODE": "package",
                "FRONTEND_UNIT_RESULT": "skipped",
                "FRONTEND_PACKAGE_RESULT": "success",
            },
        }
        for name, env in cases.items():
            with self.subTest(lane=name):
                result = self._run_aggregate(**env)
                self.assertEqual(result.returncode, 0, result.stdout + result.stderr)

    def test_aggregate_rejects_axis_result_mismatches(self) -> None:
        cases = {
            "unexpected-rust-worker": {"LINT_RESULT": "success"},
            "missing-native-worker": {
                "NATIVE_SKIA_REQUIRED": "true",
                "NATIVE_SKIA_RESULT": "skipped",
            },
            "unexpected-native-worker": {"NATIVE_SKIA_RESULT": "success"},
            "frontend-mismatch": {"FRONTEND_UNIT_RESULT": "skipped"},
            "unknown-rust-axis": {"RUST_REQUIRED": "maybe"},
            "unknown-native-axis": {"NATIVE_SKIA_REQUIRED": "maybe"},
        }
        for name, env in cases.items():
            with self.subTest(lane=name):
                result = self._run_aggregate(**env)
                self.assertNotEqual(result.returncode, 0, result.stdout + result.stderr)

    def test_aggregate_fast_pass_still_accepts_skipped_heavy_jobs(self) -> None:
        result = self._run_aggregate(
            FAST_PASS="true",
            RUST_REQUIRED="true",
            NATIVE_SKIA_REQUIRED="true",
            FRONTEND_MODE="package",
            FRONTEND_UNIT_RESULT="skipped",
        )
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)

    def test_classifier_failures_remain_fail_closed_without_failing_preflight(self) -> None:
        for step_name in (
            "Check out trusted CI impact classifier",
            "Collect CI impact input",
            "Classify CI impact",
            "Summarize CI impact classification",
        ):
            with self.subTest(step=step_name):
                self.assertIn("continue-on-error: true", self._step(step_name, self.preflight))


if __name__ == "__main__":
    unittest.main()
