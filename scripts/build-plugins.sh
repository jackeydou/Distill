#!/bin/sh
# Builds one marketplace per agent from plugins/distill, in that agent's native layout and with
# only the files it reads. CI publishes these to the marketplace-codex and marketplace-claude
# branches; locally, add an output with `claude plugin marketplace add dist/plugins/claude-code`
# or `codex plugin marketplace add dist/plugins/codex`.
#
# Usage: scripts/build-plugins.sh [--build <id>] [claude-code | codex]...   (no agent: both)
#   --build <id>  Sets the output manifest version to <version>+<agent>.<id>, so agents that
#                 cache plugins by version pick up a new build. Needs jq.
# Output goes to $DISTILL_PLUGIN_OUT/<agent> (default dist/plugins/<agent>), replaced on each run.
#
# Sources, all copied as is:
#   plugins/distill/{bin,skills}     shared, into the plugin root
#   plugins/distill/<agent>/         manifest, .mcp.json and hooks, into the plugin root
#   packaging/<agent>/               marketplace manifest, into the output root

set -eu

root=$(cd "$(dirname "$0")/.." && pwd)
src="$root/plugins/distill"
out="${DISTILL_PLUGIN_OUT:-$root/dist/plugins}"
shared="bin skills"

build_id=""
if [ "${1:-}" = "--build" ]; then
  build_id="${2:?--build needs an id, such as a commit hash}"
  shift 2
  command -v jq >/dev/null || { echo "build-plugins: --build needs jq on PATH" >&2; exit 2; }
fi

build() { # <agent>
  case "$1" in
    claude-code) manifest=".claude-plugin/plugin.json" ;;
    codex) manifest=".codex-plugin/plugin.json" ;;
  esac
  dest="$out/$1"
  plugin="$dest/plugins/distill"
  rm -rf "$dest"
  mkdir -p "$plugin"
  for f in $shared; do
    cp -Rp "$src/$f" "$plugin/$f"
  done
  cp -Rp "$src/$1/." "$plugin/"
  cp -Rp "$root/packaging/$1/." "$dest/"
  if [ -n "$build_id" ]; then
    jq --arg id "$1.$build_id" '.version += "+" + $id' "$plugin/$manifest" >"$plugin/$manifest.tmp"
    mv "$plugin/$manifest.tmp" "$plugin/$manifest"
  fi
  find "$dest" -name .DS_Store -delete
  echo "$1: $dest"
}

[ $# -gt 0 ] || set -- claude-code codex
for agent in "$@"; do
  case "$agent" in
    claude-code | codex) ;;
    *)
      echo "build-plugins: unknown agent '$agent'. Pass claude-code, codex, or nothing for both." >&2
      exit 2
      ;;
  esac
done
for agent in "$@"; do
  build "$agent"
done
