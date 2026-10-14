# YAML Specs

Declaring completions in YAML with `carapace-rs-spec` — parsing, dumping, and the `_carapace spec` protocol.

> **Source of truth**: `carapace-rs-spec/src/`. The format matches carapace-bin's spec schema; for the full format reference and schema semantics, see the **carapace** skill → `references/spec.md`.

## Parsing and dumping

```rust
let command: carapace_rs::model::Command = carapace_rs_spec::from_yaml(yaml)?;  // Result<_, String>
let yaml = carapace_rs_spec::to_yaml(&command);   // round-trip; emits the schema header
```

Parsing is strict (`deny_unknown_fields`). `to_yaml` output carries a `# yaml-language-server: $schema=…command.json` header.

## Usage with a clap app

`carapace-rs-clap`'s default-on `spec` feature exposes the tree dump:

```bash
mybin _carapace spec     # YAML dump of the clap command tree
```

External tools (carapace-bin, tabdance) consume the dump; in-process, `from_yaml` produces a `model::Command` usable with the same engine.

## Format overview

```yaml
name: mycmd              # may embed usage after a space
aliases: [myc]
description: My command description
group: main              # drives tag/highlight styles
hidden: false
parsing: interspersed    # interspersed | non-interspersed | disabled
flags:
  -v, --verbose: be verbose            # boolean flag
  -f, --file=: input file              # takes a value
  -o, --output?: optional output       # optarg: value only via `=` or next arg per style
  --tag*: repeatable                   # repeatable
  --hidden&: hidden flag
  --required!: required flag
  --two=: {description: two args, nargs: 2}   # nargs: N → exact, -1 → any, <-1 → at least one
  extra, --alias=: shorthand-only with alias
persistentflags:
  -p, --persistent=: inherited by all subcommands
commands:
  - name: sub
    description: subcommand
    flags:
      -x: exclusive flag
    completion:
      positional:
        - ["$directories"]
      positionalany: ["$files"]
completion:
  flag:
    file: ["$files"]                    # by long name
    verbose: ["true", "false"]
  positional:
    - ["first\tdescribed", "second"]    # value<TAB>description[<TAB>style]
  positionalany: ["$files"]
  dash: []
  dashany: []
```

Flag keys are comma-separated name lists; the suffix on the **last** name applies to the flag (`=`, `?`, `*`, `&`, `!`). Extra shorthands become aliases. Macro entries resolve through the macro registry — see [macro.md](macro.md).

## Environment substitution

During spec invocation, the context exports the classified request for template use:

| Variable | Content |
|---|---|
| `C_ARG<n>` | the n-th argument (0-based) |
| `C_VALUE` | the value being completed |
| `C_FLAG_<NAME>` | flag presence/values (lowercase name, uppercased) |

Substitution errors degrade to message actions (visible, non-fatal).

## Custom macros

Register in-process macros for spec authors:

```rust
// name embeds the executable prefix → $myexe.greet(args) in specs
carapace_rs_spec::add_macro(
    "myexe.greet",
    "greets the user",
    "$myexe.greet()",
    |args: &[String], value: &str| /* -> carapace_rs::Action */,
);
```

They resolve as `$myexe.greet(args)` / `$carapace.greet(args)`, and external spec authors invoke them over the `_carapace macro` protocol — see [macro.md](macro.md).

## Related

- Macro formats and modifier chaining → [macro.md](macro.md)
- Snippet/spec entry dispatch → [setup.md](setup.md)
