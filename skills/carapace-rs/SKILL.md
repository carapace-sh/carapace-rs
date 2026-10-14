---
name: carapace-rs
description: >
  Use when working with carapace-rs — shell completion for Rust CLIs with clap:
  integrating carapace into clap apps, the Action API in Rust, YAML specs, bridges,
  shell snippets, or testing completions with the testkit. Triggers on: "carapace-rs",
  "carapace_rs", "clap completion", "Carapace trait", "flag_completion",
  "positional_any_completion", "dash_any_completion", "completer!", "command_completer!",
  "ArgCompleter", "_carapace", "carapace-rs-spec", "carapace-rs-bridge",
  "carapace-rs-testkit", "Sandbox", "Action::values", "Action::multiparts_n",
  "CARAPACE_LENIENT", "CARAPACE_HIDDEN".
user-invocable: true
---

# carapace-rs Usage Reference

Reference for using [carapace-rs](https://github.com/carapace-sh/carapace-rs) — full-featured dynamic shell completion for [clap](https://github.com/clap-rs/clap)-based Rust CLIs, ported 1:1 from Go carapace.

## Data Flow

```
shell → `example _carapace <shell> <cmd> <words…>`
  → carapace_rs_clap::complete() entry
      ├─ "_carapace spec"            → YAML dump of the command tree
      ├─ "_carapace macro …"         → external macro protocol
      ├─ "_carapace style set …"     → persist to styles.json
      ├─ 1–2 args                    → shell snippet output
      └─ completion request          → to_model()  → lenient traverse
                                        → actions invoke (cache, exec, batch)
                                        → per-shell formatter → stdout
  → None (not a completion request) → normal clap execution (get_matches)
```

## Sub-Resources

Load the reference that matches your task. When in doubt, load multiple references.

| Keywords | Reference |
|----------|----------|
| Integrate carapace, clap, Carapace trait, flag_completion, positional_completion, dash_completion, completion_group, disable_flag_parsing, use_line, pre_invoke, completer!, command_completer!, ArgCompleter, ArgExt, CommandExt, complete(), snippet, derive, extension slots | [references/integrate.md](references/integrate.md) |
| Action, ActionValues, ActionCallback, ActionExecCommand, modifiers, batch, caching, timeout, MultiParts, MultiPartsN, split, split_p, tags, styles, naming, InvokedAction | [references/action.md](references/action.md) |
| YAML spec, user spec, carapace-rs-spec, from_yaml, to_yaml, spec dump, parsing modes, flags, subcommands, completion section, custom macros, add_macro | [references/spec.md](references/spec.md) |
| Macros, $files, $directories, $executables, $spec, shell macros, external macros, modifier chaining, `|||` | [references/macro.md](references/macro.md) |
| Bridges, carapace-bin, cobra, fish, bash, zsh, clap, dynamic router, empty command, executables, delegation | [references/bridges.md](references/bridges.md) |
| Setup, shell integration, snippets, styles.json, style set, CARAPACE_LOG, CARAPACE_LENIENT, CARAPACE_HIDDEN, CARAPACE_MERGEFLAGS, environment variables | [references/setup.md](references/setup.md) |
| Testing, Sandbox, testkit, mocked replies, export JSON, expect, golden snippets, fixtures | [references/testing.md](references/testing.md) |

## Quick Guide

- **How do I add carapace-rs to my clap CLI?** → [references/integrate.md](references/integrate.md)
- **How do I create a custom completion action?** → [references/action.md](references/action.md)
- **How do I attach a completer in clap derive?** → [references/integrate.md](references/integrate.md)
- **How do I write a YAML spec and use it with a clap app?** → [references/spec.md](references/spec.md)
- **Which macros can a spec entry use?** → [references/macro.md](references/macro.md)
- **How do I complete arguments of an external command?** → [references/bridges.md](references/bridges.md)
- **How do I register the shell snippet for my binary?** → [references/setup.md](references/setup.md)
- **How do I test completions?** → [references/testing.md](references/testing.md)

## Cross-Project References

- For **semantics shared with Go carapace** (modifier behavior, spec format details, macro signature syntax, environment variable effects), use the **carapace** skill — this skill documents the Rust API surface and Rust-specific differences only.
- For **internal library development** (Action internals, traverse engine, shell formatters, style system), use the **carapace-dev** skill and the repo's `AGENTS.md`.
- For **cargo/crate topics** (features, publishing, workspaces), use the **cargo** skill.
