# AGENTS.md

Guide for AI agents working in the `carapace-rs` repository.

## Project Overview

Rust port of [carapace](https://github.com/carapace-sh/carapace) (Go) — full-featured
dynamic shell completion — with [clap](https://github.com/clap-rs/clap) as the command-tree
base. Supported shells: bash, bash-ble, oil, cmd-clink, elvish, fish, ion, nushell,
powershell, tcsh, xonsh, zsh, plus the `export` JSON wire format.

**carapace (Go) is the source of truth.** Local checkouts of the reference repositories
live next to this one and should be consulted when porting or verifying behavior:

| Repository | Content |
|---|---|
| `../carapace` | core library: `action.go`, `traverse.go`, `complete.go`, `internal/shell/*`, `internal/pflagfork`, `pkg/*` |
| `../carapace-bridge` | bridge reference: `pkg/actions/bridge/*.go` |
| `../carapace-shlex` | shell lexing reference: `completion.go` (Complete, quoting states, wordbreaks) |
| `../carapace-spec` | YAML spec format and core macros |

## Repository Naming Taxonomy

`carapace-rs` is the Rust port; language ports are a reserved family. Keep tokens
atomic behind the `-` delimiter and follow these categories:

| Pattern | Category | Examples |
|---|---|---|
| `carapace-<tool>` | completion specs for a tool (full tool name) | `carapace-aws`, `carapace-python` |
| `carapace-spec-<framework>` | spec adapters | `carapace-spec-clap`, `carapace-spec-kong` |
| `carapace-lex-<name>` | lexers | `carapace-lex-bash` |
| `carapace-<lang>` | language ports (reserved) | `carapace-rs` (this repo), `carapace-py` |

Language codes (`rs`, `py`, ...) are reserved suffixes; tool specs always use the
full tool name so they never compete for those tokens.

Rule of thumb: port behavior 1:1 in spirit (same semantics, same edge cases, same wire
format), adapt the *shape* to idiomatic Rust. When Go and Rust behavior diverge in a
test, either the port or the test is wrong — check the Go source first.

## Build & Test Commands

```bash
cargo test --workspace        # 163 tests across 15 suites; must pass
cargo fmt                     # format; cargo fmt --check must be clean
cargo clippy --workspace      # must be warning-free
```

Run the demo binary to try completions end to end:

```bash
cargo run -p carapace-clap --example demo -- _carapace bash --profile ""
cargo run -p carapace-clap --example demo -- _carapace spec
```

## Workspace Layout

```
carapace-core/    framework-independent engine: model, action, traverse, shell formatters,
                  cache, uid, export, mock
carapace-clap/    clap adapter: Carapace extension trait, extension completers, _carapace entry point
carapace-macros/  completer!/command_completer! expression wrappers for clap derive
carapace-spec/    YAML specs: from_yaml parsing, to_yaml dump, $-macro resolution
carapace-bridge/  external completer bridges: fish, bash, zsh, carapace, carapace-bin,
                  clap, cobra, inshellisense
carapace-testkit/ in-process sandbox harness for tests
```

## Design Decisions

These are settled; don't relitigate without cause, and preserve them when refactoring.

1. **clap extension slots instead of Go's global storage map.** Completers are attached
   to args via `unstable-ext` (`ArgCompleter` implements `ArgExt`, same pattern as
   clap_complete's `ArgValueCompleter`). This removes the global mutable state that
   forces Go's sandbox to run tests in a subprocess.
2. **`Carapace` extension trait instead of a wrapper type.** Go's `Gen(cmd)` wraps
   commands because Go cannot add methods to foreign types; here `impl Carapace for
   clap::Command` provides the same chaining without a wrapper or `into_command()`.
   Do not name completion traits `CommandExt`/`ArgExt` — those are clap's
   payload-marker traits (opposite impl direction).
13. **Framework-agnostic model as the seam.** The engine (`carapace_core::traverse`)
   never sees framework types; adapters convert into `model::Command` once per request.
   The spec backend plugs in through the same seam. Do not let clap types leak into
   `carapace-core`.
13. **Own lenient traversal, never clap's parser.** clap's parser errors on unknown
   flags and cannot express optarg/custom-prefix semantics; carapace's completion
   needs lenient classification. The `Flag` model fields (`nargs`, `optional`,
   `argument_style`, `allow_hyphen_values`, `optarg_delimiter`) are the clap-native
   replacement for pflagfork's reflection.
13. **`LookupArg` port semantics matter.** `arg_prefix` is the flag name *including the
   delimiter* (`--profile=`), not the full argument — re-attachment via
   `Action::prefix(arg_prefix)` corrupts candidates otherwise (fixed in `122762a`).
13. **Bash patch context flows explicitly.** `complete_output` keeps the
   `bash::Patch::Patched` result in a local and passes `bash::Context` to the
   formatter; no thread-locals or hidden channels. `patch()` re-lexes `COMP_LINE`,
   drops redirect tokens and the binary name (Go's `args[2:]`), and unsets the COMP_*
   environment (consume-once).
13. **No `CARAPACE_SANDBOX`.** Go's env var is a subprocess handoff; the Rust sandbox is
   in-process and the `Mock` (replies + cache root) travels on the `Context`
   (`Context::mock`). Don't reintroduce process-global env state for tests.
13. **Context owns the environment.** Actions read environment through
   `Context::lookup_env`/`get_env`, never `std::env::var` (the plan's stated rule).
   Bridge config dirs, for example, resolve from the context. Process env reads are
   acceptable only in entry-point code (before a `Context` exists) and for genuinely
   shell-provided variables (`COMP_*`, `CARAPACE_COMPLINE`).
13. **One conflated type became two.** Go's `Action` conflates lazy and materialized;
   here `Action::invoke(&Context) -> InvokedAction` splits them. Modifiers compose via
   `map_invoked`.
13. **`ActionMessage`, not `Result`, for failures.** Completion is best-effort; a
   failing completer degrades to a visible message (Go semantics).
13. **Std threads, not rayon, for `Batch`.** Batches are small; the dependency is not
    justified. Closures must be `Send + Sync` and must not share unsynchronized state.
13. **No CompleteEnv-style activation.** The `_carapace` snippet protocol is the
    ecosystem interface (carapace-bin, tabdance, specs); env-activated completion
    would bypass COMP_* patching and bifurcate the protocol.
13. **Wire format is frozen.** `export::Export` JSON (version 0.1.0), `styles.json`,
    and the remaining `CARAPACE_*` env vars (`LOG`, `COLOR`, `NOSPACE`, `MERGEFLAGS`,
    `UNFILTERED`, `LENIENT`, `EXPERIMENTAL`) match Go byte-for-byte where consumed by
    external tooling.

## Naming

Go CamelCase maps to Rust snake_case with the prefix/suffix conventions preserved:
`ActionValuesDescribed` → `Action::values_described`; the `F`/`P`/`E`/`N` suffixes
(`UidF`, `MultiPartsN`, `ExecCommandE`) are kept as `_f`/`_n`/`_e`. Keep modifier
names aligned with Go so specs/macros port mechanically.

## Testing

- Use the `carapace-testkit` `Sandbox` for completion behavior: `files`, `reply`
  (mocked command output), `env`, `run(...).expect(Action)`. Assertions compare export
  JSON.
- The `Sandbox` holds no process-global state; tests run in parallel. Any test that
  mutates process environment (`COMP_*`, `CARAPACE_*`, `SHELL`, `XDG_*`) must hold its
  binary's `env_lock()` for the whole test — unlocked env mutation has caused flaky
  races repeatedly (e.g. `patch()` unsetting `COMP_*` mid-run of a parallel test).
- External tools in tests (fish, zsh, bash) are exercised through real binaries or
  mocked replies; never assume a tool is installed without a guard.
- Flag-value completers return **bare values** (`"dev"`); the attached form
  (`--profile=dev`) is reconstructed by `Action::prefix(arg_prefix)` in the traverse
  flag branch.

## Gotchas

- fish 4.x splits `complete -a` arguments on whitespace; a `\t` escape inside the
  argument separates value from description. Sandbox tests must set `-f` (no file
  fallback) for deterministic candidate lists.
- zsh capture output needs compadd backslash unquoting and ANSI stripping before
  parsing; lines are `\r\n`-terminated and split on ` -- ` for descriptions.
- fish `set __fish_config_dir` fails on fish ≥ 4 (read-only); the bridge sources the
  custom `config.fish` directly instead.
- clap derive's `add = <expr>` escape hatch forwards to builder methods — the
  mechanism `carapace-macros` builds on; the generated code references
  `carapace_clap::` directly, so users need that dependency too.
- `_carapace` entry handles: snippet output (0/1 args), completion (2+ args), `style
  set`, and `spec` (YAML dump). Unknown shells return `None` so the program runs
  normally.
