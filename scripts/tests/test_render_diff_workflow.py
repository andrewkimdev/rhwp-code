from __future__ import annotations

import unittest
from pathlib import Path


WORKFLOW_PATH = Path(__file__).resolve().parents[2] / ".github/workflows/render-diff.yml"


class RenderDiffTriggerPolicyTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.workflow = WORKFLOW_PATH.read_text(encoding="utf-8")

    def test_review_records_do_not_trigger_canvas_visual_diff(self) -> None:
        pull_request_trigger = self.workflow.split("  workflow_dispatch:", maxsplit=1)[0]

        self.assertIn("  pull_request:\n", pull_request_trigger)
        self.assertIn("      - 'src/renderer/**'", pull_request_trigger)
        self.assertIn("      - 'rhwp-studio/**'", pull_request_trigger)
        self.assertIn("      - 'scripts/ci-impact-classifier.cjs'", pull_request_trigger)
        self.assertNotIn("'mydocs/**'", pull_request_trigger)

    def test_canvas_uses_the_base_classifier_render_axis(self) -> None:
        self.assertIn(
            "ref: ${{ github.event_name == 'pull_request' "
            "&& github.event.pull_request.base.sha || github.sha }}",
            self.workflow,
        )
        self.assertIn("persist-credentials: false", self.workflow)
        self.assertIn("sparse-checkout: scripts/ci-impact-classifier.cjs", self.workflow)
        self.assertIn(
            "render_required: ${{ steps.impact.outputs.render_required || 'true' }}",
            self.workflow,
        )
        self.assertIn(
            "needs.preflight.outputs.render_required == 'true'",
            self.workflow,
        )

    def test_label_events_do_not_restart_render_diff_and_manual_dispatch_is_full(self) -> None:
        self.assertIn(
            "types: [opened, reopened, synchronize]",
            self.workflow,
        )
        self.assertNotIn("labeled, unlabeled", self.workflow)
        self.assertNotIn("label.name === 'ci:full'", self.workflow)
        self.assertIn("forceFullReason: 'manual-or-unsupported-event'", self.workflow)

    def test_render_classifier_failures_default_to_full(self) -> None:
        self.assertIn("continue-on-error: true", self.workflow)
        self.assertIn("'fail-closed:impact-unavailable'", self.workflow)
        self.assertIn("forceFullReason: 'collection-error'", self.workflow)


if __name__ == "__main__":
    unittest.main()
