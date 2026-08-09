"""[#3790 Stage 5A] CodeQL 보안 판정 재사용과 Rust no-build shadow 계약."""

from __future__ import annotations

import json
import re
import subprocess
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
CODEQL_WORKFLOW = REPO_ROOT / ".github/workflows/codeql.yml"


def job_body(workflow: str, job_name: str) -> str:
    match = re.search(
        rf"(?ms)^  {re.escape(job_name)}:\n.*?(?=^  [A-Za-z0-9_-]+:\n|\Z)",
        workflow,
    )
    if match is None:
        raise AssertionError(f"codeql.yml에 {job_name} job이 없다")
    return match.group(0)


class CodeQLStage5AWorkflowTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.workflow = CODEQL_WORKFLOW.read_text(encoding="utf-8")
        script = cls.workflow.split("script: |\n", maxsplit=1)[1].split(
            "\n      # 기준선 병합을 fast-pass bridge로", maxsplit=1
        )[0]
        cls.preflight_script = "\n".join(
            line.removeprefix("            ") for line in script.splitlines()
        )

    def test_reused_result_requires_candidate_bound_security_check(self) -> None:
        workflow = self.workflow
        self.assertIn("github.rest.checks.listForRef", workflow)
        self.assertIn("ref: candidateSha", workflow)
        self.assertIn("check.app?.slug === 'github-advanced-security'", workflow)
        self.assertIn("check.name === 'CodeQL'", workflow)
        self.assertIn("check.head_sha === candidateSha", workflow)
        self.assertIn("workflowRun.run_started_at || workflowRun.created_at", workflow)
        self.assertIn("securityCheckStartedAt < runAttemptStartedAt", workflow)
        self.assertIn("missing-security-check:CodeQL:${candidateSha}", workflow)
        self.assertIn("security-check-not-completed:CodeQL:${securityCheck.status}", workflow)
        self.assertIn("security-check-not-green:CodeQL:${securityCheck.conclusion}", workflow)
        self.assertIn("securityCheck.conclusion !== 'success'", workflow)
        self.assertLess(
            workflow.index("securityCheck.conclusion !== 'success'"),
            workflow.index("return { state: 'green' };"),
        )

    def test_green_analyze_jobs_cannot_reuse_a_failed_security_check(self) -> None:
        outputs = self._run_preflight("failure")
        self.assertEqual(outputs["fast_pass"], "false")
        self.assertEqual(outputs["candidate_sha"], "code-candidate")
        self.assertEqual(
            outputs["reason"],
            "security-check-not-green:CodeQL:failure",
        )

    def test_green_analyze_jobs_and_security_check_remain_reusable(self) -> None:
        outputs = self._run_preflight("success")
        self.assertEqual(outputs["fast_pass"], "true")
        self.assertEqual(outputs["candidate_sha"], "code-candidate")
        self.assertEqual(outputs["reason"], "codeql-checks-green")

    def test_security_check_from_an_earlier_run_attempt_is_not_reused(self) -> None:
        outputs = self._run_preflight(
            "success",
            run_started_at="2026-08-09T00:18:00Z",
            security_started_at="2026-08-09T00:15:00Z",
        )
        self.assertEqual(outputs["fast_pass"], "false")
        self.assertEqual(outputs["reason"], "no-green-codeql-candidate")

    def test_blocking_three_language_matrix_and_rust_prebuild_stay_unchanged(self) -> None:
        analyze = job_body(self.workflow, "analyze")
        self.assertIn("language: [javascript-typescript, python, rust]", analyze)
        self.assertIn("languages: ${{ matrix.language }}", analyze)
        self.assertIn("Build Rust (for CodeQL)", analyze)
        self.assertIn("run: cargo build", analyze)
        self.assertIn("actions/cache/restore@v6", analyze)
        self.assertIn("actions/cache/save@v6", analyze)
        self.assertNotIn("build-mode: none", analyze)

    def test_rust_no_build_shadow_is_isolated_from_code_scanning(self) -> None:
        shadow = job_body(self.workflow, "rust-no-build-shadow")
        self.assertIn("name: Rust no-build shadow", shadow)
        self.assertIn("needs: preflight", shadow)
        self.assertIn("github.event_name == 'pull_request'", shadow)
        self.assertIn("needs.preflight.result == 'success'", shadow)
        self.assertIn("needs.preflight.outputs.fast_pass != 'true'", shadow)
        self.assertIn("languages: rust", shadow)
        self.assertIn("build-mode: none", shadow)
        self.assertIn(
            "dtolnay/rust-toolchain@4cda84d5c5c54efe2404f9d843567869ab1699d4",
            shadow,
        )
        self.assertIn("upload: never", shadow)
        self.assertIn("output: rust-no-build-results", shadow)
        self.assertIn(
            "actions/upload-artifact@043fb46d1a93c77aae656e7c1c64a875d1fc6a0a",
            shadow,
        )
        self.assertIn("if: ${{ always() }}", shadow)
        self.assertIn("path: rust-no-build-results", shadow)
        self.assertIn("if-no-files-found: warn", shadow)
        self.assertNotIn("actions/cache/", shadow)
        self.assertNotIn("cargo build", shadow)

    def _run_preflight(
        self,
        security_conclusion: str,
        *,
        run_started_at: str = "2026-08-09T00:10:00Z",
        security_started_at: str = "2026-08-09T00:19:00Z",
    ) -> dict[str, str]:
        harness = """
const outputs = {};
const endpoints = {
  listWorkflowRuns: Symbol('listWorkflowRuns'),
  listJobsForWorkflowRun: Symbol('listJobsForWorkflowRun'),
  listFiles: Symbol('listFiles'),
  listCommits: Symbol('listCommits'),
  listForRef: Symbol('listForRef'),
};
const commits = {
  'review-record': {
    parents: [{ sha: 'code-candidate' }],
    files: [{ filename: 'mydocs/working/review.md', status: 'modified' }],
  },
  'code-candidate': {
    parents: [{ sha: 'base-sha' }],
    files: [{ filename: 'src/lib.rs', status: 'modified' }],
  },
};
const github = {
  rest: {
    actions: {
      listWorkflowRuns: endpoints.listWorkflowRuns,
      listJobsForWorkflowRun: endpoints.listJobsForWorkflowRun,
    },
    pulls: {
      listFiles: endpoints.listFiles,
      listCommits: endpoints.listCommits,
    },
    checks: { listForRef: endpoints.listForRef },
    repos: {
      getCommit: async ({ ref }) => ({ data: commits[ref] }),
    },
  },
  paginate: async (endpoint, params) => {
    if (endpoint === endpoints.listFiles) {
      return [
        { filename: 'src/lib.rs', status: 'modified' },
        { filename: 'mydocs/working/review.md', status: 'modified' },
      ];
    }
    if (endpoint === endpoints.listCommits) {
      return [{ sha: 'code-candidate' }, { sha: 'review-record' }];
    }
    if (endpoint === endpoints.listWorkflowRuns) {
      return [{
        id: 3790,
        path: '.github/workflows/codeql.yml',
        event: 'pull_request',
        head_sha: 'code-candidate',
        head_branch: 'feature-3790',
        head_repository: { id: 7 },
        status: 'completed',
        conclusion: 'success',
        created_at: '2026-08-09T00:10:00Z',
        run_started_at: RUN_STARTED_AT,
        completed_at: '2026-08-09T00:20:00Z',
      }];
    }
    if (endpoint === endpoints.listJobsForWorkflowRun) {
      return [
        'Analyze (javascript-typescript)',
        'Analyze (python)',
        'Analyze (rust)',
      ].map((name) => ({
        name,
        status: 'completed',
        conclusion: 'success',
        completed_at: '2026-08-09T00:19:00Z',
      }));
    }
    if (endpoint === endpoints.listForRef) {
      return [{
        name: 'CodeQL',
        app: { slug: 'github-advanced-security' },
        head_sha: params.ref,
        status: 'completed',
        conclusion: SECURITY_CONCLUSION,
        started_at: SECURITY_STARTED_AT,
        completed_at: '2026-08-09T00:21:00Z',
      }];
    }
    throw new Error('unexpected paginate endpoint');
  },
};
const context = {
  eventName: 'pull_request',
  repo: { owner: 'edwardkim', repo: 'rhwp' },
  payload: {
    pull_request: {
      number: 4310,
      created_at: '2026-08-09T00:00:00Z',
      base: { sha: 'base-sha' },
      head: { ref: 'feature-3790', repo: { id: 7 } },
    },
  },
};
const core = {
  setOutput: (name, value) => { outputs[name] = String(value); },
  info: () => {},
  warning: () => {},
};
(async () => {
PREFLIGHT_SCRIPT
})().then(() => {
  process.stdout.write(JSON.stringify(outputs));
}).catch((error) => {
  process.stderr.write(String(error.stack || error));
  process.exitCode = 1;
});
""".replace("SECURITY_CONCLUSION", json.dumps(security_conclusion)).replace(
            "RUN_STARTED_AT", json.dumps(run_started_at)
        ).replace("SECURITY_STARTED_AT", json.dumps(security_started_at)).replace(
            "PREFLIGHT_SCRIPT", self.preflight_script
        )
        completed = subprocess.run(
            ["node"],
            input=harness,
            check=False,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        self.assertEqual(completed.returncode, 0, completed.stderr)
        return json.loads(completed.stdout)


if __name__ == "__main__":
    unittest.main()
