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
- [ ] pre-invoke hook nesting order vs docs (`convert.rs:189-196` runs the
      command's own hook first; docs claim ancestors-first) — decide the Go
      semantics, align code and docs, add a nested-hooks test.
- [ ] spec `to_yaml`: drop nothing — emit `completion` section, hidden flags
      (`&`), flag aliases; round-trip test.
- [ ] spec: persistent flags not persistent (`lookup_arg` only checks the
      current node) — decide model-level persistence.
- [ ] spec: ` ||| ` modifier separator; implement the Go modifier set
      (`$nospace`, `$filter`, `$style`, ...) instead of "unsupported macro".
- [ ] spec: error visibly on unsupported flag keys (multi-char shorthand,
      NameAsShorthand) instead of silently dropping flags.
- [ ] spec: envsubst (`$VAR`/`${VAR}`) in entries; `C_ARGn`/`C_VALUE` exports.
- [ ] bridge: cobra directive-error paths should match Go (`ActionValues()`,
      "unexpected command output" on empty output).
- [ ] bridge: fish/zsh/bash/inshellisense missing `style.ForPath` styling.
- [ ] shlex: triple-quote runes missing from `raw_value`/`span` (xonsh).
- [ ] shlex: variable detection gated on format (cmd/powershell have none in Go).

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
- [ ] `Shell::detect()` has no Windows support (cmd-clink detection).
- [ ] `entry.rs`: `style set` swallows I/O errors; `args[2]` dropped silently
      in the ≥3-arg completion arm.
- [ ] `serde_yaml` is archived upstream — plan a migration.
- [ ] expose only Go-public `TokenSliceExt` methods; fix `lexer.rs:8` doc link.

## Test infrastructure

- [x] guard fish/zsh bridge tests on tool availability (AGENTS.md rule).
- [ ] env-lock discipline is per-binary and cannot serialize across test
      binaries: `config_exec.rs` mutates `XDG_CONFIG_HOME`/`CARAPACE_MATCH`
      racing style/prefix tests in other binaries. Fix structurally: shared
      env-guard helper crate, or pass env explicitly.
- [ ] non-atomic env restore on panic (`shell_dispatch.rs`, `entry.rs`,
      `config_exec.rs`): save/restore in a Drop guard.
- [ ] direct tests for `bash::patch`, bash bridge, `carapace_bin` bridge.
- [ ] replace wall-clock-margin tests (`example.rs` timeout/cache TTL) with
      deterministic ones; give `batch.rs` `Barrier` test a timeout strategy.
- [ ] nested hooks, `positional_completion` padding, spec feature-off build,
      `command_completer!`, Windows-safe `/tmp` test.

## Feature backlog (not bugs)

- [ ] missing bridges vs Go: argcomplete, aws, bpaf, click, complete, gcloud,
      jj, kingpin, kitten, powershell, symfony, typer, urfavecli, yargs.
- [ ] spec macros: `$message`, `$spec`, shell macros, custom/external macros.
- [ ] clap: `value_delimiter`, remaining `ValueHint`s, command-level
      `allow_hyphen_values`, subcommand short flags, bounded nargs.
- [ ] shlex: document the sanctioned divergences in AGENTS.md (cmd tokenizer).
