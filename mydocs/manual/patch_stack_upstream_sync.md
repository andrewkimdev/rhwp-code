---
kind: guide
status: active
canonical: mydocs/manual/patch_stack_upstream_sync.md
last_verified: 2026-08-22
---

# 로컬 패치 스택을 최신 upstream 위에 재적용하는 절차

이 문서는 **[포크 수확 규약](fork_harvest_convention.md)의 반대 방향**을 다룬다 — 그 문서는
"우리 포크의 개선을 upstream(edwardkim/rhwp)이 어떻게 거둬 가는가"이고, 이 문서는 "upstream이
계속 앞서 나가는 동안 우리가 로컬에 쌓아 둔 patch 브랜치를, 매번 오래된 병합을 다시 풀지 않고
어떻게 최신 위에 다시 얹는가"다.

## 왜 `git merge`가 아니라 재적용인가

`patch/*`, `investigate/*` 같은 로컬 브랜치는 만들어진 시점의 `main`에서 갈라진다. 시간이
지나 `main`이 계속 앞서 나가면(자체 커밋이든 upstream을 받아온 것이든) 그 브랜치는 점점
더 오래된 base 위에 서 있게 된다. 이 상태를 `git merge`로 `main`에 합치면, 실제로 바뀐
내용과 무관하게 **그 사이에 쌓인 main의 전체 커밋 수만큼의 drift를 배경으로 병합 결과를
추론해야 한다** — 충돌이 파일 하나뿐이어도 "정말 이게 맞는 병합인가"를 그 drift 전체를
염두에 두고 판단해야 하니 부담이 줄지 않는다.

반대로, 브랜치가 담고 있는 **개별 커밋들을 최신 `main` 위에 하나씩 cherry-pick**하면:

- 각 커밋은 자기 자신의 diff만큼만 충돌 가능성을 가진다. 드리프트가 152개든 1000개든
  상관없다 — 드리프트가 실제로 겹치는 지점에서만, 그 지점만큼만 충돌한다.
- 결과가 곧 "최신 `main` + 이 patch들"이라는 명확한 선형 이력이 된다. `git merge`가 만드는
  병합 커밋과 달리 각 원래 커밋의 메시지·저자·리뷰 맥락이 그대로 보존된다.
- 스택 중 일부만 원하는 경우(아래 참고) 자연스럽게 선택적으로 반영할 수 있다.

## 실측 사례 (2026-08-22)

`rhwp-code`의 `main`은 자체 커밋만으로 `upstream/main`(edwardkim/rhwp) 대비 이미 152개
앞서 있었고 — 즉 이 사례는 서드파티 upstream 자체를 당겨오는 상황이 아니라, **우리 포크의
`main`이 우리 자신의 patch 브랜치들보다 152 커밋 앞서 나간 상황**이었다. `investigate/17958715-
nested-transform`을 `git merge`로 합치는 시도가 진행 중이었으나, 실제로는 이 문서의 절차로
전환해 `main`(당시 tip `cadab78a6`) 위에 14개 커밋을 cherry-pick으로 재적용했다. 전체
과정과 판단 근거는 이 repo가 아니라 `chosun-form/rhwp-code`의 작업 세션 기록에 있다 — 아래
절차는 그 경험을 일반화한 것이다.

## 절차

### 1. 최신 상태와 실제 drift 확인

```bash
git fetch origin && git fetch upstream
git log --oneline -1 main
git log --oneline -1 origin/main   # 또는 upstream/main — 무엇을 기준으로 재적용할지에 따라
git rev-list --left-right --count main...upstream/main
```

`ahead/behind` 숫자만 보고 판단하지 않는다 — `main`이 앞서 있어도 그 사이 커밋들이 patch
브랜치가 만들려는 변경을 이미 다른 형태로 흡수했을 수 있다. **반드시** 아래 2단계로
patch-id 기준 중복 여부를 확인한다.

### 2. 각 patch 브랜치가 이미 반영됐는지 `git cherry`로 확인

```bash
git cherry -v main <patch-branch>
```

`-` 접두사는 patch-id가 이미 `main`에 있다는 뜻(반영됨), `+`는 아직 없다는 뜻이다. **모든
브랜치를 병합 대상으로 가정하지 말고**, 이름이 비슷해 보이는 커밋(예: 같은 파일을 건드리는
다른 버그 수정)을 patch-id만으로 오탐/누락하지 않도록, 의심되는 커밋은 파일 단위로도
한 번 더 확인한다(`git show --stat <hash>`로 건드리는 파일을 확인하고, `main`의 해당 파일
현재 상태에서 같은 심볼/로직이 이미 존재하는지 grep).

### 3. 브랜치 인벤토리 정리 — 스택 구조, 중복 계열, 폐기 대상 식별

`patch/vX.Y-*` 명명은 브랜치가 늘어날수록 **독립된 변경이 아니라 서로를 포함하는 선형
스택**이 되기 쉽다. 반영 전에 반드시 확인한다:

```bash
# A가 B의 조상인지 (스택 관계 확인)
git merge-base --is-ancestor <A> <B> && echo "A는 B에 포함됨"

# 버전 접두어가 바뀐 예전 시리즈(v0.7.19 -> v0.8.4 같은)가 있다면 diff로 동일 변경인지 확인
git diff <old-branch> <new-branch> --stat
```

- 스택의 **tip 하나만 cherry-pick하면 그 아래 전체가 함께 딸려 온다** — "9개의 독립 패치"처럼
  보여도 실제로는 최신 tip 하나 + 원치 않는 중간 커밋 제외 목록으로 이해해야 한다.
- 오래된 버전 접두어 시리즈(예: `v0.7.19-*`)가 최신 시리즈(`v0.8.4-*`)와 동일한 로직을 담고
  있다면 오래된 쪽은 폐기 대상이다 — 최신 쪽만 재적용한다.
- 커밋 메시지나 관련 설계 문서에 `(spike)` 표시, "아직 Current patch로 승격되지 않음" 같은
  자기 신고가 있으면 **기본적으로 제외**하고, 포함 여부는 작업지시자에게 확인한다 — 스택
  중간에 끼어 있어도 예외 없이 적용한다.

### 4. 로컬 전용 브랜치는 먼저 백업

```bash
git branch -a -vv   # origin에 대응 원격 브랜치가 없는 로컬 전용 브랜치를 먼저 찾는다
git push origin <local-only-branch>
```

병합·리베이스 시도로 로컬 브랜치 상태를 건드리기 전에, `origin`에 아직 한 번도 push되지
않은 브랜치가 있다면 반드시 먼저 백업 push한다. 이 브랜치의 ref 자체는 이후 작업(진행 중인
merge를 abort하는 것 포함)으로 사라지지 않지만, 로컬 저장소가 유일한 사본인 상태로 큰
변경을 계속 진행하는 위험을 없앤다.

### 5. 진행 중인 merge가 있다면 abort하고 작업 브랜치에서 재적용

```bash
git merge --abort   # 원래 브랜치가 그대로 남아 있으므로 안전하게 되돌릴 수 있다
git switch -c consolidated/<주제> main
git cherry-pick <hash1> <hash2> ...   # 3단계에서 정리한 순서대로, 제외 대상은 건너뛴다
```

충돌은 각 커밋 단위로 개별적으로 발생한다. 특히 **건너뛴 스택 중간 커밋에 의존하는 후속
커밋**은 그 커밋이 남긴 주변 컨텍스트(주석, 인접 코드)가 diff 컨텍스트에 섞여 충돌로
나타날 수 있다 — 이때 `git show <원본 hash> -- <파일>`로 그 커밋이 **실제로** 무엇을
바꿨는지 확인하고(건너뛴 커밋이 남긴 컨텍스트 vs 이 커밋의 진짜 변경), 후자만 반영한다.

### 6. 검증

- Rust: `cargo build`, 그리고 영향받은 모듈의 `cargo test`(전체 `cargo test --lib`가 몇 분
  안에 끝나는 규모라면 전체 실행을 권장 — 전체 실행 승인 규칙은
  [문서·Git 워크플로 > PR Workflow](codex/docs_and_git_workflow.md)를 따른다).
- `rhwp-studio`: `npm run build`(`tsc && vite build`)와 `npm test`.
- 여러 그룹(예: 성능 패치 스택 + UI 기능 두 개)을 함께 재적용했다면, 서로 다른 그룹이 같은
  파일(`rhwp-studio/src/main.ts`의 메뉴 등록부처럼)을 건드릴 수 있으므로 그룹별로 순차
  cherry-pick 후 매 그룹이 끝날 때마다 빌드를 확인한다.

### 7. `main`에 반영하고 정리

```bash
git switch main
git merge --ff-only consolidated/<주제>   # 재적용이 fast-forward라는 것 자체가 검증이다
git branch -d consolidated/<주제>
```

`origin/main` push는 [문서·Git 워크플로](codex/docs_and_git_workflow.md)의 Branch And PR
Rule을 따른다 — 공유 브랜치 변경이므로 별도 승인 없이는 push하지 않는다.

## 참고

- 반대 방향(우리 개선을 upstream이 거둬 가게 하는 것)은 [포크 수확 규약](fork_harvest_convention.md).
- 로컬 작업·검증 기준 브랜치, PR 대상 브랜치, push 승인 규칙은
  [문서·Git 워크플로 > Branch And PR Rule](codex/docs_and_git_workflow.md#branch-and-pr-rule).
