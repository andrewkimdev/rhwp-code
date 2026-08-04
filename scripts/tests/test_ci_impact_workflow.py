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

    def test_shadow_classifier_uses_pr_base_sha_without_checkout_credentials(self) -> None:
        step_name = "Check out trusted CI impact classifier"
        self.assertIn(step_name, self.preflight)
        step = self.preflight.split(f"      - name: {step_name}", maxsplit=1)[1]
        step = step.split("\n      - name:", maxsplit=1)[0]
        self.assertIn(
            "ref: ${{ github.event_name == 'pull_request' "
            "&& github.event.pull_request.base.sha || github.sha }}",
            step,
        )
        self.assertIn("persist-credentials: false", step)
        self.assertIn("sparse-checkout: scripts/ci-impact-classifier.cjs", step)
        self.assertIn("sparse-checkout-cone-mode: false", step)
        self.assertIn("id: checkout-impact-classifier", step)
        self.assertIn("Classify CI impact in shadow mode", self.preflight)
        self.assertIn("Advisory only: existing worker conditions are unchanged", self.preflight)
        self.assertIn("Pull requests classify with the base SHA classifier", self.preflight)
        self.assertIn(
            "CLASSIFIER_CHECKOUT_OUTCOME: "
            "${{ steps.checkout-impact-classifier.outcome }}",
            self.preflight,
        )
        self.assertIn("pr-base-trusted-shadow", self.preflight)
        self.assertNotIn("pr-merge-advisory", self.preflight)

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
            "              ? 'unavailable-advisory'",
            self.preflight,
        )

    def test_review_only_fast_pass_does_not_pay_shadow_checkout_cost(self) -> None:
        for step_name in (
            "Check out trusted CI impact classifier",
            "Collect shadow CI impact input",
            "Classify CI impact in shadow mode",
        ):
            with self.subTest(step=step_name):
                step = self.preflight.split(f"      - name: {step_name}", maxsplit=1)[1]
                step = step.split("\n      - name:", maxsplit=1)[0]
                self.assertIn(
                    "if: ${{ steps.detect.outputs.fast_pass != 'true' }}",
                    step,
                )

    def test_existing_worker_conditions_do_not_consume_shadow_outputs(self) -> None:
        self.assertNotIn("shadow_", self.workers)
        for reference in (
            "needs.preflight.outputs.shadow_",
            "needs.preflight.outputs['shadow_",
            'needs.preflight.outputs["shadow_',
        ):
            with self.subTest(reference=reference):
                self.assertNotIn(reference, self.workflow)
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
            "Check out trusted CI impact classifier",
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
