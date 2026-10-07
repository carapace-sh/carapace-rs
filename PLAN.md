# carapace-lib-rs — Rust port of carapace with clap as base

Port of [carapace-sh/carapace](https://github.com/carapace-sh/carapace) (Go) to Rust.
**Key architectural change:** cobra/pflag(-fork) is replaced by `clap` v4 as the command-tree
source of truth. Everything above the framework adapter (Action API, traverse engine, shell
formatters, styles, export format) is ported 1:1 in spirit; everything below is a clap adapter.

## Goals

- Full-featured dynamic shell completion for clap-based Rust CLIs: bash, bash-ble, oil,
  cmd-clink, elvish, fish, ion, nushell, powershell, tcsh, xonsh, zsh.
- Keep the carapace action model (`Action` → `Invoke` → `InvokedAction`) and modifier naming
  (`Filter`, `Prefix`, `Tag`, `MultiParts`, …) so specs/macros/bridges port mechanically.
- Same `CARAPACE_*` env vars, `styles.json`, and export JSON wire format for ecosystem
  compatibility (bridge, tabdance, spec).
- Non-goals for v1: YAML spec crate, bridge crate, man pages — phases 2+.

## Why clap as base changes things

| Go carapace | Rust port |
|---|---|
| `cobra.Command` tree | `clap::Command` tree (`cmd.build()` first) |
| `pflag` + `pflagfork` reflection hacks | clap public API (`Arg`, `get_num_args()`, `require_equals`, `ArgAction`, `Arg::get`) |
| Global `storage` map keyed by command path | **clap extension slots** (`unstable-ext` `ArgExt`/`CommandExt`) holding completers on the args themselves |
| `DisableFlagParsing`, `FParseErrWhitelist` | custom lenient tokenizer over the internal model (clap's own parser is not reused for completion) |
| `bridge.ActionClap` (spawns binary, unstable-dynamic) | superseded: in-process completers, richer than clap_complete's engine |
| `CompleteEnv`-style registration | same idea, carapace snippets instead |

The internal normalized model is **framework-agnostic** (`carapace-core::model::Command`) so a
spec/YAML backend or other adapters can plug in later; `carapace-clap` is the primary adapter.

## Workspace layout

```
carapace-lib-rs/
├── carapace-core/        # framework-independent: model, action, traverse, shell, style, cache, uid, export
├── carapace-clap/        # clap adapter: Command → model, extension API, integration entry points
├── carapace-macros/      # (phase 3) optional derive/attribute sugar
├── carapace-testkit/     # sandbox harness (port of carapace/sandbox) + golden tests
├── carapace-spec/        # YAML specs as command trees
├── carapace-bridge/      # external completer bridges (fish, bash, zsh, carapace, clap)
└── examples/             # demo CLI used by sandbox tests
```

Dependencies: `clap` (feature `unstable-ext`, adapter only), `shlex` (bash/clink patching),
`anstyle`/`anstyle-parse` (styles), `dirs` (cache paths), `serde`/`serde_json` (export),
`regex`, `rayon` (Batch). MSRV: current stable, edition 2024.

## carapace-core

### `model::Command` (normalized, owned)

Built once per completion request by the adapter from `clap::Command` (after `build()`):

```rust
pub struct Command {
    pub name: String,
    pub aliases: Vec<String>,
    pub flags: Vec<Flag>,          // merged local+persistent, with source info
    pub positionals: Vec<Positional>,
    pub subcommands: Vec<Command>, // recursive
    pub interspersed: bool,
    pub flag_prefix: char,         // '-' default; keep hook for custom prefixes
}
pub struct Flag {
    pub id: String,                // long or short id (clap Arg id)
    pub long: Option<String>,
    pub short: Option<char>,
    pub takes_value: bool,         // false for SetTrue/Count/Help/Version
    pub nargs: Nargs,              // from clap num_args: fixed(n), variable(0..=1 → optarg), etc.
    pub optional: bool,            // NoOptDefVal analog: num_args range includes 0 while takes_value
    pub argument_style: ArgumentStyle, // AcceptsNext | AcceptsDelimited | AcceptsAttached
                                       // derived from require_equals / allow_hyphen_values /
                                       // value_terminator; default = all three (like pflagfork)
    pub optarg_delimiter: char,    // '=' default (from require_equals semantics)
    pub repeatable: bool,          // Append actions / multiple occurrences
    pub required: bool,
    pub hidden: bool,
    pub mutually_exclusive: Vec<Vec<String>>, // clap conflicts_with groups
    pub meta: FlagMeta,            // style/tag/description for flag-name completion
}
```

This replaces pflagfork's reflection reads; the `LookupArg` dispatch, `Consumes()`,
`ArgumentStyle` bitmask and delimiter logic port directly against these fields.

### `Action` (lazy) / `InvokedAction` (materialized)

```rust
pub enum ActionKind {
    RawValues(Vec<RawValue>),
    Callback(ActionCallback),                       // Arc<dyn Fn(&Context) -> Action + Send + Sync>
}
pub struct Action { meta: Meta, kind: ActionKind }
pub struct InvokedAction { meta: Meta, values: RawValues }
```

- Modifiers chain on `Action` and mostly wrap in `Callback`, mirroring Go semantics exactly:
  `Filter`/`FilterArgs`/`FilterParts`/`Retain`/`Unique`, `Prefix`/`Suffix`/`NoSpace`/`NoPrefix`,
  `MultiParts[NP]`/`List`/`UniqueList[F]`/`Split[P]`, `Style`/`StyleF`/`StyleR`, `Tag`/`TagF`,
  `Chdir[F]`, `Shift`, `Cache`, `Timeout`, `Unless[F]`, `Suppress`, `Usage[F]`, `Uid[F]`, `Query[F]`.
- Constructors: `ActionValues`, `ActionValuesDescribed`, `ActionStyledValues(Described)`,
  `ActionMessage`, `ActionCallback`, `ActionDirectories`, `ActionFiles`, `ActionMultiParts[N]`,
  `ActionExecCommand[E]` (curried via closure-returning fn), `ActionCommands`,
  `ActionPositional`, `ActionExecute`, `ActionImport`, `ActionExecutables`, `ActionStyles`.
- `Meta`: messages, nospace set, usage, queries, tags per value — mirrors `common.Meta`.
- `Batch` (parallel invocation) via rayon over `Arc<Context>`; `InvokedAction::Merge` for join.
- Suffix naming convention (`F`/`P`/`E`/`N`) preserved.

### `Context`

Same shape as Go: `value`, `args`, `parts`, `env` (HashMap), `dir`. Methods
`lookup_env`/`get_env`/`set_env`/`envsubst`/`command`/`abs`. No `os.Environ` global reads
inside actions — context owns the environment.

### `traverse` (completion engine)

Direct port of the Go state machine over `model::Command`:

- Classification order: flag-argument (via `Flag::consumes`) → `--` dash → flag → subcommand →
  positional; subcommand recursion parses accumulated args leniently (our own tokenizer, never
  errors on unknown flags; `CARAPACE_LENIENT` is always effectively on plus a strictness toggle).
- `LookupArg` dispatch (longhand/shorthand/POSIX/non-POSIX, longest ArgPrefix wins,
  delimiter-disabled optargs) ported against `Flag` fields above.
- Completion decision table (flag arg / optional arg / attached arg / flag names /
  positional + subcommands batch) identical.
- Shell patching before traverse: `bash` (re-lex `COMP_LINE`, strip redirects, wordbreak prefix,
  COMP_TYPE), `nushell` (strip quotes), `clink` (re-lex `CARAPACE_COMPLINE`) — `shlex` crate.

### Shell formatters

`carapace_core::shell` with a `Shell` trait (`snippet(cmd)`, `value(meta, values)`) and one
module per shell, porting output formats, quoting state machines, nospace/message handling and
patching behavior per the cross-shell comparison (zsh 5-state quoting machine, bash
`\x1c`-separated, elvish/nushell/xonsh JSON, …). `shell::Value()` shared dispatch implements the
7 post-processing steps (color disable, prefix filter, flag merging, message integration,
nospace propagation, sort, uid clearance). Export JSON (serde structs, versioned) included here.

### Style / config / cache / uid

- `style`: XTerm256+TrueColor parse/format on top of `anstyle`; `LS_COLORS` path styling.
- `config`: `styles.json` load, `CARAPACE_*` env vars (`LOG`, `COLOR`, `NOSPACE`,
  `MERGEFLAGS`, `UNFILTERED`, `LENIENT`, `EXPERIMENTAL`, `SANDBOX`), global state via
  `OnceLock` only.
- `cache`: disk cache (`dirs::cache_dir()/carapace`), TTL, key types (`file`, `line`, custom),
  `Action::Cache` wrapper; export-JSON as cached payload.
- `uid`: `cmd://` scheme URL building, `UidF`/`QueryF` hooks.

## carapace-clap (adapter)

### Registration via extensions (no global map)

```rust
use carapace_clap::prelude::*;

let cmd = Command::new("myapp")
    .arg(Arg::new("profile")
        .add(ArgCompleter::new(ActionValues("dev", "prod"))))   // ArgExt (unstable-ext)
    .subcommand(Command::new("build").arg(...));

carapace_clap::complete(cmd_factory).unwrap();   // like CompleteEnv: handles CARAPACE/snippet mode
```

- `ArgCompleter` / `CommandCompleter` wrapper types hold `Action`s; `get::<ArgCompleter>()`
  during model extraction routes to `storage.getFlag`/`getPositional` equivalents.
- Defaults when no extension: `PossibleValue`s from the value parser → values;
  `ValueHint` (`AnyPath`/`FilePath`/`DirPath`) → `ActionFiles`/`ActionDirectories`.
- Flag-name completion, descriptions, tags (shorthand/longhand flags) generated from clap
  metadata automatically.

### Entry points (mirror Go `carapace.Gen(cmd)`)

- `carapace_clap::Gen(&mut Command)` → builder: `.Standalone()` (injects hidden `_carapace`
  subcommand; rewrites snippets), `.PositionalCompletion(i, action)`,
  `.PositionalAnyCompletion(action)`, `.FlagCompletion(name, action)`,
  `.PreInvoke(fn)`, `.PreRun(fn)`, `.SnippetCustomizer(...)`.
- `complete(factory)`: env-var dispatch — no `COMPLETE`/`CARAPACE_<SHELL>` → normal run;
  no args → print snippet; args → patch + traverse + invoke + format.
- `_carapace` CLI port: `spec`, `style`, `export`, multi-completer re-invocation for
  binaries embedding several commands.

### Derive story (phase 3)

Leverage clap derive's `add = ...` escape hatch (as clap_complete does) plus an optional
`carapace_macros::completer(...)` attribute for terser wiring.

## carapace-testkit

Port of `sandbox`: spawn the example binary in a sandboxed shell session per target shell,
send compline, capture output, assert. Use `completest`-style PTY harness only behind a
test feature; keep core no-PTY. Golden tests per shell for snippets and completions.

## carapace-bridge

Port of `carapace-bridge/pkg/actions/bridge`: wrap completions provided by
external frameworks as regular carapace [`Action`]s.

- `bridge::action(name, command)` dispatches by name; each bridge goes through
  `action_command`, which — when no command is given — completes the first
  positional with `executables + files` and delegates shifted afterwards.
- `fish`: `fish --no-config --command` snippet sourcing
  `~/.config/carapace/bridge/fish/config.fish`, output of `complete
  --do-complete` parsed as `value\tdescription`.
- `bash` (`shell::bash`): interactive `bash --rcfile` with
  `~/.config/carapace/bridge/bash/.bashrc`, runs the vendored `bash.sh`
  snippet via `COMP_LINE`, parses `value(description)` lines.
- `zsh`: zpty-based completion capture (vendored
  `Valodim/zsh-capture-completion`) with
  `~/.config/carapace/bridge/zsh/.zshrc`, output unquoted/ANSI-stripped and
  split on ` -- ` for descriptions.
- `carapace` / `carapace-bin`: invoke the target's `_carapace export` (or the
  `carapace` binary) and `ActionImport` the export JSON.
- `clap`: invoke the target's clap dynamic `complete` CLI
  (`--index/--type/--no-space/--ifs`) and mirror its nospace heuristic.

## Milestones

| # | Scope | Status |
|---|---|---|
| M0 | Workspace, `model`, `Context`, `Action` core + modifiers, `Meta` | done |
| M1 | `carapace-clap`: `Command → model` extraction, `ArgCompleter` extensions, `Gen` builder | done |
| M2 | `traverse`: classifier, lenient parser, `LookupArg`, dash/optarg/attached edge cases | done |
| M3 | `export` JSON + bash formatter + snippet + bash patching; `_carapace` entry | done |
| M4 | zsh, fish, elvish, nushell formatters; styles.json/config | done |
| M5 | powershell, xonsh, tcsh, ion, oil, bash-ble; `Batch`, `Timeout` | done (std threads, not rayon) |
| M6 | `exec_command`/`_e`, `styles`/`style_config`, snippet modes + `$SHELL` detect | done |
| M6.5 | UIDs/queries, `ActionImport`/`ActionExecute` + `execute_embedded` multi-completer, `_carapace style set`, `_carapace spec` dump | done |
| P2 | sandbox testkit (in-process harness), spec crate (YAML), bridge crate | done; derive sugar open |

## Risks / decisions

- **clap parser not reused for traversal** — clap's parser can't do carapace's lenient
  classification (unknown flags, optargs, custom prefixes); we extract metadata and implement
  our own tokenizer. Cost: dual-maintenance of parse semantics mapping (mitigated by the
  `Flag` field mapping table + tests).
- **`unstable-ext`** for arg extensions; pin minor clap versions, gate with a fallback
  side-map keyed by arg id if the API churns.
- **Closures in actions** need `Send + Sync` (Batch spawns threads); document that completers
  run in parallel and must not share unsynchronized state; `Context` is owned per invocation.
- **Error handling**: internal `Error` type, no panics across the API; `ActionMessage` for
  user-visible failures.
- **OS support**: Windows (powershell/cmd-clink) from day one — no cobra/pty assumptions.
