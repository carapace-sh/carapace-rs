# plan.md — next session: finish the carapace-shlex port

Working notes for the continuation of the carapace-shlex port. carapace (Go)
is the source of truth; see AGENTS.md for the settled design decisions and
../carapace-shlex for the reference.

## State (as of this session)

- The completion core is ported: the 12-state tokenizer (`lexer.rs`), all
  eleven format impls (`formats.rs`), quoting (`quote.rs`: QuoteWord per
  shell + quoteInsertionDefault), token slice helpers (`tokenslice.rs`:
  pipelines, word merging, redirect filtering, wordbreak prefix),
  variables (`variable.rs`: `$name`/`${name` detection with per-format
  expansion rules, naive COMP_WORDS splitting, `Insert`), substitution
  scopes (`substitution.rs`), and the `Complete()` API (`completion.rs`).
- Consumers migrated: `bash::patch` (redirect detection + wordbreak prefix
  from the lexer), `Action::split` (context rewrite, span-based prefixes,
  `variable.insert`, per-state `quote()`), bridges (`join`).
- 194 tests green across 17 suites; the crate has 19 unit tests including
  the `completion_test.go` cases ported 1:1.

## Next steps, in order

1. **Port the format test corpora** — `format_bash_test.go`,
   `format_cmd_test.go` (+ `format_cmd_edge_test.go`),
   `format_fish_test.go`, `format_zsh_test.go`, `format_elvish_test.go`,
   `format_nushell_test.go`, `format_xonsh_test.go`,
   `format_powershell_test.go`, `format_hilbish_test.go`,
   `format_tcsh_test.go`. These are Split-based table tests; porting them
   is mechanical and will shake out state-machine divergences. Also
   `edgecase_test.go` (550 lines) and `join_test.go`/`quote_test.go`.
2. **Port `verify_*_test.go`** (bash, zsh, fish, xonsh, powershell; 600–800
   lines each): they verify lexing/quoting against the REAL shells by
   feeding completions and checking what the shell parses back. Port as
   integration tests behind `#[ignore]` + an env guard (they need the
   shells installed) so CI without shells stays green; run locally.
3. **Verify edge semantics against Go, expected divergences**:
   - `dequote_partial` unescapes every escaped rune inside double quotes;
     bash only treats `` $ ` " \ `` (and newline) as escapable there.
     Needs per-format escape tables (nushell processes `\n` as a newline).
   - `quote_insertion` for `Format::Cmd`: Go's cmdFormat implements
     QuoteInsertion (double the quote inside QUOTING_ESCAPING_STATE); our
     quote_insertion is format-blind — thread the Format through when a
     cmd insertion consumer exists.
   - `raw_last_token`'s scanner vs Go's lexer wordbreak classification for
     quoted wordbreak chars (the scanner treats a quote as boundary-
     relevant only via the quote state; wordbreak runes inside quotes are
     not boundaries — matches bash).
4. **Split follow-ups** (consumers exist but are partial):
   - `SplitP` (pipelines: redirect → ActionFiles) — port when a user needs
     it; the plumbing is `Action::split_p`.
   - Wire `Format` per-shell into the docs: split's format is chosen
     statically by the action author (Go's design); note that the format
     should match the ACTIVE shell for correct quoting (Default works for
     most).
   - The AGENTS.md "unported" note for `${...}` brace forms is STALE —
     `variable.rs` handles the brace form; fix the note.
5. **Migration opportunities** (same pattern as before, do when touched):
   - The zsh formatter's CARAPACE_COMPLINE 5-state machine could derive
     its state from `carapace_shlex::complete(compline, Zsh)` instead of
     its own scanner — verify identical results first.
   - cmd-clink's clink-specific compline handling (CARAPACE_COMPLINE with
     the Cmd format) is ported; double-check the clink quoting output
     against the cmd lexer states.
6. **Publishing** (decide once the port is complete): crates.io has
   `carapace-rs` and `carapace-shlex` free; `carapace` is taken (dormant
   unrelated crate). The repo currently documents git-dependency usage.

## Gotchas learned while porting the lexer (do not regress)

- At EOF the scanner must NOT remove the last raw rune: Go appends a `\0`
  at EOF and its removeLastRaw undoes that; our scanner doesn't append at
  EOF, so EOF branches must not remove (removing corrupts operator tokens
  like the `)` in `$(...)` and broke substitution merging).
- The WORDBREAK/InWord boundary branches must not unread past EOF — an
  `index -= 1` on a None read loops forever (the rune is re-read and
  returned endlessly).
- The cmd tokenizer truncates a dangling-escape tail (keep), shlex drops
  it (keep); `complete_tokens` replaces the cmd-truncated last word with
  the recovered dequoted word — never append without popping first.
- Token spans are rune-based; byte offsets for slicing come from
  `char_indices().nth(span.start)`.
- The lexer emits the trailing empty cursor word for whitespace-terminated
  input and recovers open-quote tails natively — callers (bash patch,
  split) must not duplicate that logic.
