#!/bin/sh
# Applies the local debugging overlay to a staged plugin. Called by build-plugins.sh.
set -eu

agent=$1
dest=$2
dev_home=$3
dev_binary=$4
root=$(cd "$(dirname "$0")/.." && pwd)
plugin="$dest/plugins/distill-dev"
case "$agent" in
  codex)
    manifest=.codex-plugin/plugin.json
    marketplace=.agents/plugins/marketplace.json
    ;;
  claude-code)
    manifest=.claude-plugin/plugin.json
    marketplace=.claude-plugin/marketplace.json
    ;;
esac

jq '.name = "distill-dev" | .description = "Local Distill development with an isolated vault." |
    .interface.displayName = "Distill dev"' "$plugin/$manifest" >"$plugin/$manifest.tmp"
mv "$plugin/$manifest.tmp" "$plugin/$manifest"
jq '.name = "distill-dev" | .interface.displayName = "Distill dev" |
    .plugins[0].name = "distill-dev" |
    if (.plugins[0].source | type) == "object" then
      .plugins[0].source.path = "./plugins/distill-dev"
    else .plugins[0].source = "./plugins/distill-dev" end' \
    "$dest/$marketplace" >"$dest/$marketplace.tmp"
mv "$dest/$marketplace.tmp" "$dest/$marketplace"
jq '.mcpServers["distill-dev"] = .mcpServers.distill |
    del(.mcpServers.distill) | .mcpServers["distill-dev"].title = "Distill dev" |
    .mcpServers["distill-dev"].description = "Distill dev: local debugging tools; isolated from saved Distill notes."' \
    "$plugin/.mcp.json" >"$plugin/.mcp.json.tmp"
mv "$plugin/.mcp.json.tmp" "$plugin/.mcp.json"

mv "$plugin/skills/distill" "$plugin/skills/distill-dev"
sed -e 's/^name: distill$/name: distill-dev/' -e 's/^# Distill$/# Distill dev/' \
    -e 's/distill-source/distill-dev-source/g' -e 's/distill-suggest/distill-dev-suggest/g' \
    -e '/^Read this once/i\
Use only tools from the `distill-dev` MCP server for this skill. Its vault is for local\
debugging. Use the plugin launcher for CLI commands; a plain `distill` command uses the\
regular setup. The launcher initializes the dev vault when the MCP server starts.\
' "$plugin/skills/distill-dev/SKILL.md" >"$plugin/skills/distill-dev/SKILL.md.tmp"
mv "$plugin/skills/distill-dev/SKILL.md.tmp" "$plugin/skills/distill-dev/SKILL.md"
cp "$root/scripts/distill-dev-launch.sh" "$plugin/bin/distill-launch"
chmod +x "$plugin/bin/distill-launch"
printf '%s\n' "$dev_home" >"$plugin/bin/distill-dev-home"
printf '%s\n' "$dev_binary" >"$plugin/bin/distill-dev-binary"
rm "$plugin/bin/distill-version"
