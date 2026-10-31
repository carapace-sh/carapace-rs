use carapace_rs_shlex::Format;

#[test]
fn is_bare_word() {
    let tests: Vec<(&str, Format, &str, bool)> = vec![
        ("empty", Format::Bash, "", false),
        ("digits leading", Format::Bash, "7z", true),
        ("dotted", Format::Bash, "toit.lsp", true),
        ("dashed", Format::Bash, "git-rebase", true),
        ("plus inert", Format::Bash, "g++", true),
        ("plus leading", Format::Bash, "+g++", false),
        ("dollar", Format::Bash, "a$(touch x)", false),
        ("backtick", Format::Bash, "a`touch x`", false),
        ("quote", Format::Bash, "a'; touch x", false),
        ("space", Format::Bash, "a b", false),
        ("newline", Format::Bash, "a\nb", false),
        ("colon wordbreak", Format::Bash, "a:b", false),
        ("tcsh equals history", Format::Tcsh, "a=b", false),
        ("tcsh bang", Format::Tcsh, "a!b", false),
        ("xonsh plus arithmetic", Format::Xonsh, "2+2", false),
        ("xonsh minus arithmetic", Format::Xonsh, "1-2", false),
        ("xonsh numeric literal", Format::Xonsh, "1_000", false),
        ("xonsh hex literal", Format::Xonsh, "0x1f", false),
        ("xonsh octal literal", Format::Xonsh, "0o7_7", false),
        ("xonsh float literal", Format::Xonsh, "2.", false),
        ("xonsh exponent literal", Format::Xonsh, "2e3", false),
        ("xonsh invalid exponent literal", Format::Xonsh, "2e", true),
        ("xonsh not a literal", Format::Xonsh, "7z", true),
        ("xonsh invalid literal", Format::Xonsh, "2.4.5", true),
        ("xonsh letter plus", Format::Xonsh, "g++", true),
        ("xonsh empty", Format::Xonsh, "", false),
        ("powershell empty", Format::Powershell, "", false),
        (
            "powershell plus arithmetic",
            Format::Powershell,
            "2+2",
            false,
        ),
        ("powershell hex literal", Format::Powershell, "0x1f", false),
        ("powershell long literal", Format::Powershell, "2l", false),
        (
            "powershell unsigned literal",
            Format::Powershell,
            "2U",
            false,
        ),
        (
            "powershell multiplier literal",
            Format::Powershell,
            "2Kb",
            false,
        ),
        ("powershell micro literal", Format::Powershell, "2us", false),
        (
            "powershell exponent literal",
            Format::Powershell,
            "2e3",
            false,
        ),
        (
            "powershell trailing dot literal",
            Format::Powershell,
            "2.",
            false,
        ),
        ("powershell inert literal", Format::Powershell, "42", true),
        ("powershell inert float", Format::Powershell, "2.5", true),
        ("powershell not a literal", Format::Powershell, "2z", true),
        (
            "powershell mid-string exponent",
            Format::Powershell,
            "x2e3",
            false,
        ),
        ("lua identifier", Format::Lua, "foo_1", true),
        ("lua digit leading", Format::Lua, "7z", false),
        ("lua dotted", Format::Lua, "toit.lsp", false),
        ("lua dashed", Format::Lua, "git-rebase", false),
        ("lua plus", Format::Lua, "g++", false),
        ("lua empty", Format::Lua, "", false),
        ("shared screen rejects", Format::Default, "a;b", false),
        ("shared screen accepts", Format::Default, "toit.lsp", true),
    ];
    for (name, format, word, expected) in tests {
        assert_eq!(
            carapace_rs_shlex::is_bare_word(word, format),
            expected,
            "IsBareWord({word:?}) for {name}"
        );
    }
}
