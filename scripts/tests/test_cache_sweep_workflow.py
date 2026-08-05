"""[#4080] cache-generation-sweep.yml 의 정리 판정 계약 테스트.

스윕 로직은 checkout 금지 안전 경계 때문에 workflow YAML 안에 인라인되어 있다.
따라서 `test_ci_impact_workflow.py` 의 aggregate shell 과 같은 방식으로, YAML 에서
github-script 본문을 추출해 node 스텁 위에서 실행하고 판정만 단언한다.
"""

from __future__ import annotations

import json
import subprocess
import tempfile
import textwrap
import unittest
from pathlib import Path


WORKFLOW_PATH = (
    Path(__file__).resolve().parents[2] / ".github/workflows/cache-generation-sweep.yml"
)
SCRIPT_MARKER = "          script: |\n"

HARNESS = """
const fixture = %(fixture)s;

const result = {
  deleted: [],
  info: [],
  warnings: [],
  failed: null,
  summary: null,
  calls: [],
};

for (const [key, value] of Object.entries(fixture.env)) {
  process.env[key] = value;
}

const context = { repo: { owner: 'edwardkim', repo: 'rhwp' } };

const core = {
  info: (m) => result.info.push(String(m)),
  warning: (m) => result.warnings.push(String(m)),
  setFailed: (m) => { result.failed = String(m); },
  summary: {
    addHeading() { return this; },
    addTable(rows) { result.summary = rows; return this; },
    async write() { return this; },
  },
};

const listPulls = Symbol('pulls.list');
const listBranches = Symbol('repos.listBranches');
const listTags = Symbol('repos.listTags');
const listCaches = Symbol('actions.getActionsCacheList');

const github = {
  rest: {
    pulls: { list: listPulls },
    repos: { listBranches: listBranches, listTags: listTags },
    actions: {
      getActionsCacheList: listCaches,
      deleteActionsCacheById: async ({ cache_id: id }) => {
        if (fixture.deleteFails && fixture.deleteFails.includes(id)) {
          throw new Error(`stub delete failure ${id}`);
        }
        result.deleted.push(id);
      },
    },
  },
  paginate: async (fn) => {
    if (fn === listPulls) { result.calls.push('pulls'); return fixture.openPrs; }
    if (fn === listBranches) {
      result.calls.push('branches');
      if (fixture.branchesThrow) throw new Error('stub listBranches failure');
      return fixture.branches;
    }
    if (fn === listTags) { result.calls.push('tags'); return fixture.tags || []; }
    if (fn === listCaches) { result.calls.push('caches'); return fixture.caches; }
    throw new Error('unexpected paginate target');
  },
};

(async () => {
%(script)s
})().then(
  () => console.log(JSON.stringify(result)),
  (error) => {
    result.threw = String(error && error.message);
    console.log(JSON.stringify(result));
  },
);
"""


def cache(cid, key, ref, created, mb=100):
    return {
        "id": cid,
        "key": key,
        "ref": ref,
        "created_at": created,
        "size_in_bytes": mb * 1024**2,
    }


class CacheSweepWorkflowTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        workflow = WORKFLOW_PATH.read_text(encoding="utf-8")
        body = workflow.split(SCRIPT_MARKER, maxsplit=1)[1]
        cls.script = textwrap.indent(textwrap.dedent(body), "  ")

    def run_sweep(self, **fixture):
        payload = {
            "env": {
                "DRY_RUN": "false",
                "KEEP_GENERATIONS": "2",
                "SWEEP_ORPHAN_REFS": "true",
                "LIMIT_GB": "10",
                "WARN_PERCENT": "80",
                "FAIL_PERCENT": "95",
            },
            "openPrs": [],
            "branches": [{"name": "devel"}],
            "tags": [],
            "caches": [],
            "deleteFails": [],
            "branchesThrow": False,
        }
        payload.update(fixture)
        payload["env"] = {**payload["env"], **fixture.get("env", {})}

        source = HARNESS % {
            "fixture": json.dumps(payload),
            "script": self.script,
        }
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "harness.mjs"
            path.write_text(source, encoding="utf-8")
            proc = subprocess.run(
                ["node", str(path)], capture_output=True, text=True, check=False
            )
        self.assertEqual(proc.returncode, 0, proc.stderr)
        out = json.loads(proc.stdout.strip().splitlines()[-1])
        self.assertIsNone(out.get("threw"), out.get("threw"))
        return out

    # --- 고아 ref 정리 (#4080 원인 2) ---

    def test_deletes_cache_of_deleted_branch_regardless_of_generation(self):
        out = self.run_sweep(
            branches=[{"name": "devel"}],
            caches=[
                cache(1, "grp-aaaaaaaa", "refs/heads/deleted-branch", "2026-08-05T00:00:00Z"),
                cache(2, "grp-bbbbbbbb", "refs/heads/deleted-branch", "2026-08-04T00:00:00Z"),
            ],
        )
        # 세대가 keep=2 이내여도 ref 가 없으면 전량 삭제한다.
        self.assertEqual(sorted(out["deleted"]), [1, 2])

    def test_deletes_cache_of_closed_pull_request(self):
        out = self.run_sweep(
            openPrs=[{"number": 10}],
            caches=[
                cache(1, "grp-aaaaaaaa", "refs/pull/10/merge", "2026-08-05T00:00:00Z"),
                cache(2, "grp-bbbbbbbb", "refs/pull/99/merge", "2026-08-05T00:00:00Z"),
            ],
        )
        self.assertEqual(out["deleted"], [2], "열린 PR 은 보호하고 닫힌 PR 만 지운다")

    def test_keeps_cache_on_existing_tag_ref(self):
        out = self.run_sweep(
            tags=[{"name": "v1.2.3"}],
            caches=[cache(1, "grp-aaaaaaaa", "refs/tags/v1.2.3", "2026-08-05T00:00:00Z")],
        )
        self.assertEqual(out["deleted"], [])

    def test_reads_caches_before_refs_to_avoid_race(self):
        # 캐시는 자기 ref 보다 먼저 생길 수 없다. 캐시를 먼저 읽어야 조회 사이에 열린
        # PR·브랜치의 캐시를 고아로 오인하지 않는다.
        out = self.run_sweep(
            caches=[cache(1, "grp-aaaaaaaa", "refs/heads/devel", "2026-08-05T00:00:00Z")],
        )
        self.assertEqual(out["calls"][0], "caches", out["calls"])
        self.assertIn("pulls", out["calls"])
        self.assertLess(out["calls"].index("caches"), out["calls"].index("pulls"))
        self.assertLess(out["calls"].index("caches"), out["calls"].index("branches"))

    # --- fail-closed 가드 ---

    def test_skips_orphan_sweep_when_branch_list_is_empty(self):
        out = self.run_sweep(
            branches=[],
            caches=[cache(1, "grp-aaaaaaaa", "refs/heads/devel", "2026-08-05T00:00:00Z")],
        )
        self.assertEqual(out["deleted"], [], "목록을 못 믿으면 아무것도 지우지 않는다")
        self.assertTrue(any("건너뛴다" in w for w in out["warnings"]), out["warnings"])

    def test_skips_orphan_sweep_when_ref_lookup_fails(self):
        out = self.run_sweep(
            branchesThrow=True,
            caches=[cache(1, "grp-aaaaaaaa", "refs/heads/gone", "2026-08-05T00:00:00Z")],
        )
        self.assertEqual(out["deleted"], [])
        self.assertTrue(any("조회 실패" in w for w in out["warnings"]), out["warnings"])

    def test_orphan_sweep_can_be_disabled(self):
        out = self.run_sweep(
            env={"SWEEP_ORPHAN_REFS": "false"},
            caches=[cache(1, "grp-aaaaaaaa", "refs/heads/gone", "2026-08-05T00:00:00Z")],
        )
        self.assertEqual(out["deleted"], [])

    # --- 세대 상한 (#3684 기존 계약) ---

    def test_keeps_latest_generations_per_ref_and_group(self):
        out = self.run_sweep(
            caches=[
                cache(1, "grp-aaaaaaaa", "refs/heads/devel", "2026-08-05T00:00:00Z"),
                cache(2, "grp-bbbbbbbb", "refs/heads/devel", "2026-08-04T00:00:00Z"),
                cache(3, "grp-cccccccc", "refs/heads/devel", "2026-08-03T00:00:00Z"),
            ],
        )
        self.assertEqual(out["deleted"], [3], "최신 2세대를 남기고 가장 오래된 것만 지운다")

    def test_generation_limit_is_per_ref(self):
        out = self.run_sweep(
            branches=[{"name": "devel"}, {"name": "main"}],
            caches=[
                cache(1, "grp-aaaaaaaa", "refs/heads/devel", "2026-08-05T00:00:00Z"),
                cache(2, "grp-bbbbbbbb", "refs/heads/devel", "2026-08-04T00:00:00Z"),
                cache(3, "grp-cccccccc", "refs/heads/main", "2026-08-03T00:00:00Z"),
            ],
        )
        self.assertEqual(out["deleted"], [], "ref 가 다르면 서로의 세대를 잠식하지 않는다")

    def test_dry_run_deletes_nothing(self):
        out = self.run_sweep(
            env={"DRY_RUN": "true"},
            caches=[
                cache(1, "grp-aaaaaaaa", "refs/heads/gone", "2026-08-05T00:00:00Z"),
                cache(2, "grp-aaaaaaaa", "refs/heads/devel", "2026-08-05T00:00:00Z"),
                cache(3, "grp-bbbbbbbb", "refs/heads/devel", "2026-08-04T00:00:00Z"),
                cache(4, "grp-cccccccc", "refs/heads/devel", "2026-08-03T00:00:00Z"),
            ],
        )
        self.assertEqual(out["deleted"], [])
        self.assertTrue(any("(예정)" in line for line in out["info"]), out["info"])

    def test_delete_failure_is_a_warning_not_a_crash(self):
        out = self.run_sweep(
            deleteFails=[1],
            caches=[cache(1, "grp-aaaaaaaa", "refs/heads/gone", "2026-08-05T00:00:00Z")],
        )
        self.assertEqual(out["deleted"], [])
        self.assertTrue(any("삭제 실패" in w for w in out["warnings"]), out["warnings"])

    # --- 한도 경보 (#4080 제안 3) ---

    def test_fails_when_post_sweep_total_exceeds_fail_threshold(self):
        out = self.run_sweep(
            caches=[cache(1, "grp-aaaaaaaa", "refs/heads/devel", "2026-08-05T00:00:00Z", mb=9800)],
        )
        self.assertIsNotNone(out["failed"], "한도의 95% 초과는 실패로 드러낸다")

    def test_warns_when_post_sweep_total_exceeds_warn_threshold(self):
        out = self.run_sweep(
            caches=[cache(1, "grp-aaaaaaaa", "refs/heads/devel", "2026-08-05T00:00:00Z", mb=8600)],
        )
        self.assertIsNone(out["failed"])
        self.assertTrue(
            any("경고 임계" in w for w in out["warnings"]), out["warnings"]
        )

    def test_quiet_when_total_is_below_thresholds(self):
        out = self.run_sweep(
            caches=[cache(1, "grp-aaaaaaaa", "refs/heads/devel", "2026-08-05T00:00:00Z", mb=1000)],
        )
        self.assertIsNone(out["failed"])
        self.assertEqual([w for w in out["warnings"] if "임계" in w], [])

    def test_threshold_uses_post_sweep_total_not_pre_sweep(self):
        # 정리 전에는 한도를 넘지만 고아를 지우고 나면 임계 아래로 내려간다.
        out = self.run_sweep(
            caches=[
                cache(1, "grp-aaaaaaaa", "refs/heads/gone", "2026-08-05T00:00:00Z", mb=8000),
                cache(2, "grp-bbbbbbbb", "refs/heads/devel", "2026-08-05T00:00:00Z", mb=1000),
            ],
        )
        self.assertEqual(out["deleted"], [1])
        self.assertIsNone(out["failed"])
        self.assertEqual([w for w in out["warnings"] if "임계" in w], [])

    # --- summary 계약 ---

    def test_summary_reports_orphan_and_generation_separately(self):
        out = self.run_sweep(
            caches=[
                cache(1, "grp-aaaaaaaa", "refs/heads/gone", "2026-08-05T00:00:00Z"),
                cache(2, "grp-bbbbbbbb", "refs/heads/devel", "2026-08-05T00:00:00Z"),
                cache(3, "grp-cccccccc", "refs/heads/devel", "2026-08-04T00:00:00Z"),
                cache(4, "grp-dddddddd", "refs/heads/devel", "2026-08-03T00:00:00Z"),
            ],
        )
        labels = [row[0] for row in out["summary"] if isinstance(row[0], str)]
        for expected in ["고아 ref", "구 세대", "한도 대비", "고아 ref 정리"]:
            self.assertIn(expected, labels)


if __name__ == "__main__":
    unittest.main()
