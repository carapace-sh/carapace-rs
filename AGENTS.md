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
| `../carapace-shlex` | shell lexing reference: `completion.go`, `shlex.go`, `quote.go`, `format_*.go` — the completion core is ported as the `carapace-rs-shlex` crate (format tests 1:1; `verify_*.go` source-crosscheck corpora are ported in `src/verify.rs`, fish/zsh/xonsh/powershell sections condensed) |
| `../carapace-spec` | YAML spec format and core macros |

Rule of thumb: port behavior 1:1 in spirit (same semantics, same edge cases, same wire
format), adapt the *shape* to idiomatic Rust. When Go and Rust behavior diverge in a
test, either the port or the test is wrong — check the Go source first.

## Build & Test Commands

```bash
cargo test --workspace        # must pass
cargo fmt                     # format; cargo fmt --check must be clean
cargo clippy --workspace      # must be warning-free
```

Run the example binary to try completions end to end:

```bash
cargo run -p carapace-rs-clap --example example -- _carapace bash example --profile ""
cargo run -p carapace-rs-clap --example example -- _carapace spec
```

## Workspace Layout

```
carapace-rs-shlex/   the ported carapace-shlex completion core: per-format lexing
                  (12-state machine, quoting states, wordbreak/operator
                  classification, spans), Complete() context (words, current
                  word, wordbreak prefix, redirect/variable/substitution
                  detection), Quote/Join per format
carapace-rs/         framework-independent engine: model, action, traverse, shell formatters,
                  cache, uid, export, mock
carapace-rs-clap/    clap adapter: Carapace extension trait, extension completers, _carapace entry point,
                  completer!/command_completer! macro_rules! wrappers for clap derive
carapace-rs-spec/    YAML specs: from_yaml parsing, to_yaml dump, $-macro resolution
carapace-rs-bridge/  external completer bridges: fish, bash, zsh, carapace, carapace-bin,
                  clap, cobra, inshellisense
carapace-rs-testkit/ in-process sandbox harness for tests
```

## Example Command

`carapace-rs-clap/examples/example/` ports the Go repo's `example/cmd` tree —
a deliberately wide command tree (groups, persistent flags, optargs, embedded
commands, shorthand chains, disabled flag parsing, mutually exclusive flags,
every action modifier) serving as the end-to-end regression fixture. Its
sandbox tests live in `carapace-rs-clap/tests/example.rs`, one topic module
per Go test file. Cobra-only surfaces are adapted: `Standalone` hides the
help flag, the `PreRun` plugin command is attached statically, the root
`PreInvoke` hook is available as the `pre_invoke` extension (Go
`Gen(cmd).PreInvoke`) and `StyleR` as `Action::style_r`. The `compat`
topic (cobra's `RegisterFlagCompletionFunc`/`ValidArgsFunction` directives)
stays a bare command because the directive semantics are covered by the
cobra bridge tests in `carapace-rs-bridge`. Deliberately unported (no Go
test coverage, no clap-native equivalent): the `flag` command's `--optarg`
`PreRun` toggling of `NoOptDefVal`. Snippet golden fixtures live in
`carapace-rs-clap/examples/example/_test/` (including the export shell's
command-structure JSON) and are asserted by `tests/example_snippets.rs`;
regenerate them via the example binary.

Deferred: the multi-completer port (Go `example-multi` + `WithSubcommands`/
`WithDefault` + the ten `SnippetMulti` shell templates; cmd-clink and ion
unsupported upstream). Completion itself is covered in-process via the
`execute_embedded` multi-completer pattern; only the routing snippets and
the embedder-facing dispatch API are missing. Port order when picking it
up: routing API first, then templates shell-by-shell starting with
bash/zsh, then the `example-multi/cmd/_test/` fixtures.

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
3. **No non-POSIX flag modes.** Go's delimiter machinery (`OptargDelimiter`,
    delimiter-disabled optargs accepting attached values) serves third-party
    binaries with non-POSIX pflag, completed via carapace-bin. This engine only
    completes clap trees and YAML specs — both POSIX-only (`=` delimiter,
    attached/delimited/next styles via `ArgumentStyle`) — so the machinery is
    not ported; non-clap tools are delegated to their own completers by the
    bridges.
4. **Framework-agnostic model as the seam.** The engine (`carapace_rs::traverse`)
   never sees framework types; adapters convert into `model::Command` once per request.
   The spec backend plugs in through the same seam. Do not let clap types leak into
   `carapace-rs`.
5. **Own lenient traversal, never clap's parser.** clap's parser errors on unknown
   flags and cannot express optarg/custom-prefix semantics; carapace's completion
   needs lenient classification. The `Flag` model fields (`nargs`, `optional`,
   `argument_style`, `allow_hyphen_values`, `optarg_delimiter`) are the clap-native
   replacement for pflagfork's reflection.
6. **`LookupArg` port semantics matter.** `arg_prefix` is the flag name *including the
   delimiter* (`--profile=`), not the full argument — re-attachment via
   `Action::prefix(arg_prefix)` corrupts candidates otherwise (fixed in `122762a`).
7. **Bash patch context flows explicitly.** `complete_output` keeps the
   `bash::Patch::Patched` result in a local and passes `bash::Context` to the
   formatter; no thread-locals or hidden channels. `patch()` re-lexes `COMP_LINE`,
   drops redirect tokens and the binary name (Go's `args[2:]`), and unsets the COMP_*
   environment (consume-once). A trailing partial token (open quote or trailing
   escape) is recovered leniently — the shlex crate drops it, which would lose the
   word the cursor sits in; quoted whitespace stays within the word.
8. **No `CARAPACE_SANDBOX`.** Go's env var is a subprocess handoff; the Rust sandbox is
   in-process and the `Mock` (replies + cache root) travels on the `Context`
   (`Context::mock`). Don't reintroduce process-global env state for tests.
9. **Context owns the environment.** Actions read environment through
   `Context::lookup_env`/`get_env`, never `std::env::var` (the plan's stated rule).
   Bridge config dirs, for example, resolve from the context. Process env reads are
   acceptable only in entry-point code (before a `Context` exists) and for genuinely
   shell-provided variables (`COMP_*`, `CARAPACE_COMPLINE`). The `CARAPACE_*`
   completion flags ride the `Context` as `config::Flags` (parsed from the context
   env map, re-derived at every traverse level from the merged environment);
   `Shell::value_with` takes them as a parameter, `Shell::value` defaults to the
   process environment. Formatters receive the flags through the dispatch
   (description-length trimming, color disable, tooltips, match mode).
   Shell-provided variables (`CARAPACE_COMPLINE`, `CARAPACE_ZSH_HASH_DIRS`,
   `COMP_*`) stay process-side; the `look_path` check for tabdance stays
   process-global.
10. **One conflated type became two.** Go's `Action` conflates lazy and materialized;
   here `Action::invoke(&Context) -> InvokedAction` splits them. Modifiers compose via
   `map_invoked`.
11. **`ActionMessage`, not `Result`, for failures.** Completion is best-effort; a
   failing completer degrades to a visible message (Go semantics).
12. **Std threads, not rayon, for `Batch`.** Batches are small; the dependency is not
    justified. Closures must be `Send + Sync` and must not share unsynchronized state.
13. **No CompleteEnv-style activation.** The `_carapace` snippet protocol is the
    ecosystem interface (carapace-bin, tabdance, specs); env-activated completion
    would bypass COMP_* patching and bifurcate the protocol.
14. **Wire format is frozen — Go's exact shape.** `export::Export` JSON follows Go
    `internal/export`: meta is *flattened* into the top level with lowercase keys
    (Go struct embedding), nospace/noprefix are sorted character *strings* with `*`
    wildcard (Go matcher JSON forms), queries are uid-URL strings, and value keys
    are lowercase. Never reintroduce the nested `"Meta"` wrapper or capitalized
    keys — the carapace bridge cannot read Go carapace-bin output then. The one
    sanctioned extension: optional per-value `nospace` (Go ignores unknown keys)
    so cache round-trips stay faithful. `styles.json` and the `CARAPACE_*` env
    vars (`LOG`, `COLOR`, `NOSPACE`, `MERGEFLAGS`, `UNFILTERED`, `LENIENT`,
    `EXPERIMENTAL`) likewise match Go.
15. **`PATH` splitting is platform-aware** via `std::env::split_paths` (`:` on
    unix, `;` on Windows); likewise executables check the unix permission bits
    or the `.exe` extension.
16. **Per-crate licensing follows upstream provenance.** Crates porting
    Apache-2.0 upstreams (`carapace-shlex` → `carapace-rs-shlex`, `carapace` →
    `carapace-rs`) stay Apache-2.0;
    crates porting MIT upstreams (`carapace-spec` → `carapace-rs-spec`,
    `carapace-bridge` → `carapace-rs-bridge`) and
    original work (`carapace-rs-clap`, `carapace-rs-testkit`) are MIT. `carapace-rs`
    carries the upstream `LICENSE.appendix` reserving `Uid`, `Query`, `Trace`
    for Tabdance™. Do not flatten back to one workspace-wide license.
17. **Core internals are published but not API.** `cache`, `config`, `log`,
    `meta`, `mock`, `uid` mirror Go's `internal/` packages and are
    `#[doc(hidden)]`; they stay reachable for adapters and the test harness but
    are not semver commitments. Do not move stable API into them.
18. **The `_carapace spec` dump is feature-gated.** `carapace-rs-clap` has a
    default-on `spec` feature; without it the YAML machinery (serde_yaml) is
    not compiled and `_carapace spec` behaves like an unknown shell (returns
    `None`). Keep the feature default-on for Go parity.

## Action surface notes

- `Action::split(format)` completes a value that is itself a shell-parsed
  command line: the value is re-lexed, the inner action runs with the
  preceding words as args, and candidates are requoted (open quotes closed
  via the context's `quote()`), spaced unless the inner action claims nospace,
  and the raw prefix re-attached. A `$name`/`${name` current word completes
  environment variable names instead. `split_p` supports pipelines: the
  context is the current pipeline's words with redirects filtered, and a
  redirect target completes files.
- `Action::execute` forwards the outer context (env, dir, mock) into the
  embedded traversal; embedded trees are not separate processes.
- Shell patches at the entry point: bash re-lexes `COMP_LINE` (redirect
  filtering, wordbreak prefix, binary drop), nushell dequotes quoted args,
  cmd-clink re-lexes `CARAPACE_COMPLINE` with cmd rules (`^` escaping, `"`
  quoting). Carapace-shlex lexing is format-aware; its cmd tokenizer is the
  first non-POSIX one.

## Naming

The engine crate is `carapace-rs` (matching the repository name) and workspace
members take it as their family prefix with hyphen separators (`carapace-rs-clap`,
…), following clap's family-prefix model (`clap`, `clap_builder`, …) but keeping
hyphenated spelling — clap's all-underscore form only works because its base
crate is a single word. Cargo derives the underscore Rust identifiers
(`carapace_rs_clap`) from the hyphenated packages; code and doc examples use those.

Go CamelCase maps to Rust snake_case with the prefix/suffix conventions preserved:
`ActionValuesDescribed` → `Action::values_described`; the `F`/`P`/`E`/`N` suffixes
(`UidF`, `MultiPartsN`, `ExecCommandE`) are kept as `_f`/`_n`/`_e`. Keep modifier
names aligned with Go so specs port mechanically.

## Testing

- Use the `carapace-rs-testkit` `Sandbox` for completion behavior: `files`, `reply`
  (mocked command output), `env`, `run(...).expect(Action)`. Assertions compare export
  JSON.
- The `Sandbox` holds no process-global state; tests run in parallel. `CARAPACE_*`
  flags travel on the sandbox context (`sandbox.env(...)`), so no process mutation
  is needed for them. Any test that must mutate process environment (`COMP_*`/
  `CARAPACE_COMPLINE` for entry-point and shell-dispatch tests, `PATH` for the
  tabdance `look_path`, `XDG_*`) must hold its binary's `env_lock()` for the whole
  test — unlocked env mutation has caused flaky races repeatedly (e.g. `patch()`
  unsetting `COMP_*` mid-run of a parallel test).
- External tools in tests (fish, zsh, bash) are exercised through real binaries or
  mocked replies; never assume a tool is installed without a guard.
- Flag-value completers return **bare values** (`"dev"`); the attached form
  (`--profile=dev`) is reconstructed by `Action::prefix(arg_prefix)` in the traverse
  flag branch.

## Gotchas

- Snippets invoke `{binary} _carapace <shell>` with the command name, not
  Go's `uid.Executable()` absolute path (which has no Rust equivalent in
  the snippets; PATH lookup covers registration).
- The bridges delegate word joining/quoting to `carapace_rs_shlex::join` (Go's
  `shlex.Join`); quoting is only applied when the shell needs it, and an empty
  word renders as `""` (the inshellisense workaround strips it).
- fish 4.x splits `complete -a` arguments on whitespace; a `\t` escape inside the
  argument separates value from description. Sandbox tests must set `-f` (no file
  fallback) for deterministic candidate lists.
- zsh capture output needs compadd backslash unquoting and ANSI stripping before
  parsing; lines are `\r\n`-terminated and split on ` -- ` for descriptions.
- fish `set __fish_config_dir` fails on fish ≥ 4 (read-only); the bridge sources the
  custom `config.fish` directly instead.
- clap derive's `add = <expr>` escape hatch forwards to builder methods — the
  mechanism `completer!`/`command_completer!` build on; they are `macro_rules!`
  exported from carapace-rs-clap, so that one dependency covers derive usage.
- `_carapace` entry handles: snippet output (0/1 args), completion (2+ args), `style
  set`, and `spec` (YAML dump, behind the default-on `spec` feature). Unknown
  shells return `None` so the program runs normally.
