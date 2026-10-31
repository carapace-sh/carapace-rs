//! `modifier` command: every action modifier (Go `example/cmd/modifier.go`).

use carapace_rs::Action;
use carapace_rs_clap::{Carapace, execute_embedded};
use clap::{Arg, Command, builder::ArgAction};

use super::with_hidden_help;

pub fn cmd() -> Command {
    let mut cmd = with_hidden_help(
        Command::new("modifier")
            .about("modifier example")
            .completion_group("modifier"),
    )
    .use_line("modifier [pos1]")
    .arg(Arg::new("pos1").index(1));

    for (name, description) in [
        ("batch", "Batch()"),
        ("cache", "Cache()"),
        ("cache-key", "Cache()"),
        ("chdir", "Chdir()"),
        ("chdirf", "ChdirF()"),
        ("filter", "Filter()"),
        ("filterargs", "FilterArgs()"),
        ("filterparts", "FilterParts()"),
        ("invoke", "Invoke()"),
        ("list", "List()"),
        ("multiparts", "MultiParts()"),
        ("multipartsp", "MultiPartsP()"),
        ("nospace", "NoSpace()"),
        ("noprefix", "NoPrefix()"),
        ("prefix", "Prefix()"),
        ("retain", "Retain()"),
        ("shift", "Shift()"),
        ("split", "Split()"),
        ("splitp", "SplitP()"),
        ("style", "Style()"),
        ("stylef", "StyleF()"),
        ("styler", "StyleR()"),
        ("suffix", "Suffix()"),
        ("suppress", "Suppress()"),
        ("tag", "Tag()"),
        ("tagf", "TagF()"),
        ("timeout", "Timeout()"),
        ("unique", "Unique()"),
        ("uniquelist", "UniqueList()"),
        ("uniquelistf", "UniqueListF()"),
        ("unless", "Unless()"),
        ("unlessf", "UnlessF()"),
        ("usage", "Usage()"),
    ] {
        cmd = cmd.arg(
            Arg::new(name)
                .long(name)
                .action(ArgAction::Set)
                .help(description),
        );
    }

    cmd.flag_completion([
        ("batch", batch()),
        ("cache", cache()),
        ("cache-key", cache_key()),
        ("chdir", chdir()),
        ("chdirf", chdirf()),
        ("filter", filter()),
        ("filterargs", filterargs()),
        ("filterparts", filterparts()),
        ("invoke", invoke()),
        ("list", list()),
        ("multiparts", multiparts()),
        ("multipartsp", multipartsp()),
        ("nospace", nospace()),
        ("noprefix", noprefix()),
        ("prefix", prefix()),
        ("retain", retain()),
        ("shift", shift()),
        ("split", split()),
        ("splitp", splitp()),
        ("style", style()),
        ("stylef", stylef()),
        ("styler", styler()),
        ("suffix", suffix()),
        ("suppress", suppress()),
        ("tag", tag()),
        ("tagf", tagf()),
        ("timeout", timeout()),
        ("unique", unique()),
        ("uniquelist", uniquelist()),
        ("uniquelistf", uniquelistf()),
        ("unless", unless()),
        ("unlessf", unlessf()),
        ("usage", usage_action()),
    ])
    .positional_completion([Action::default().usage("explicit positional usage")])
}

pub fn batch() -> Action {
    carapace_rs::batch(vec![
        Action::values_described([("A", "description of A"), ("B", "description of first B")]),
        Action::values_described([
            ("B", "description of second B"),
            ("C", "description of first C"),
        ]),
        Action::values_described([("C", "description of second C"), ("D", "description of D")]),
    ])
}

pub fn cache() -> Action {
    Action::callback(|_| Action::values([current_time()])).cache(
        "example/cmd/modifier.go:cache",
        5,
        Vec::new(),
    )
}

pub fn cache_key() -> Action {
    Action::multiparts("/", |c| match c.parts.len() {
        0 => Action::values(["one", "two"]).suffix("/"),
        1 => {
            let part = c.parts[0].clone();
            Action::callback(move |_| Action::values([current_time()])).cache(
                "example/cmd/modifier.go:cache-key",
                10,
                vec![format!("String({part})")],
            )
        }
        _ => Action::default(),
    })
}

pub fn chdir() -> Action {
    Action::files([]).chdir(std::env::temp_dir().to_string_lossy().into_owned())
}

pub fn chdirf() -> Action {
    Action::files([])
        .chdir_f(|c| carapace_rs::traverse::git::git_work_tree(c).unwrap_or_else(|error| error))
}

pub fn filter() -> Action {
    Action::values_described([("1", "one"), ("2", "two"), ("3", "three"), ("4", "four")])
        .filter(["2", "4"])
}

pub fn filterargs() -> Action {
    Action::values(["one", "two", "three"]).filter_args()
}

pub fn filterparts() -> Action {
    Action::multiparts(",", |_| {
        Action::values(["one", "two", "three"])
            .filter_parts()
            .suffix(",")
    })
}

pub fn invoke() -> Action {
    Action::callback(|c| {
        let mut context = c.clone();
        if let Some(rest) = c.value.strip_prefix("file://") {
            context.value = rest.to_owned();
        } else if c.value.starts_with("file") && c.value.contains("://") {
            context.value = String::new();
        } else {
            return Action::default();
        }
        Action::files([])
            .invoke(&context)
            .prefix("file://")
            .to_action()
    })
}

pub fn list() -> Action {
    Action::values(["one", "two", "three"]).list(",")
}

pub fn multiparts() -> Action {
    values_multiparts(
        Action::values([
            "dir/subdir1/fileA.txt",
            "dir/subdir1/fileB.txt",
            "dir/subdir2/fileC.txt",
        ]),
        "/",
    )
}

/// Segment-wise completion of the action's values (Go `Action.MultiParts`
/// via `ToMultiPartsA`).
fn values_multiparts(action: Action, divider: &'static str) -> Action {
    Action::callback(move |c| action.clone().invoke(c).to_multiparts_a(&[divider]))
}

pub fn multipartsp() -> Action {
    Action::styled_values_described([
        ("keys/<key>", "key example", ""),
        ("keys/<key>/<value>", "key/value example", ""),
        ("styles/custom", "custom style", "blue blink"),
        ("styles", "list", "yellow"),
        ("styles/<style>", "details", ""),
    ])
    .multiparts_p("/", "<.*>", |placeholder, matches| match placeholder {
        "<key>" => Action::values(["key1", "key2"]),
        "<style>" => Action::styles([]),
        "<value>" => match matches.get("<key>").map(String::as_str) {
            Some("key1") => Action::values(["val1", "val2"]),
            Some("key2") => Action::values(["val3", "val4"]),
            _ => Action::default(),
        },
        _ => Action::default(),
    })
}

pub fn nospace() -> Action {
    Action::values(["one,", "two/", "three"]).nospace([',', '/'])
}

pub fn noprefix() -> Action {
    Action::values(["--", "---", "one--", "one---"]).no_prefix(['-'])
}

pub fn prefix() -> Action {
    Action::files([]).prefix("file://")
}

pub fn retain() -> Action {
    Action::values_described([("1", "one"), ("2", "two"), ("3", "three"), ("4", "four")])
        .retain(["2", "4"])
}

pub fn shift() -> Action {
    Action::callback(|c| Action::message(format!("{:?}", c.args))).shift(1)
}

pub fn split() -> Action {
    Action::callback(|_| execute_embedded(split_cmd())).split(carapace_rs::Format::Bash)
}

pub fn splitp() -> Action {
    Action::callback(|_| execute_embedded(split_cmd())).split_p(carapace_rs::Format::Bash)
}

fn split_cmd() -> Command {
    with_hidden_help(Command::new("split-embedded"))
        .arg(
            Arg::new("bool")
                .short('b')
                .long("bool")
                .action(ArgAction::SetTrue)
                .help("bool flag"),
        )
        .arg(
            Arg::new("string")
                .short('s')
                .long("string")
                .action(ArgAction::Set)
                .help("string flag"),
        )
        .flag_completion([("string", Action::values(["one", "two", "three with space"]))])
        .positional_completion([Action::values(["pos1", "positional1"]), Action::files([])])
}

pub fn style() -> Action {
    Action::values(["one", "two"]).style("green")
}

pub fn stylef() -> Action {
    Action::values(["one", "two", "three"]).style_f(|_, value| match value.value.as_str() {
        "one" => "green".to_owned(),
        "two" => "red".to_owned(),
        _ => String::new(),
    })
}

pub fn styler() -> Action {
    // Go's StyleR reads the style at invocation time; the context-aware
    // style_f closure is the runtime-configurable equivalent
    Action::values(["one", "two"]).style_f(|_, _| {
        carapace_rs::config::styles()
            .carapace
            .keyword_ambiguous
            .clone()
    })
}

pub fn suffix() -> Action {
    Action::values(["apple", "melon", "orange"]).suffix("juice")
}

pub fn suppress() -> Action {
    carapace_rs::batch(vec![
        Action::message("unexpected error"),
        Action::message("ignored error"),
    ])
    .suppress(["ignored"])
}

pub fn tag() -> Action {
    Action::values(["192.168.1.1", "127.0.0.1"]).tag("interfaces")
}

pub fn tagf() -> Action {
    Action::values(["one.png", "two.gif", "three.txt", "four.md"])
        .style_f(|c, value| carapace_rs::style::for_path_ext(c, &value.value))
        .tag_f(|value| {
            match value
                .value
                .rsplit('.')
                .next()
                .map(|ext| format!(".{ext}"))
                .unwrap_or_default()
                .as_str()
            {
                ".png" | ".gif" => "images",
                ".txt" | ".md" => "documents",
                _ => "",
            }
            .to_owned()
        })
}

pub fn timeout() -> Action {
    Action::callback(|_| {
        std::thread::sleep(std::time::Duration::from_secs(3));
        Action::values(["within timeout"])
    })
    .timeout(2, Action::message("timeout exceeded"))
}

pub fn unique() -> Action {
    Action::values(["one", "two", "two", "three", "three", "three"]).unique()
}

pub fn uniquelist() -> Action {
    Action::values(["one", "two", "three"]).unique_list(",")
}

pub fn uniquelistf() -> Action {
    Action::multiparts_n(":", 2, |c| match c.parts.len() {
        0 => Action::values(["one", "two", "three"]),
        _ => Action::values(["1", "2", "3"]),
    })
    .unique_list_f(",", |s| s.split(':').next().unwrap_or_default().to_owned())
}

pub fn unless() -> Action {
    Action::multiparts_n(":", 2, |c| match c.parts.len() {
        0 => Action::values(["true", "false"]).suffix(":"),
        _ => {
            let first = c.parts[0].clone();
            carapace_rs::batch(vec![
                Action::values(["yes", "positive"]).unless(first != "true"),
                Action::values(["no", "negative"]).unless(first != "false"),
            ])
        }
    })
}

pub fn unlessf() -> Action {
    Action::values(["./local", "~/home", "/abs", "one", "two", "three"])
        .unless_f(carapace_rs::condition::completing_path)
}

pub fn usage_action() -> Action {
    Action::default().usage("explicit usage")
}

fn current_time() -> String {
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let (h, m, s) = ((seconds / 3600) % 24, (seconds / 60) % 60, seconds % 60);
    format!("{h:02}:{m:02}:{s:02}")
}
