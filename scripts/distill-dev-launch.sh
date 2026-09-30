#!/bin/sh
# The dev package pins local paths so installation into an agent cache keeps the same vault.
set -eu

here=$(cd "$(dirname "$0")" && pwd)
export DISTILL_HOME="$(cat "$here/distill-dev-home")"
export DISTILL_VAULT="$DISTILL_HOME/vault"
bin=$(cat "$here/distill-dev-binary")
export DISTILL_DEV=1
unset PORT

if [ ! -x "$bin" ]; then
  [ "${1:-}" != hook ] || exit 0
  echo "Distill dev: local binary $bin is missing. Run mise run build:plugins:dev in the source checkout." >&2
  exit 127
fi

if [ "${1:-}" = hook ]; then
  # A prompt never initializes a vault or opens an index.
  [ -f "$DISTILL_HOME/config/config.toml" ] || exit 0
  "$bin" "$@" | sed -e 's/distill-source:/distill-dev-source:/g' \
    -e 's/distill-suggest:/distill-dev-suggest:/g' \
    -e 's/Distill is installed/Distill dev is installed/g' \
    -e 's/the distill skill/the distill-dev skill/g' \
    -e 's/to distill\./to distill. Use only the distill-dev MCP server for this dev context./g'
  exit 0
fi

if [ ! -f "$DISTILL_HOME/config/config.toml" ]; then
  "$bin" init --vault "$DISTILL_VAULT" >/dev/null
  "$bin" config set ui.port 4778 >/dev/null
fi
exec "$bin" "$@"
