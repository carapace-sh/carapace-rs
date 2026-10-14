# The Action API in Rust

Building completion candidates with `carapace_rs::Action` — constructors, modifiers, batching, caching, multipart values.

> **Semantics are identical to Go carapace.** For modifier behavior and patterns, see the **carapace** skill → `references/action.md`. This document covers the Rust surface and naming only.

## Naming conventions

Go's CamelCase maps to snake_case, and the `F`/`P`/`E`/`N` suffixes are kept as `_f`/`_n`/`_e`:

| Go | Rust |
|---|---|
| `ActionValues` | `Action::values` |
| `ActionValuesDescribed` | `Action::values_described` |
| `ActionMultiPartsN` | `Action::multiparts_n` |
| `ActionExecCommandE` | `Action::exec_command_e` |
| `StyleR` | `Action::style_r` |

## Constructors

```rust
Action::values(["dev", "staging", "prod"])
Action::values_described([("main.rs", "rust source"), ("main.go", "go source")])
Action::styled_values([("true", "green"), ("false", "red")])
Action::styled_values_described([("val", "description", "blue")])
Action::directories()
Action::files([".md", ".go", ".mod"])        // empty iterator = all files
Action::executables()                        // $PATH lookup
Action::executables_in(["/usr/bin", "dir"])  // also completes dirs
Action::message("no such thing")             // visible failure (completion is best-effort)
Action::callback(|ctx| /* -> Action */)
Action::exec_command("git", ["remote"], |output| { /* parse bytes -> Action */ })
Action::exec_command_e("false", [""; 0], |result| /* Result<&[u8], String> -> Action */)
Action::import(export_json)                  // parse export-format output (bridge protocol)
Action::execute(Arc::new(model_command))     // complete an embedded command tree in-process
```

The two-phase split: `Action::invoke(&Context) -> InvokedAction` materializes candidates; lazy `Action` composes via `map_invoked`. Failures degrade to `Action::message` — never `Result`.

## Modifiers

All `self -> Self`. Semantics (filtering, prefix/suffix, nospace/noprefix, tag/style, chdir, shift, unless, suppress, cache, timeout, list/multiparts) are the Go ones — see the **carapace** skill for behavior details. Rust signatures:

```rust
action.filter(["unwanted"]).retain(["keep"])
action.prefix("--profile=").suffix("/")
action.nospace(['/', '=', ':'])          // empty = '*' wildcard
action.no_prefix(['-'])
action.tag("main commands").style("blue")
action.style_f(|ctx, value| style)       // dynamic per value
action.style_r(Arc<RwLock<String>>)      // shared mutable style
action.tag_f(|value| tag)
action.usage("usage text")
action.unless(cond).unless_f(|ctx| cond)
action.shift(1)                          // drop N leading args before delegating
action.chdir(dir).chdir_f(|ctx| dir)
action.suppress(["regex…"])              // regex-filtered messages
action.cache("site", 3600, keys)         // <0 never expires, 0 always revalidate
action.timeout(5, fallback_action)       // detached thread; not cancelled
action.list(",").unique_list(";")
```

Multipart values (associated functions, not methods):

```rust
Action::multiparts(":", |ctx| /* -> Action */)
Action::multiparts_n(",", -1, |ctx| /* unlimited */)
Action::multipartsp(":", pattern, |part, known| /* -> Action */)
```

Value-splitting for values that are themselves command lines:

```rust
action.split(format)   // re-lex value, complete inner, requote; $var words complete env names
action.split_p(format) // pipelines; redirect targets complete files
```

## Batch and InvokedAction

```rust
carapace_rs::batch(vec![action_a, action_b])  // parallel invocation (std threads), merged
```

`InvokedAction` (materialized): `to_export()` is the only sanctioned read path; `filter`/`retain`/`prefix`/`suffix`/`unique`/`sort`/`merge` post-process it; `to_action()` wraps it back into a lazy `Action`.

## Gotchas

- Closures must be `Send + Sync` and must not share unsynchronized state; `batch` runs on std threads, not rayon.
- `Action::execute` forwards the outer context (env, dir, mock) into the embedded traversal — embedded trees are not separate processes.
- Go's `NoFish` modifier is not implemented.

## Related

- Wiring actions into clap args → [integrate.md](integrate.md)
- Using actions in YAML spec entries → [spec.md](spec.md) and [macro.md](macro.md)
