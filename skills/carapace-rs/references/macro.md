# Spec Macros and Modifier Chaining

Macros usable in YAML spec entries, custom macro registration, and ` ||| ` modifier chaining.

> For the **macro signature syntax** (MacroI/MacroN/MacroV argument forms) and general modifier semantics, see the **carapace** skill → `references/macro.md`. This document covers the macros available in carapace-rs and their registration.

## Built-in macros

| Macro | Completes |
|---|---|
| `$files([exts])` | files, optionally filtered by extension list |
| `$directories` | directories |
| `$executables` | executables on `PATH` |
| `$message(...)` | displays a message |
| `$spec(path, …)` | completes from another spec |
| `$sh(command)`, `$bash`, `$cmd`, `$elvish`, `$fish`, `$nu`, `$osh`, `$pwsh`, `$xonsh`, `$zsh` | delegate to the named shell's own completion |

## External macros

`$myexe.macro(args)` invokes a macro registered by another binary over the `_carapace macro` protocol. The `_carapace` entry point handles this out of the box:

- `_carapace macro` → JSON list of registered macros (`{name, signature, description}`)
- `_carapace macro <name>` → signature or `unknown macro: <name>`
- `_carapace macro <name> <words…>` → runs the macro, export JSON on stdout

Requires the consuming binary to opt in (the carapace-rs clap adapter wires this when the `spec` feature is on).

## Custom macros

Register in the process-wide registry (Go `AddMacro`):

```rust
// name embeds the executable prefix → $myexe.greet(args) in specs
carapace_rs_spec::add_macro(
    "myexe.greet",
    "greets the user",
    "$myexe.greet()",              // signature shown by `_carapace macro`
    |args: &[String], value: &str| /* -> carapace_rs::Action */,
);
```

The deprecated `$_` prefix maps to `$carapace.` with a visible deprecation error.

## Modifier chaining (` ||| `)

Entries split on ` ||| `; each segment is a modifier or the final action entry. A leading modifier applies to the whole batch. Supported modifiers: `chdir` (with `$gitdir`, `$gitworktree`, `$parent(...)` forms), `filter`, `filterargs`, `list`, `multiparts`, `nospace`, `noprefix`, `prefix`, `retain`, `shift`, `split`, `splitp`, `suffix`, `suppress`, `style`, `tag`, `uniquelist`, `usage`.

For chaining semantics and worked examples, see the **carapace** skill → `references/macro.md` (identical behavior).

## Related

- Spec format and entry syntax → [spec.md](spec.md)
- Action semantics behind the modifiers → [action.md](action.md)
