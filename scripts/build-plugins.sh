#!/bin/sh
# Builds one marketplace per agent from plugins/distill, in that agent's native layout and with
# only the files it reads. CI publishes these to the marketplace-codex and marketplace-claude
# branches; locally, add an output with `claude plugin marketplace add dist/plugins/claude-code`
# or `codex plugin marketplace add dist/plugins/codex`.
#
# Usage: scripts/build-plugins.sh [--dev] [--build <id>] [claude-code | codex]...
#   --build <id>  Sets the output manifest version to <version>+<agent>.<id>, so agents that
#                 cache plugins by version pick up a new build. Needs jq.
#   --dev         Builds distill-dev under dist/plugins-dev, using the local debug binary
#                 and dist/dev for config, index and vault. Needs jq and cargo.
# Output goes to $DISTILL_PLUGIN_OUT/<agent> (default dist/plugins/<agent>), replaced on each run.
#
# Sources, all copied as is:
#   plugins/distill/{bin,skills}     shared, into the plugin root
#   plugins/distill/<agent>/         manifest, .mcp.json and hooks, into the plugin root
#   packaging/<agent>/               marketplace manifest, into the output root
# Generated:
#   bin/distill-version              the workspace version from Cargo.toml: the release
#                                    bin/distill-launch downloads when no distill is installed

set -eu

root=$(cd "$(dirname "$0")/.." && pwd)
src="$root/plugins/distill"
out="${DISTILL_PLUGIN_OUT:-$root/dist/plugins}"
shared="bin skills"
distill_version=$(sed -n 's/^version = "\(.*\)"$/\1/p' "$root/Cargo.toml" | head -n 1)
[ -n "$distill_version" ] || { echo "build-plugins: no version in $root/Cargo.toml" >&2; exit 2; }

build_id=""
dev=false
while [ $# -gt 0 ]; do
  case "$1" in
    --dev) dev=true; shift ;;
    --build) build_id="${2:?--build needs an id, such as a commit hash}"; shift 2 ;;
    *) break ;;
  esac
done
if [ -n "$build_id" ] || "$dev"; then
  command -v jq >/dev/null || { echo "build-plugins: --build and --dev need jq on PATH" >&2; exit 2; }
fi
plugin_name=distill
if "$dev"; then
  plugin_name=distill-dev
  out="${DISTILL_PLUGIN_OUT:-$root/dist/plugins-dev}"
  dev_home="$root/dist/dev"
  dev_binary=$(cargo metadata --manifest-path "$root/Cargo.toml" --no-deps --format-version 1 | jq -er '.target_directory + "/debug/distill"')
fi

build() { # <agent>
  case "$1" in
    claude-code) manifest=".claude-plugin/plugin.json" ;;
    codex) manifest=".codex-plugin/plugin.json" ;;
  esac
  dest="$out/$1"
  plugin="$dest/plugins/$plugin_name"
  rm -rf "$dest"
  mkdir -p "$plugin"
  for f in $shared; do
    cp -Rp "$src/$f" "$plugin/$f"
  done
  cp -Rp "$src/$1/." "$plugin/"
  echo "$distill_version" >"$plugin/bin/distill-version"
  cp -Rp "$root/packaging/$1/." "$dest/"
  if "$dev"; then
    "$root/scripts/build-dev-plugin.sh" "$1" "$dest" "$dev_home" "$dev_binary"
  fi
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
