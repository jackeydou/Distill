# Install Distill

Written so an agent can follow it step by step. If you are an agent: run each check, skip the
steps that are already done, and ask the user only where a step says so.

`<source>` below is this repository: its Git URL, or the path of a local clone.

## 1. The `distill` binary

Check: `distill --version` prints `distill 0.0.1` or later.

If it is missing, build it (needs a Rust toolchain; `rustup` or `mise` provides one):

```bash
cargo install --path crates/distill-cli
```

Run that from a clone of `<source>`. With only a Git URL:
`cargo install --git <source> distill-cli`.

## 2. This device

Check: `distill doctor` does not report "no Distill config".

If it does, **ask the user where to keep their vault** before running anything. Tell them:

- On macOS with iCloud Drive, `iCloud Drive/Distill` syncs across their Macs.
- Otherwise the default is `Distill` in their documents folder; any folder inside Dropbox,
  OneDrive or Syncthing also syncs.
- If another device already has a vault, point at the synced copy of that folder.

Then run, with their answer:

```bash
distill init --vault "<their folder>"
```

`icloud:Distill` is accepted on macOS as shorthand for `iCloud Drive/Distill`.

## 3. The plugin

Install it in the agent(s) the user works with. Both commands are safe to re-run.

Codex:

```bash
codex plugin marketplace add <source>
codex plugin add distill@distill
```

Claude Code:

```bash
claude plugin marketplace add <source>
claude plugin install distill@distill
```

Codex App and Claude desktop can add the same marketplace from their plugin settings instead.

## 4. Check

```bash
distill doctor
```

It should list the plugin under "Plugins" and end with "All good." Then tell the user to open a
new session: plugins load when a session starts.

## Uninstall

`codex plugin remove distill@distill`, `claude plugin uninstall distill@distill`, and
`cargo uninstall distill-cli`. The vault is plain Markdown and stays where it is.
