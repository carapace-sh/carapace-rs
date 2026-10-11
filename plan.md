# plan.md — carapace-shlex port state

Working notes for the carapace-shlex port. carapace (Go) is the source of
truth; see AGENTS.md for the settled design decisions and ../carapace-shlex
for the reference.

## State (as of this session)

The port is complete:

- The completion core: the 12-state tokenizer (`lexer.rs`), all eleven
  format impls (`formats.rs`), quoting (`quote.rs`: QuoteWord per shell +
  quoteInsertionDefault + PowerShell's backtick insertion), token slice
  helpers (`tokenslice.rs`, public: pipelines, words, strings,
  filter_redirects, wordbreak_prefix), variables (`variable.rs`:
  `$name`/`${name` detection with per-format expansion rules, naive
  COMP_WORDS splitting, `Insert`), substitution scopes (`substitution.rs`),
  and the `Complete()` API (`completion.rs`).
- Full test parity with the reference: the format corpora (bash, cmd,
  fish, zsh, elvish, nushell, xonsh, powershell, hilbish, tcsh, oil incl.
  edge files), edgecase/join/quote suites (as integration tests in
  `tests/`), and the five verify_* suites ported as in-crate unit tests
  (`src/verify.rs`) — they cross-check shell source constants
  (classifier tables, operator grammars, escape tables), not real shells,
  so no ignore-gating is needed. 436 tests in the crate.
- Porting the corpora fixed seven divergences from Go: multi-rune operator
  merging (`;;`, `>&`, `<<<`, ...), bash's restricted double-quote escape
  table (only $ ` " \ newline), CRLF line continuations, the nushell
  substitution post-process ($(( arithmetic, <( process substitution, ))
  closer merging), hilbish's operator grammar (full bash set, not reduced),
  xonsh's any-char double-quote escapes, fish's own quoting table
  (backtick is a regular character), and PowerShell's backtick insertion
  quoting / stop-parsing token state.
- Consumers migrated: `bash::patch`, `Action::split`, bridges (`join`).

## Remaining (optional / deferred)

- `SplitP` (pipelines: redirect → ActionFiles) — port when a user needs
  it; the plumbing is `Action::split_p`. The AGENTS.md "unported" note
  for `${...}` brace forms was stale and is fixed; per-format lexing is
  ported.
- Migration opportunities (same pattern as before, do when touched):
  - The zsh formatter's CARAPACE_COMPLINE 5-state machine could derive
    its state from `carapace_shlex::complete(compline, Zsh)` instead of
    its own scanner — verify identical results first.
  - cmd-clink's clink-specific compline handling (CARAPACE_COMPLINE with
    the Cmd format) is ported; double-check the clink quoting output
    against the cmd lexer states.
- Publishing (decide once the port is complete): crates.io has
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
- Operator extension in the WORDBREAK state must not re-append the rune:
  it is already in `raw_value` (pushed at the top of the loop) —
  double-appending made every multi-rune operator split.
- The cmd tokenizer truncates a dangling-escape tail (keep), shlex drops
  it (keep); `complete_tokens` replaces the cmd-truncated last word with
  the recovered dequoted word — never append without popping first.
- Token spans are rune-based; byte offsets for slicing come from
  `char_indices().nth(span.start)`. Wordbreak tokens at input position 0
  need their span end set too (no kind restriction) or `Words()` won't
  merge them (`[a` in elvish).
- The lexer emits the trailing empty cursor word for whitespace-terminated
  input and recovers open-quote tails natively — callers (bash patch,
  split) must not duplicate that logic.
- Formats with a per-rune unescape table (nushell) keep the backslash on
  unrecognized escapes; formats with an escape-chars table (bash, zsh)
  drop it only for table members; an EMPTY table (elvish, xonsh,
  powershell) means the escape char escapes ANY character.
- `scan_stop_parsing` sets the token state itself (StopParsing) and resets
  the lexer to Start; `next()` must not overwrite the token state or
  insertion quoting loses the raw-mode passthrough.
