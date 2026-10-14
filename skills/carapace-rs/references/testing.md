# Testing Completions

Verifying completion behavior with `carapace-rs-testkit` — in-process sandboxes, mocked external commands, export-JSON assertions, and golden snippet fixtures.

> **Source of truth**: `carapace-rs-testkit/src/lib.rs` and the existing suites (`carapace-rs-clap/tests/example.rs`, `tests/entry.rs`, `carapace-rs-spec/tests/spec.rs`, `carapace-rs-bridge/tests/bridge.rs`).

## The Sandbox

```rust
use carapace_rs_testkit::Sandbox;

let mut sandbox = Sandbox::new("my_test");     // temp dir; cache/ + work/ inside
sandbox.files([("readme.md", "content")]);     // relative paths only (panics on "/", "..")
sandbox.reply("git", &["branch", "--list"]).with("main\nfeature");  // mock external command
sandbox.env("CARAPACE_LENIENT", "1");          // travels on the context, no process env

sandbox
    .run(|| cli(), &["--profile", ""])          // words like a shell hands them over
    .expect(carapace_rs::Action::values(["dev", "prod"]));
```

Assertions compare **normalized export JSON** (values filtered by the completed prefix, sorted by value/display, `uid` and the per-value `nospace` extension stripped). `Run` also offers `values()`, `output()`, `expect_output()`, and `expect_not()`.

The `Sandbox` holds no process-global state — tests run in parallel, and `Drop` removes the sandbox directory. `CARAPACE_*` flags travel on the sandbox context (`sandbox.env`), so they need no process mutation.

## Mocked external commands

`sandbox.reply(program, args).with(stdout)` covers `Action::exec_command` and bridge delegations deterministically — do not require real `git`/`fish`/… in tests. When a test genuinely needs a shell binary (fish/zsh bridge integration), guard on its availability.

## Process environment (entry-point tests only)

Tests that must mutate process env (`COMP_*`/`CARAPACE_COMPLINE` for entry-point and shell-dispatch tests, `PATH` for tabdance `look_path`, `XDG_*`) must hold the test binary's `env_lock()` for the whole test — unlocked env mutation races with parallel tests. The lock is a test-file-local helper (static mutex) — see `carapace-rs-clap/tests/entry.rs` for the pattern.

## Golden snippet fixtures

Snippet output is pinned against `_test/` fixture files, one per shell plus `export.json`:

```bash
cargo run -p carapace-rs-clap --example example -- _carapace <shell> > examples/example/_test/<shell>.<ext>
```

Asserted by `carapace-rs-clap/tests/example_snippets.rs` (`snippets_match_fixtures`, `export_snippet_matches_fixture`). Regenerate all of them when the tree or a snippet template changes; keep the diff reviewable.

## Gotchas

- fish 4.x splits `complete -a` arguments on whitespace — sandbox tests set `-f` (no file fallback) for deterministic candidate lists.
- zsh capture output needs compadd backslash unquoting and ANSI stripping before parsing; lines are `\r\n`-terminated and split on ` -- `.
- Expect actions with styles/tags must match the tree's group assignment — check the batch structure (`Action::styled_values_described` parts with `.tag`/`.style`) rather than a flat list when the dispatch emits tagged batches.

## Related

- What the export JSON contains → [setup.md](setup.md)
- Wiring under test → [integrate.md](integrate.md)
