#!/usr/bin/env bash
# [#3508] multi_pr_update_branch.md 2.5의 stale run 수동 정리를 원커맨드로 감싼다.
# [#4025] --author 모드: 한 기여자의 열린 PR run 을 일괄 정리한다(에이전트 오작동 대응).
#
#   사용법: scripts/cancel_stale_pr_runs.sh <PR번호> [--dry-run]
#          scripts/cancel_stale_pr_runs.sh --author <로그인> [--dry-run]
#
# - 현재 head SHA를 확인하고, 같은 PR head(head_repository+head_branch)의 이전 SHA
#   active run만 force-cancel API로 취소한다. 일반 `gh run cancel`은 시도하지 않는다(2.5 규정).
# - 최신 head SHA run은 어떤 경우에도 대상에서 제외한다.
# - 대상 SHA·run URL·완료 상태를 출력한다 — review 문서 기록 요건을 그대로 채운다.
# - fork PR·same-repo PR 모두 동작한다(로컬 gh 인증 토큰 권한 기준).
set -euo pipefail

REPO="${RHWP_REPO:-edwardkim/rhwp}"
PR="${1:?사용법: $0 <PR번호> | --author <로그인>  [--dry-run]}"
DRY="${2:-}"

# ─── [#4025] --author 모드 ────────────────────────────────────────────────
# 2026-08-04 실측: 에이전트 오작동으로 fork PR 69건이 하루에 생성돼 러너가 포화됐다
# (진행 15 + 대기 39). PR 모드는 "최신 head run 보존"이 설계 목적이라 이 상황을 못 푼다
# — 사고에서는 그 최신 run 들이 러너를 먹기 때문이다. 그래서 별도 모드로 둔다.
#
# 안전 경계: **열린 PR 의 head 브랜치 목록과 대조**해 목록 밖(devel·main·타 기여자)은
# 절대 취소하지 않는다. 8/4 실측에서 이 대조가 devel CI 를 지켰다.
if [ "$PR" = "--author" ]; then
  AUTHOR="${2:?사용법: $0 --author <로그인> [--dry-run]}"
  DRY="${3:-}"

  branches=$(gh pr list --repo "$REPO" --state open --author "$AUTHOR" \
    --limit 200 --json headRefName --jq '[.[].headRefName]')
  bcount=$(jq 'length' <<<"$branches")
  echo "author=$AUTHOR  열린 PR head 브랜치 ${bcount}개 (${REPO})"
  if [ "$bcount" -eq 0 ]; then
    echo "  열린 PR 이 없습니다 — 정리할 대상이 없습니다."
    exit 0
  fi

  # [#4025] --paginate 전량 조회는 run 이력이 쌓이면 수 분씩 걸린다(실측 타임아웃).
  # active 상태만 서버에서 걸러 받는다 — 정리 대상은 정의상 active 뿐이다.
  fetch_active() {
    { gh api "repos/$REPO/actions/runs?status=in_progress&per_page=100" --jq '.workflow_runs[]' 2>/dev/null
      gh api "repos/$REPO/actions/runs?status=queued&per_page=100" --jq '.workflow_runs[]' 2>/dev/null; }
  }
  active=$(fetch_active | jq -s --argjson brs "$branches" '
      [ .[]
        | select(.status == "queued" or .status == "in_progress"
                 or .status == "pending" or .status == "requested"
                 or .status == "waiting")
        | select(.head_branch as $b | $brs | index($b))
        | {id, name: (.name // .display_title), branch: .head_branch,
           status, url: .html_url}
      ]')
  acount=$(jq 'length' <<<"$active")

  # 보호된(대조 밖) active run 수 — 요약에 함께 보고한다.
  protected=$(fetch_active | jq -s --argjson brs "$branches" '
      [ .[]
        | select(.status == "queued" or .status == "in_progress")
        | select(.head_branch as $b | ($brs | index($b)) | not)
      ] | length')

  echo "  대상 active run ${acount}건 / 보호(대조 밖) ${protected}건"
  jq -r '.[] | "    [\(.status)] \(.name)  \(.branch)  \(.url)"' <<<"$active" | head -40
  [ "$acount" -gt 40 ] && echo "    … 외 $((acount - 40))건"

  if [ "$DRY" = "--dry-run" ]; then
    echo "  --dry-run: 취소하지 않고 종료합니다."
    exit 0
  fi
  if [ "$acount" -eq 0 ]; then
    echo "  취소할 run 이 없습니다."
    exit 0
  fi

  # 1차 gh run cancel → 2차 force-cancel.
  # 8/4 실측: 1차만으로는 44건이 남았다(취소 반영 지연·신규 큐잉). 2차가 필수다.
  ok=0
  for run_id in $(jq -r '.[].id' <<<"$active"); do
    gh run cancel "$run_id" --repo "$REPO" >/dev/null 2>&1 && ok=$((ok + 1)) || true
  done
  echo "  1차 cancel 요청: ${ok}/${acount}"
  sleep 20

  remain=$(fetch_active | jq -s --argjson brs "$branches" '
      [ .[]
        | select(.status == "queued" or .status == "in_progress")
        | select(.head_branch as $b | $brs | index($b))
        | .id
      ]')
  rcount=$(jq 'length' <<<"$remain")
  if [ "$rcount" -gt 0 ]; then
    echo "  2차 force-cancel 대상 ${rcount}건"
    for run_id in $(jq -r '.[]' <<<"$remain"); do
      gh api --method POST "repos/$REPO/actions/runs/$run_id/force-cancel" >/dev/null 2>&1 || true
    done
    sleep 20
  fi

  final=$(fetch_active | jq -s --argjson brs "$branches" '
      [ .[] | select(.status == "queued" or .status == "in_progress")
        | select(.head_branch as $b | $brs | index($b)) ] | length')
  echo "완료: 대상 ${acount}건 → 잔여 ${final}건 (보호 ${protected}건은 건드리지 않음)"
  [ "$final" -gt 0 ] && echo "  잔여가 있으면 재실행하세요(신규 큐잉 가능성)."
  exit 0
fi
# ─────────────────────────────────────────────────────────────────────────

pr_json=$(gh pr view "$PR" --repo "$REPO" \
  --json state,headRefOid,headRefName,headRepositoryOwner,headRepository)
state=$(jq -r '.state' <<<"$pr_json")
head_sha=$(jq -r '.headRefOid' <<<"$pr_json")
head_ref=$(jq -r '.headRefName' <<<"$pr_json")
head_repo=$(jq -r '"\(.headRepositoryOwner.login)/\(.headRepository.name)"' <<<"$pr_json")

echo "PR #$PR (${REPO})  state=$state"
echo "  head: $head_sha  ($head_repo:$head_ref)"
if [ "$state" != "OPEN" ]; then
  echo "  OPEN 상태가 아니므로 중단합니다."
  exit 1
fi

# 같은 head branch의 pull_request run을 나열하고, 이 PR head(저장소+브랜치)로 좁힌 뒤
# 최신 head SHA를 제외한 active run만 고른다.
stale=$(gh api --paginate \
  "repos/$REPO/actions/runs?event=pull_request&branch=$head_ref&per_page=100" \
  --jq '.workflow_runs[]' 2>/dev/null | jq -s \
  --arg sha "$head_sha" --arg ref "$head_ref" --arg repo "$head_repo" '
    [ .[]
      | select(.head_branch == $ref)
      | select(.head_repository.full_name == $repo)
      | select(.head_sha != $sha)
      | select(.status == "queued" or .status == "in_progress"
               or .status == "pending" or .status == "requested"
               or .status == "waiting")
      | {id, name: (.name // .display_title), head_sha, status, url: .html_url}
    ]')

count=$(jq 'length' <<<"$stale")
if [ "$count" -eq 0 ]; then
  echo "  이전 SHA의 active run 없음 — 정리할 대상이 없습니다."
  exit 0
fi

echo "  이전 SHA active run ${count}건:"
jq -r '.[] | "    [\(.status)] \(.name)  \(.head_sha[0:9])  \(.url)"' <<<"$stale"

if [ "$DRY" = "--dry-run" ]; then
  echo "  --dry-run: 취소하지 않고 종료합니다."
  exit 0
fi

# 취소 직전 live head 재확인 — update가 또 있었으면 새 head run을 건드리지 않는다.
live_sha=$(gh pr view "$PR" --repo "$REPO" --json headRefOid --jq .headRefOid)
if [ "$live_sha" != "$head_sha" ]; then
  echo "  head가 $live_sha 로 이동했습니다. 목록을 다시 만드세요 — 중단."
  exit 1
fi

echo "$stale" | jq -r '.[].id' | while read -r run_id; do
  gh api --method POST "repos/$REPO/actions/runs/$run_id/force-cancel" >/dev/null
  echo "  force-cancel 요청: run $run_id"
done

# completed/cancelled 재확인 (최대 6회 × 10초)
echo "  완료 상태 재확인:"
for run_id in $(jq -r '.[].id' <<<"$stale"); do
  ok=""
  for _ in 1 2 3 4 5 6; do
    rs=$(gh api "repos/$REPO/actions/runs/$run_id" --jq '"\(.status)/\(.conclusion // "-")"')
    if [[ "$rs" == completed/* ]]; then ok="$rs"; break; fi
    sleep 10
  done
  if [ -n "$ok" ]; then
    echo "    run $run_id → $ok"
  else
    echo "    run $run_id → 아직 미완료 (마지막 상태: $rs) — 재확인 필요"
  fi
done
echo "완료. 위 run URL·SHA를 review 문서에 기록하세요."
