//! `flag` command: every pflag type as a flag (Go `example/cmd/flag.go`).
//!
//! The Go source toggles `NoOptDefVal` via a `PreRun` when `--optarg` is
//! the second command line argument; that dynamic rewiring has no test
//! coverage and no clap-native equivalent, so it is omitted.

use carapace_rs::Action;
use carapace_rs_clap::Carapace;
use clap::builder::ArgAction;
use clap::{Arg, Command};

use super::with_help;

pub fn cmd() -> Command {
    let mut cmd = with_help(
        Command::new("flag")
            .about("flag example")
            .completion_group("main"),
    );

    for name in [
        "Bool",
        "BoolSlice",
        "BytesBase64",
        "BytesHex",
        "Count",
        "Duration",
        "DurationSlice",
        "Float32P",
        "Float32Slice",
        "Float64P",
        "Float64Slice",
        "Int16",
        "Int32",
        "Int32Slice",
        "Int64",
        "Int64Slice",
        "Int8",
        "Int",
        "IntSlice",
        "IPMask",
        "IP",
        "IPNet",
        "IPSlice",
        "StringArray",
        "String",
        "StringSlice",
        "StringToInt64",
        "StringToInt",
        "StringToString",
        "Uint16",
        "Uint32",
        "Uint64",
        "Uint8",
        "Uint",
        "UintSlice",
    ] {
        cmd = cmd.arg(Arg::new(name).long(name).action(ArgAction::Set).help(name));
    }

    cmd = cmd
        .flag_completion([
            ("Bool", Action::values(["true", "false"])),
            ("BoolSlice", Action::values(["true", "false"])),
            ("BytesBase64", Action::values(["MQo=", "Mgo=", "Mwo="])),
            ("BytesHex", Action::values(["01", "02", "03"])),
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
            ("IPMask", Action::values(["0.0.0.1", "0.0.0.2", "0.0.0.3"])),
            ("IP", Action::values(["0.0.0.1", "0.0.0.2", "0.0.0.3"])),
            (
                "IPNet",
                Action::values(["0.0.0.1/0", "0.0.0.2/0", "0.0.0.3/0"]),
            ),
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
