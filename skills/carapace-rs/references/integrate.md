# Integrating carapace-rs into a clap CLI

How to wire carapace-rs completion into a clap application — builder API, derive macros, and the `_carapace` entry point.

> **Source of truth**: `carapace-rs-clap/src/`, the README, and the example tree `carapace-rs-clap/examples/example/`.
> For modifier and action semantics, see the **carapace** skill → `references/action.md` (identical semantics, Rust naming).

## The wiring pattern

A completion request arrives as `mybin _carapace <shell> <cmd> <words…>` in argv. Handle it **before** clap parses anything — the words are raw completion input (partial flags, unknown flags, open quotes) that clap's parser must not touch:

```rust
use carapace_rs_clap::Carapace;   // trait methods on clap::Command
use carapace_rs::Action;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Some(output) = carapace_rs_clap::complete(cli, &args) {
        println!("{output}");
        return;
    }
    let _matches = cli().get_matches();   // normal execution
}

fn cli() -> clap::Command {
    Command::new("myapp")
        .flag_completion([("profile", Action::values(["dev", "prod"]))])
        .positional_completion([Action::values(["x86", "arm"])])
        .positional_any_completion(Action::files([]))
}
```

`complete()` returns `Some(String)` for completion/snippet/spec/macro/style requests and `None` when the program should run normally. Snippets invoke `{binary} _carapace <shell>` with the command name; the word being completed is last and dropped before traversal (Go `complete()` convention).

## The `Carapace` trait

Implemented for `clap::Command`, chainable, fluent (port of Go `carapace.Gen`):

| Method | Completes |
|---|---|
| `flag_completion([(key, action), …])` | values of named flags; keys match long names, aliases **or** shorthands (Go `FlagCompletion` ActionMap) |
| `positional_completion([action, …])` | the Nth action completes the Nth positional (unless it has its own slot) |
| `positional_any_completion(action)` | any positional beyond the defined ones |
| `dash_completion([action, …])` | positionals after `--` |
| `dash_any_completion(action)` | any positional after `--` |
| `completion_group(id)` | assigns a display group; the id becomes the completion tag and selects the highlight style (named `completion_group` because clap's inherent `Command::group` takes precedence) |
| `disable_flag_parsing()` | every argument completes as a positional (cobra `DisableFlagParsing`) |
| `use_line(line)` | full usage line, used as fallback usage for positional completion (cobra `cmd.Use`) |
| `pre_invoke(hook)` | hook wrapping every flag/positional completion of the command and its subcommands; runs at invocation time (Go `Gen(cmd).PreInvoke`) |

Free-function equivalents for `&mut clap::Command` exist in `carapace_rs_clap`: `flag_completion`, `positional_completion`, `dash_completion`. Note the assignment semantics: `flag_completion` merges per key; `positional_completion`/`dash_completion` replace the whole list.

## clap derive

Attach completers inline via clap derive's `add = <expr>` escape hatch:

```rust
use carapace_rs_clap::completer;

#[derive(clap::Parser)]
struct Cli {
    #[arg(long, add = completer!(Action::values(["dev", "prod"])))]
    profile: Option<String>,
}
```

- `completer!(action)` → `ArgCompleter` (per-arg slot).
- `command_completer!(action)` → `CommandCompleter` (per-command slot, e.g. any-positional on the derive struct).

## Extension slots

Completers attach to args/commands via clap's `unstable-ext` mechanism — no global state:

| Type | Trait | Slot |
|---|---|---|
| `ArgCompleter(Action)` | `ArgExt` | per-arg completion |
| `ArgConflicts(Vec<String>)` | `ArgExt` | directional conflicts removed from flag-name completion (clap does not expose its conflict graph) |
| `ArgPersistent` | `ArgExt` | mark an arg persistent; clap's `global` args are also treated as persistent |
| `CommandCompleter(Action)` | `CommandExt` | command-level completion |
| `DashAnyCompleter(Action)` | `CommandExt` | any-positional after `--` |

## Free completions from existing clap annotations

An arg without an explicit slot still completes if clap already knows enough (checked in this order):

1. explicit `ArgCompleter` slot (per arg)
2. explicit command-level registration — `flag_completion`/`positional_completion` entries matching the arg
3. `possible_values` — `ValueEnum` and `PossibleValuesParser` values (with their help as descriptions)
4. `ValueHint` — `DirPath` → directories, `FilePath`/`AnyPath` → files, `CommandName`/`CommandString`/`CommandWithArguments` → executables

Note that clap derive infers `ValueHint::AnyPath` for `PathBuf`/`Path` args automatically; explicit registration (slot or command-level map) outranks it.

For `CommandWithArguments`, prefer explicit dynamic bridge wiring — see [bridges.md](bridges.md).

## Gotchas

- `Context.parsed_flags` keys are flag **ids**, not long flag names: clap argument ids (`work_tree` for `--work-tree`), YAML spec keys exactly as written (Go spec parity), and the shorthand string for shorthand-only flags. Registration (`flag_completion`, slots) accepts either spelling.
- Flag-value completers return **bare values** (`"dev"`); the attached form (`--profile=dev`) is reconstructed by the traversal — do not pre-attach the prefix.
- `complete()` takes a *factory* (`Fn() -> clap::Command`), not a `Command` value; the tree is rebuilt per request.
- The `_carapace` subcommand visible in the tree is carapace's own hidden entry point; completion of it is gated behind `CARAPACE_HIDDEN=2`.
- `_carapace spec` requires the default-on `spec` feature.

## Related

- Shell registration and environment variables → [setup.md](setup.md)
- Testing completions in-process → [testing.md](testing.md)
