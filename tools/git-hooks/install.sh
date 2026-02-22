#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$repo_root"

hook_source="$repo_root/tools/git-hooks/pre-commit"

hooks_path=$(git config --get core.hooksPath || true)
if [[ -n "$hooks_path" ]]; then
  if [[ "$hooks_path" = /* ]]; then
    hook_dir="$hooks_path"
  else
    hook_dir="$repo_root/$hooks_path"
  fi
else
  hook_dir="$repo_root/.git/hooks"
fi

mkdir -p "$hook_dir"

hook_target="$hook_dir/pre-commit"

force=false
if [[ "${1:-}" == "--force" ]]; then
  force=true
fi

if [[ -e "$hook_target" && "$force" == false ]]; then
  if [[ -L "$hook_target" && "$(readlink "$hook_target")" == "$hook_source" ]]; then
    echo "pre-commit hook already installed."
    exit 0
  fi
  echo "pre-commit hook already exists at $hook_target."
  echo "Re-run with --force to overwrite."
  exit 1
fi

chmod +x "$hook_source"
ln -sf "$hook_source" "$hook_target"

echo "Installed pre-commit hook to $hook_target"
