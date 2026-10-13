# Review Findings & Fix Plan

From the 2026-10 full-project review. Checked items are fixed (with the commit that
fixed them); the rest is tracked backlog, ordered by priority.

## Design decisions (re-evaluation)

All settled decisions hold; two are strained:

- **DD5 (Flag model)**: `Flag::optional` conflates "optarg binding style" with
  "never consumes the next arg" (`model.rs` `consumes()` short-circuits before
  `nargs`). clap `num_args(0..)` cannot be expressed. Fixed in the adapter mapping.
- **DD9 (context owns env)**: violated by `bridge::config_dir` (`dirs` fallback
  reads process env) and `cache::cache_dir` (`dirs::cache_dir`).

## High priority (correctness)

- [x] clap visible aliases dropped (`get_aliases()` returns hidden-only aliases)
      → use `get_all_aliases()`; test with `.visible_alias`.
- [x] clap `num_args(0..)` flags become non-consuming: `optional` must apply
      only to the `0..=1` optarg shape; `min==0, max>1` maps to variadic
      lookahead (`Nargs::Any`, `optional=false`).
- [x] shlex: line continuation inside a word resets state to `Start`
      (`foo\<LF> bar` merges words, wrong span); Go stays in `IN_WORD_STATE`.
- [x] shlex: block-comment opener `<#` not detected after a wordbreak
      (powershell); trailing words swallowed by a line comment.
- [x] clap `.global(true)` args not inherited by subcommands (only the adapter's
      own `ArgPersistent` marker propagates).
- [x] spec: `--flag?` must take a value (optarg), not be a boolean flag.

## Medium priority

- [x] bridge clap: panic slicing `line[..len-1]` on multi-byte last char
      (`bridge/src/lib.rs:171`).
- [x] bridge `config_dir` falls back to process env via `dirs`; must use the
      context environment (DD9).
- [x] pre-invoke hook nesting order vs docs — Go's `storage.preinvoke`
      nests parents outermost; the wrap loop was inverted. Fixed with a
      nested-hooks test (clap conversion commit).
- [x] spec `to_yaml`: hidden flags (`&`) and flag aliases are dumped again;
      persistent flags get their own `persistentflags:` section. Note: Go's
      `_carapace spec` does not emit `completion:` sections either (actions
      cannot be decompiled) — the earlier review finding was a misdiagnosis.
- [x] spec: persistent flags not persistent — `Flag::persistent` added to the
      model; spec materializes copies on every subcommand (cobra inheritance).
- [x] spec: ` ||| ` modifier separator and the generic modifier set
      (`$nospace`, `$filter`, `$style`, ...) instead of "unsupported macro".
- [x] spec: error visibly on unsupported flag keys (multi-char shorthand)
      instead of silently dropping flags.
- [x] spec: envsubst (`$VAR`/`${VAR}`) in entries; `C_ARGn`/`C_VALUE` exports.
      Repeatable flags now accumulate their values across occurrences (pflag
      slice semantics) so `C_FLAG_<NAME>` joins them with `,`.
- [x] bridge: cobra directive-error paths match Go (`ActionValues()` on
      unparseable directive and ShellCompDirectiveError).
- [x] bridge: fish/zsh/bash/inshellisense missing `style.ForPath` styling
      (`for_path`/`for_path_ext` applied via `style_f`, Go parity incl. the
      fish `--flag=value` optarg heuristic; inshellisense test added).
- [x] shlex: triple-quote runes missing from `raw_value`/`span` (xonsh).
- [x] shlex: variable detection gated on format (cmd/powershell have none in Go)
      — already the case (`variable.rs` returns `None` for cmd/powershell;
      oil rides bash like Go; tested in `tests/edgecase.rs`).

## Low priority / hygiene

- [x] unused `shlex = "1"` dependency in carapace-rs-shlex.
- [x] dead code: `traverse/mod.rs` `let action = action;`; `condition.rs` has no
      callers (wire into `unless_f` ecosystem or remove — currently kept,
      tests added).
- [x] shlex: block-comment opener runes pushed twice into `raw_value`.
- [x] shlex: PowerShell stream-redirect digits 2-9 merge (Go: 1-6 only).
- [x] shlex: cmd/powershell get POSIX variable detection (Go: none).
- [x] bridge bash: temp snippet file — atomic create with unique name
      (`os.CreateTemp` analog), no pre-epoch clock panic.
- [x] bridge bash: description trim order (`trim()` before trimming `)`)
      leaves trailing whitespace.
- [x] bridge zsh: strip ANSI only from the value part; handle non-SGR CSI
      sequences.
- [x] `Shell::detect()` has no Windows support (cmd-clink detection).
      Refactored detection into `shell/ps.rs`: the `/proc` walk (unix, incl.
      the Termux linker resolution) and a `CreateToolhelp32Snapshot` walk
      (windows, via a target-gated `windows-sys` dependency) both feed a
      shared `classify_shell`; `detect()` works on both platforms.
- [x] `entry.rs`: `style set` swallows I/O errors; `args[2]` dropped silently
      in the ≥3-arg completion arm. `style_set` now returns its errors (Go
      `style.Set` reports them on stderr; a test covers the unwritable-dir
      path). The `args[2]` drop is Go parity: Go's `complete()` also drops
      the command name (`traverse(cmd, args[2:])`) and the entry comment
      documents this.
- [x] `serde_yaml` is archived upstream — plan a migration (recorded below;
      execution deliberately deferred until the fork landscape settles).

      **Migration plan.** Usage is small and isolated: `carapace-rs-spec`
      only uses `serde_yaml::from_str`, `to_string` and `Value`/`Mapping`
      (`src/lib.rs:63`, `src/marshal.rs:10`). Preferred path: switch to the
      drop-in fork `serde_yaml_ng` (same API, actively maintained, MIT) —
      a one-line dependency change plus round-trip re-verification of the
      existing spec tests (`yaml_round_trip`, `hidden_flags_survive_round_trip`,
      `persistent_flags_round_trip_and_inherit`). Fallback if that fork
      stalls: a thin serde deserializer/serializer over `saphyr-parser`,
      kept behind the `spec` feature (DD18) so the default build stays
      unaffected. Do NOT adopt `serde_yml` (libyml dependency has open
      soundness issues). Acceptance: `cargo test -p carapace-rs-spec` and
      the `_carapace spec` snippet tests pass unchanged, no YAML-format
      drift in the dumped specs.
- [x] expose only Go-public `TokenSliceExt` methods; fix `lexer.rs:8` doc link.
      `pipelines`/`current_token`/`words_with_substitutions` (Go-unexported)
      moved to a separate `#[doc(hidden)] pub trait TokenSliceInternals`;
      `TokenSliceExt` now carries only the five Go-exported methods.

## Test infrastructure

- [x] guard fish/zsh bridge tests on tool availability (AGENTS.md rule).
- [x] env mutations hold their binary's env lock and restore state with a
      panic-safe Drop guard (`config_exec.rs`, `shell_dispatch.rs`,
      `entry.rs`). Correction to the review: test binaries are separate
      processes, so env races are intra-binary only — the per-binary lock is
      sufficient; the earlier "cross-binary" framing was wrong.
- [ ] shared filesystem temp dirs without unique suffixes audited
      (`carapace-files-test` fixed; Mock dirs pointing at global temp).
- [ ] direct tests for `bash::patch`, bash bridge, `carapace_bin` bridge.
- [ ] replace wall-clock-margin tests (`example.rs` timeout/cache TTL) with
      deterministic ones; give `batch.rs` `Barrier` test a timeout strategy.
- [ ] nested hooks, `positional_completion` padding, spec feature-off build,
      `command_completer!`, Windows-safe `/tmp` test.

## Feature backlog (not bugs)

- [ ] missing bridges vs Go: argcomplete, aws, bpaf, click, complete, gcloud,
      jj, kingpin, kitten, powershell, symfony, typer, urfavecli, yargs.
- [ ] spec macros: `$message`, `$spec`, shell macros, custom/external macros
      — done except **custom in-process macros** (Go `AddMacro`/`AddMacroI`/
      `AddMacroV`); the external `_carapace macro` protocol covers
      cross-binary macros.
- [x] clap: `value_delimiter`, remaining `ValueHint`s, command-level
      `allow_hyphen_values`, subcommand short flags, bounded nargs.
      Done: `value_delimiter` (model field + traverse split, flags and
      positionals), `ValueHint`s (path hints, command hints → executables;
      host/user/url hints have no engine action yet), subcommand short
      flags (model + `lookup_subcommand` token match). Command-level
      `allow_hyphen_values` is deprecated in clap 4 (private getter) —
      per-arg is the supported form. Remaining: bounded `Nargs` (`2..=4`
      still collapses to variadic).
- [ ] shlex: document the sanctioned divergences in AGENTS.md (cmd tokenizer).
