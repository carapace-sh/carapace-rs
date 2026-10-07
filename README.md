# carapace-lib-rs

Rust port of [carapace](https://github.com/carapace-sh/carapace) — full-featured
dynamic shell completion — with [clap](https://github.com/clap-rs/clap) as the
command-tree base.

Supported shells: bash, bash-ble, oil, cmd-clink (Windows), elvish, fish, ion,
nushell, powershell, tcsh, xonsh, zsh — plus the `export` JSON wire format used
for bridging, caching and embedding.

## Usage

```toml
[dependencies]
carapace-clap = { git = "https://github.com/carapace-sh/carapace-lib-rs" }
```

```rust,no_run
use carapace_core::Action;
use carapace_clap::Gen;
use clap::builder::{ArgAction, ValueHint};
use clap::{Arg, Command};

fn cli() -> Command {
    let cmd = Command::new("demo")
        .arg(Arg::new("profile").long("profile").action(ArgAction::Set))
        .arg(Arg::new("target").index(1))
        .subcommand(Command::new("build").about("build things"));

    Gen::new(cmd)
        .flag_completion("profile", Action::values(["dev", "staging", "prod"]))
        .positional_completion(1, Action::values(["x86", "arm", "wasm"]))
        .positional_any_completion(Action::files(Vec::<String>::new()))
        .into_command()
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

    // handle completion requests; normal execution otherwise
    if let Some(output) = carapace_clap::complete(cli, &args) {
        println!("{output}");
        return;
    }
    let _matches = cli().get_matches();
}
```

Register completions per shell (see `snippet()` for the exact scripts):

```bash
source <(demo _carapace bash)          # bash (via snippet)
demo _carapace zsh > ~/.zfunc/_demo    # zsh
demo _carapace fish | source           # fish
```

A runnable demo lives in `carapace-clap/examples/demo.rs`:

```bash
cargo run -p carapace-clap --example demo -- _carapace bash --profile ""
cargo run -p carapace-clap --example demo -- _carapace export -- ""
```

## How it works

- `carapace-clap` attaches completers to clap args via extension slots and
  converts the `clap::Command` tree into a normalized model.
- `carapace-core` provides the framework-independent pieces: the lazy two-phase
  `Action` API (with caching, timeouts, multipart values, styling and tags), the
  traversal engine (lenient argument classification with optarg/shorthand/attach
  semantics), and per-shell formatters behind the shared dispatch
  (filtering, flag merging, message integration, nospace propagation, sorting).

See `PLAN.md` for the full porting plan and status.

## Development

```bash
cargo test          # 148 tests across 14 suites
cargo clippy        # clean
```

Layout:

```
carapace-core/    engine: model, action, traverse, shell formatters, cache, uid, export
carapace-clap/    clap adapter: Gen builder, extension slots, _carapace entry
carapace-testkit/ sandbox harness: work dir, mocked replies, export-JSON assertions
carapace-spec/    YAML user specs: flag suffixes, macros ($files, $directories), parsing modes
carapace-bridge/  external completer bridges: fish, bash, zsh, carapace, carapace-bin, clap
```

Testing completions:

```rust
use carapace_testkit::Sandbox;

let mut sandbox = Sandbox::new("my_test");
sandbox.files([("readme.md", "content")]);
sandbox.reply("git", &["branch", "--list"]).with("main\nfeature");
sandbox.run(|| cli(), &[""]).expect(Action::values(["expected"]));
```

Bridging external completion frameworks:

```rust
use carapace_bridge as bridge;

Gen::new(cmd).positional_any_completion(
    bridge::action("fish", &["myapp"]),   // or bash/zsh/carapace/carapace-bin/clap
).into_command();
```
