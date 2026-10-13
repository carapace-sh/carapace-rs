//! `flag` command: flag shapes exercised by the engine (Go `example/cmd/flag.go`).
//!
//! The Go version lists every pflag type, but completion only observes
//! flag *shapes*, so the flags map onto clap-native equivalents: booleans
//! (`SetTrue`), counting (`Count`), delimited multi-value (`Append` with
//! `value_delimiter`), variadic multi-value without delimiter
//! (`StringArray`), typed single values (`value_parser!` for ints,
//! floats, durations and IPs) and map-ish `k=v` strings (no clap map
//! type). pflag-only types without a std or clap parser (`BytesBase64`,
//! `BytesHex`, `IPNet`, `IPMask`) are dropped.
//!
//! The Go `--optarg` flag with its `PreRun` `NoOptDefVal` toggle is
//! unported; the optional-value shape it enabled is covered natively by
//! `action`'s `--embedded-optarg` and `chain`'s `--optarg`.

use std::net::IpAddr;
use std::time::Duration;

use carapace_rs::Action;
use carapace_rs_clap::Carapace;
use clap::builder::{ArgAction, IntoResettable, ValueParser};
use clap::{Arg, Command};

use super::with_help;

type DurationParser = fn(&str) -> Result<Duration, String>;

fn value_flag(name: &'static str, parser: impl IntoResettable<ValueParser>) -> Arg {
    Arg::new(name)
        .long(name)
        .action(ArgAction::Set)
        .value_parser(parser)
        .help(name)
}

fn slice_flag(name: &'static str, parser: impl IntoResettable<ValueParser>) -> Arg {
    Arg::new(name)
        .long(name)
        .action(ArgAction::Append)
        .num_args(1..)
        .value_delimiter(',')
        .value_parser(parser)
        .help(name)
}

fn parse_duration(value: &str) -> Result<Duration, String> {
    let split = value
        .find(|c: char| !c.is_ascii_digit())
        .ok_or_else(|| format!("missing unit: {value}"))?;
    let (number, unit) = value.split_at(split);
    let multiplier = match unit {
        "h" => 3600,
        "m" => 60,
        "s" => 1,
        other => return Err(format!("unsupported unit: {other}")),
    };
    let seconds: u64 = number
        .parse()
        .map_err(|err| format!("invalid number: {err}"))?;
    Ok(Duration::from_secs(seconds * multiplier))
}

pub fn cmd() -> Command {
    let mut cmd = with_help(
        Command::new("flag")
            .about("flag example")
            .completion_group("main"),
    );

    let args = [
        Arg::new("Bool")
            .long("Bool")
            .action(ArgAction::SetTrue)
            .help("Bool"),
        slice_flag("BoolSlice", clap::value_parser!(bool)),
        Arg::new("Count")
            .long("Count")
            .action(ArgAction::Count)
            .help("Count"),
        value_flag(
            "Duration",
            ValueParser::from(parse_duration as DurationParser),
        ),
        slice_flag(
            "DurationSlice",
            ValueParser::from(parse_duration as DurationParser),
        ),
        value_flag("Float32P", clap::value_parser!(f32)),
        slice_flag("Float32Slice", clap::value_parser!(f32)),
        value_flag("Float64P", clap::value_parser!(f64)),
        slice_flag("Float64Slice", clap::value_parser!(f64)),
        value_flag("Int16", clap::value_parser!(i16)),
        value_flag("Int32", clap::value_parser!(i32)),
        slice_flag("Int32Slice", clap::value_parser!(i32)),
        value_flag("Int64", clap::value_parser!(i64)),
        slice_flag("Int64Slice", clap::value_parser!(i64)),
        value_flag("Int8", clap::value_parser!(i8)),
        value_flag("Int", clap::value_parser!(i64)),
        slice_flag("IntSlice", clap::value_parser!(i64)),
        value_flag("IP", clap::value_parser!(IpAddr)),
        slice_flag("IPSlice", clap::value_parser!(IpAddr)),
        Arg::new("StringArray")
            .long("StringArray")
            .action(ArgAction::Append)
            .num_args(1..)
            .help("StringArray"),
        value_flag("String", clap::value_parser!(String)),
        slice_flag("StringSlice", clap::value_parser!(String)),
        value_flag("StringToInt64", clap::value_parser!(String)),
        value_flag("StringToInt", clap::value_parser!(String)),
        value_flag("StringToString", clap::value_parser!(String)),
        value_flag("Uint16", clap::value_parser!(u16)),
        value_flag("Uint32", clap::value_parser!(u32)),
        value_flag("Uint64", clap::value_parser!(u64)),
        value_flag("Uint8", clap::value_parser!(u8)),
        value_flag("Uint", clap::value_parser!(u64)),
        slice_flag("UintSlice", clap::value_parser!(u64)),
    ];
    cmd = cmd.args(args);

    cmd = cmd
        .flag_completion([
            ("Bool", Action::values(["true", "false"])),
            ("BoolSlice", Action::values(["true", "false"])),
            ("Count", Action::values(Vec::<String>::new())),
            ("Duration", Action::values(["1h", "2m", "3s"])),
            ("DurationSlice", Action::values(["1h", "2m", "3s"])),
            ("Float32P", Action::values(["1", "2", "3"])),
            ("Float32Slice", Action::values(["1", "2", "3"])),
            ("Float64P", Action::values(["1", "2", "3"])),
            ("Float64Slice", Action::values(["1", "2", "3"])),
            ("Int16", Action::values(["1", "2", "3"])),
            ("Int32", Action::values(["1", "2", "3"])),
            ("Int32Slice", Action::values(["1", "2", "3"])),
            ("Int64", Action::values(["1", "2", "3"])),
            ("Int64Slice", Action::values(["1", "2", "3"])),
            ("Int8", Action::values(["1", "2", "3"])),
            ("Int", Action::values(["1", "2", "3"])),
            ("IntSlice", Action::values(["1", "2", "3"])),
            ("IP", Action::values(["0.0.0.1", "0.0.0.2", "0.0.0.3"])),
            ("IPSlice", Action::values(["0.0.0.1", "0.0.0.2", "0.0.0.3"])),
            ("StringArray", Action::values(["1", "2", "3"])),
            ("String", Action::values(["1", "2", "3"])),
            ("StringSlice", Action::values(["1", "2", "3"])),
            ("StringToInt64", Action::values(["a=1", "b=2", "c=3"])),
            ("StringToInt", Action::values(["a=1", "b=2", "c=3"])),
            ("StringToString", Action::values(["a=1", "b=2", "c=3"])),
            ("Uint16", Action::values(["1", "2", "3"])),
            ("Uint32", Action::values(["1", "2", "3"])),
            ("Uint64", Action::values(["1", "2", "3"])),
            ("Uint8", Action::values(["1", "2", "3"])),
            ("Uint", Action::values(["1", "2", "3"])),
            ("UintSlice", Action::values(["1", "2", "3"])),
        ])
        .dash_completion([
            Action::values(["d1", "dash1", "-d1", "--dash1"]),
            Action::values(["d2", "dash2", "-d2", "--dash2"]),
        ])
        .dash_any_completion(Action::values(["dAny", "dashAny", "-dAny", "--dashAny"]));

    cmd
}
