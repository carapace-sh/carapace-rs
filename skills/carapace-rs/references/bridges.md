# Bridges

Completing values that belong to *external* commands — frameworks, other shells, other completion engines.

> **Source of truth**: `carapace-rs-bridge/src/`. Go equivalent: `carapace-bridge/pkg/actions/bridge/`.

## Bridge actions

Every bridge is a module with an `action(command)` constructor (plus `carapace_rs_bridge::action(name, command)` for dispatch by string). The delegate command is fixed at wiring time:

```rust
use carapace_rs_bridge as bridge;

bridge::action("fish", &["myapp"])       // or by module:
bridge::fish::action(&["myapp"])
bridge::bash::action(&["myapp"])
bridge::zsh::action(&["myapp"])
bridge::carapace::action(&["myapp"])     // Go carapace export protocol
bridge::carapace_bin(&["git"])           // carapace-bin specs (export JSON)
bridge::clap::action(&["myapp"])         // clap_complete unstable-dynamic
bridge::cobra::action(["git"])           // cobra __complete with ShellCompDirective bits
bridge::inshellisense::action(&["myapp"])
```

Additional bridges in the crate: `argcomplete`, `aws`, `bpaf`, `click`, `complete` (posener/complete), `gcloud`, `jj`, `kingpin`, `kitten`, `powershell`, `symfony`, `typer`, `urfavecli`, `yargs`, `shell` (generic `_carapace`-style dispatch).

Wiring example:

```rust
Command::new("outer").positional_any_completion(bridge::cobra::action(["git"]));
```

## Dynamic router: empty command

Passing an **empty command** turns a bridge into a router — the first word of the value selects the delegate:

```rust
// `--exec git st<TAB>` first completes executables (files as fallback),
// then delegates the remainder to git's completion
Command::new("outer").flag_completion([
    ("exec", bridge::carapace_bin([])),
]);
```

Mechanics (port of Go `actionCommand`): with an empty command, the bridge builds a mini command tree whose first positional completes `batch(executables, files)` and whose any-positional delegates `f(first_word).shift(1)`; it completes via `Action::execute` (in-process embedded traversal, outer context forwarded).

## Gotchas

- Word joining/quoting is delegated to `carapace_rs_shlex::join` (Go `shlex.Join`): quoting is applied only when the target shell needs it; an empty word renders as `""`.
- Bridge output empty → no candidates or a files fallback (bridge-specific); errors degrade to message actions.
- In tests, mock the delegate with `Sandbox::reply` instead of executing real binaries — see [testing.md](testing.md).
- The cobra directive semantics (`RegisterFlagCompletionFunc`/`ValidArgsFunction`) live here, not in the clap adapter — see `carapace-rs-bridge/tests/bridge.rs`.

## Related

- `Action::import`/`Action::execute` engine side → [action.md](action.md)
- The `_carapace` export wire format → [setup.md](setup.md)
