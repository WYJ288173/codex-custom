#!/bin/sh
set -eu

if [ "$#" -ne 1 ]; then
    echo "用法: $0 <upstream-tag>" >&2
    exit 2
fi

repo_root="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
tag="$1"
feature_branch="feature/claude-style-statusline"
temporary_branch="update/claude-statusline-${tag}"

cd "$repo_root"
if [ -n "$(git status --porcelain)" ]; then
    echo "更新前工作区必须干净" >&2
    exit 1
fi
if [ "$(git branch --show-current)" != "$feature_branch" ]; then
    echo "请先切换到 $feature_branch" >&2
    exit 1
fi

git fetch upstream --tags
git rev-parse --verify "${tag}^{commit}" >/dev/null
old_base="$(git merge-base "$feature_branch" upstream/main)"

cleanup() {
    git rebase --abort >/dev/null 2>&1 || true
    git switch "$feature_branch" >/dev/null 2>&1 || true
    git branch -D "$temporary_branch" >/dev/null 2>&1 || true
}
trap cleanup EXIT HUP INT TERM

git switch -c "$temporary_branch"
git rebase --onto "$tag" "$old_base"
(
    cd codex-rs
    just fmt
    just fix -p codex-tui
    just test -p codex-core config_toml_status_line_layout -- --nocapture
    just test -p codex-tui
)
"$repo_root/scripts/install-custom-codex.sh"

git switch "$feature_branch"
git merge --ff-only "$temporary_branch"
git branch -D "$temporary_branch"
trap - EXIT HUP INT TERM
echo "已更新到 $tag；确认后执行 git push origin $feature_branch"
