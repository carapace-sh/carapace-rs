# carapace-rs

Rust port of [carapace](https://github.com/carapace-sh/carapace) — full-featured
dynamic shell completion — with [clap](https://github.com/clap-rs/clap) as the
command-tree base.

Supported shells: bash, bash-ble, oil, cmd-clink (Windows), elvish, fish, ion,
nushell, powershell, tcsh, xonsh, zsh — plus the `export` JSON wire format used
for bridging, caching and embedding.

## Usage

```toml
[dependencies]
carapace-rs-clap = "0.0.5"
```

```rust,no_run
use carapace_rs_clap::Carapace;
use carapace_rs::Action;
use clap::builder::ArgAction;
use clap::{Arg, Command};

fn cli() -> Command {
    Command::new("example")
        .arg(Arg::new("profile").long("profile").action(ArgAction::Set))
        .arg(Arg::new("target").index(1))
        .subcommand(Command::new("build").about("build things"))
        .flag_completion([("profile", Action::values(["dev", "staging", "prod"]))])
        .positional_completion([Action::values(["x86", "arm", "wasm"])])
        .positional_any_completion(Action::files([]))
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

    // handle completion requests; normal execution otherwise
    if let Some(output) = carapace_rs_clap::complete(cli, &args) {
        println!("{output}");
        return;
    }
    let _matches = cli().get_matches();
}
```

With clap derive, the `completer!` macro attaches completers inline (imported for the shortest form):

```rust
use carapace_rs::Action;
use carapace_rs_clap::completer;
use clap::Parser;

#[derive(Parser)]
struct Cli {
    #[arg(long, add = completer!(Action::values(["dev", "prod"])))]
    profile: Option<String>,
}
```

Register completions per shell (see `snippet()` for the exact scripts):

```bash
source <(example _carapace bash) # bash (via snippet)
source <(example _carapace zsh)  # zsh
example _carapace fish | source  # fish
```

A runnable example with a wide command tree (groups, persistent flags,
optargs, every action modifier) lives in `carapace-rs-clap/examples/example/`:

```bash
cargo run -p carapace-rs-clap --example example -- _carapace bash example --profile ""
cargo run -p carapace-rs-clap --example example -- _carapace export example ""
```

The word list after the shell mirrors the generated snippets: the command
name first, the word being completed last (it is dropped before traversal,
matching Go's `complete()`).

## How it works

- `carapace-rs-clap` attaches completers to clap args via extension slots and
  converts the `clap::Command` tree into a normalized model.
- `carapace-rs` provides the framework-independent pieces: the lazy two-phase
  `Action` API (with caching, timeouts, multipart values, styling and tags), the
  traversal engine (lenient argument classification with optarg/shorthand/attach
  semantics), and per-shell formatters behind the shared dispatch
  (filtering, flag merging, message integration, nospace propagation, sorting).

See `AGENTS.md` for architecture, design decisions, and contribution guidance.

## Development

```bash
cargo test --workspace
cargo clippy --workspace
```

Layout:

```
carapace-rs-shlex/   shell lexing: tokenizing, partial tokens, insertion quoting
carapace-rs/         engine: model, action, traverse, shell formatters, cache, uid, export
carapace-rs-clap/    clap adapter: Carapace extension trait, extension slots, _carapace entry,
                  completer!/command_completer! derive macros
carapace-rs-testkit/ sandbox harness: work dir, mocked replies, export-JSON assertions
carapace-rs-spec/    YAML user specs: flag suffixes, macros ($files, $directories, $executables), parsing modes, spec dump
carapace-rs-bridge/  external completer bridges: argcomplete, aws, bpaf, carapace, carapace-bin,
                  clap, click, cobra, fish, gcloud, inshellisense, jj, kingpin, kitten,
                  powershell, shell, symfony, typer, urfavecli, yargs, zsh
```

Testing completions:

```rust
use carapace_rs_testkit::Sandbox;

let mut sandbox = Sandbox::new("my_test");
sandbox.files([("readme.md", "content")]);
sandbox.reply("git", &["branch", "--list"]).with("main\nfeature");
sandbox.run(|| cli(), &[""]).expect(Action::values(["expected"]));
```

Bridging external completion frameworks:

```rust
use carapace_rs_bridge as bridge;

Command::new("outer").positional_any_completion(
    bridge::action("fish", &["myapp"]),   // any name accepted by `bridge::action`
);
```

Passing an empty command name turns a bridge into a dynamic router: the
first word of the value completes executables (plus a file fallback), and
the remainder delegates to that command's own completion — including the
hundreds of specs shipped with an installed carapace-bin:

```rust
Command::new("outer").flag_completion([
    ("exec", bridge::carapace_bin([])),   // `--exec git st<TAB>` completes git's status
]);
```

## YAML specs

The `spec` feature (enabled by default) adds the YAML spec format known from
[carapace-bin](https://github.com/carapace-sh/carapace-bin): a clap app dumps
its own command tree with `example _carapace spec`, and
`carapace-rs-spec` parses the same format in-process:

```yaml
name: mycmd
flags:
  -v, --verbose: be verbose
  -f, --file=: input file
commands:
  - name: sub
    completion:
      positionalany: ["$files"]
completion:
  flag:
    file: ["$files([.go, .mod])"]
  positional:
    - ["first\tdescribed", "second"]
```

```rust
let command = carapace_rs_spec::from_yaml(yaml).unwrap();
```

Flags use suffixes (`=` value, `?` optarg, `*` repeatable, `!` required,
`&` hidden), entries support macros (`$files`, `$directories`, `$executables`,
`$spec`, shell macros like `$bash(cmd)`) and ` ||| ` modifier chaining
(`prefix`, `shift`, `multiparts`, `nospace`, ...). Custom macros register via
`carapace_rs_spec::add_macro`; macros referencing an external binary
(`$myexe.macro(...)`) are served over the `_carapace macro` protocol, which
`_carapace` handles out of the box.

## Licensing

Licensing follows each crate's upstream provenance.
