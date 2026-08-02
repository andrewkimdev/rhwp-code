from __future__ import annotations

import unittest
from pathlib import Path


WORKFLOW_PATH = Path(__file__).resolve().parents[2] / ".github/workflows/ci.yml"
WORKER_MARKER = "  # [#2393] 기본 테스트 병렬화"


class CiImpactShadowWorkflowTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.workflow = WORKFLOW_PATH.read_text(encoding="utf-8")
        cls.preflight, cls.workers = cls.workflow.split(WORKER_MARKER, maxsplit=1)

    def test_preflight_exposes_every_shadow_axis_with_fail_closed_defaults(self) -> None:
        expected_defaults = {
            "shadow_rust_required": "'true'",
            "shadow_frontend_mode": "'package'",
            "shadow_render_required": "'true'",
            "shadow_native_skia_required": "'true'",
            "shadow_codeql_languages": "'javascript-typescript,python,rust'",
            "shadow_classification_status": "'full'",
            "shadow_classifier_version": "'unavailable'",
            "shadow_reason": "'fail-closed:shadow-unavailable'",
            "shadow_authority": "'unavailable-advisory'",
        }
        for output, default in expected_defaults.items():
            with self.subTest(output=output):
                self.assertIn(f"      {output}:", self.preflight)
                self.assertIn(default, self.preflight)

    def test_shadow_classifier_is_advisory_and_does_not_receive_checkout_credentials(self) -> None:
        self.assertIn("Check out repository for advisory impact classifier", self.preflight)
        self.assertIn("persist-credentials: false", self.preflight)
        self.assertIn("Classify CI impact in shadow mode", self.preflight)
        self.assertIn("Advisory only: existing worker conditions are unchanged", self.preflight)
        self.assertIn("pr-head-advisory", self.preflight)

    def test_existing_worker_conditions_do_not_consume_shadow_outputs(self) -> None:
        self.assertNotIn("shadow_", self.workers)
        self.assertIn(
            "needs.preflight.outputs.frontend_required == 'true'",
            self.workers,
        )
        self.assertIn(
            "needs.preflight.outputs.frontend_required != 'true'",
            self.workers,
        )
        self.assertIn("needs.preflight.outputs.fast_pass != 'true'", self.workers)

    def test_shadow_failures_cannot_fail_preflight(self) -> None:
        for step_name in (
            "Check out repository for advisory impact classifier",
            "Collect shadow CI impact input",
            "Classify CI impact in shadow mode",
            "Summarize shadow CI impact classification",
        ):
            with self.subTest(step=step_name):
                step = self.preflight.split(f"      - name: {step_name}", maxsplit=1)[1]
                step = step.split("\n      - name:", maxsplit=1)[0]
                self.assertIn("continue-on-error: true", step)


if __name__ == "__main__":
    unittest.main()
