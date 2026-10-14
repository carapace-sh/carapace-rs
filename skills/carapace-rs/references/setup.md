# Setup and Shell Integration

Registering snippets, per-shell notes, styles, and the `CARAPACE_*` environment variables.

> For what the environment variables *do* in detail, see the **carapace** skill → `references/setup.md` (same semantics). This document covers invocation and the exact variable list carapace-rs reads.

## Snippet generation

The snippet is emitted by the binary itself — no separate install step:

```bash
source <(mybin _carapace bash)              # detected shell: `mybin _carapace` (no arg)
mybin _carapace zsh > ~/.zfunc/_mybin       # zsh: dump to fpath
mybin _carapace fish | source
```

Accepted shell names (`Shell::parse`): `bash`, `bash-ble`, `cmd-clink`, `elvish`, `fish`, `nushell`, `oil`, `powershell`, `tcsh`, `xonsh`, `zsh`, `export`. There is no `osh` alias — oil is `oil`.

The word list after the shell mirrors the snippets: command name first, word being completed last (dropped before traversal). Snippets invoke `{binary} _carapace <shell>` — the binary name, not an absolute path; `PATH` lookup covers registration.

## `_carapace` entry dispatch

| Invocation | Behavior |
|---|---|
| `_carapace` | snippet for the detected shell |
| `_carapace <shell>` | snippet for the named shell |
| `_carapace <shell> <cmd> <words…>` | completion request |
| `_carapace spec` | YAML dump of the command tree (`spec` feature, default on) |
| `_carapace macro [name [words…]]` | external macro protocol |
| `_carapace style set <name> <style>` | persist to `styles.json` |

Unknown shells return `None` so the program runs normally. The `export` shell produces the export JSON used for bridging, caching and embedding.

## Styles

User styles persist in `$XDG_CONFIG_HOME|$HOME/.config/carapace/styles.json` and are settable from the completion UI or the command line:

```bash
mybin _carapace style set main.commands blue
```

## Environment variables

Read from the context environment (completion) or process environment (entry-point only):

| Variable | Effect |
|---|---|
| `CARAPACE_LOG` | enable logging (set, non-empty, != `0`) |
| `CARAPACE_COLOR` | `"0"` disables color (also honors `NO_COLOR` / `CLICOLOR=0`) |
| `CARAPACE_NOSPACE` | extra nospace characters |
| `CARAPACE_MERGEFLAGS` | merge flags of the whole chain (unset: zsh merges implicitly) |
| `CARAPACE_UNFILTERED` | show hidden/filtered entries |
| `CARAPACE_LENIENT` | fallback positional completion despite parse errors |
| `CARAPACE_HIDDEN` | `1` excludes carapace's own entry, `2` includes it |
| `CARAPACE_TOOLTIP` | tooltips (default on; empty/`0` disables) |
| `CARAPACE_DESCRIPTION_LENGTH` | trim descriptions (default 80) |
| `CARAPACE_MATCH` | `CASE_INSENSITIVE` or `1` |
| `CARAPACE_EXPERIMENTAL` | tabdance uids (also requires `tabdance` on `PATH`) |
| `CARAPACE_COMPLINE` | raw compline for zsh capture / cmd-clink re-lexing |
| `CARAPACE_ZSH_HASH_DIRS` | zsh snippet hash dirs |

Shell-provided variables (`COMP_LINE`, `COMP_TYPE`, `COMP_WORDBREAKS`) are read process-side by the entry point, then travel on the context.

## Not ported from Go

CompleteEnv-style activation, the choices/`CARAPACE_BRIDGES` system, MCP server, man pages, and scraping — bridging is explicit wiring via `carapace-rs-bridge` (see [bridges.md](bridges.md)).
