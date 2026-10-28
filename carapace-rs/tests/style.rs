//! The public style API (port of Go `pkg/style`).

use carapace_rs::Context;
use carapace_rs::style::{self, Style};

#[test]
fn constants_match_go_names() {
    assert_eq!(style::DEFAULT, "");
    assert_eq!(style::BLACK, "black");
    assert_eq!(style::BRIGHT_WHITE, "bright-white");
    assert_eq!(style::BG_MAGENTA, "bg-magenta");
    assert_eq!(style::BG_BRIGHT_CYAN, "bg-bright-cyan");
    assert_eq!(style::BOLD, "bold");
    assert_eq!(style::UNDERLINED, "underlined");
}

#[test]
fn of_combines_styles() {
    assert_eq!(style::of([style::BOLD, style::BLUE]), "bold blue");
    assert_eq!(style::of(["", style::RED, ""]), "red");
    assert_eq!(style::of([]), "");
}

#[test]
fn colors_render_like_elvish_ui() {
    assert_eq!(style::xterm_256_color(5), "color5");
    assert_eq!(style::xterm_256_color(0), "color0");
    assert_eq!(style::true_color(0x0a, 0x0b, 0x0c), "#0a0b0c");
    assert_eq!(style::sgr("blue bold"), "1;34");
}

#[test]
fn parse_covers_styling_words() {
    let style = Style::parse("blue bold");
    assert!(style.bold);
    assert_eq!(style.foreground.as_deref(), Some("blue"));

    let style = Style::parse("bg-red underlined");
    assert_eq!(style.background.as_deref(), Some("red"));
    assert!(style.underlined);

    // 256-color and true color names
    assert_eq!(Style::parse("color5").foreground.as_deref(), Some("color5"));
    assert_eq!(
        Style::parse("#ff8800").foreground.as_deref(),
        Some("#ff8800")
    );
    assert_eq!(
        Style::parse("fg-bright-red").foreground.as_deref(),
        Some("bright-red")
    );
}

#[test]
fn parse_resets_and_toggles() {
    let style = Style::parse("bold blue no-bold");
    assert!(!style.bold);
    assert_eq!(style.foreground.as_deref(), Some("blue"));

    let style = Style::parse("bold toggle-bold");
    assert!(!style.bold);

    let style = Style::parse("blue fg-default");
    assert_eq!(style.foreground, None);

    let style = Style::parse("bg-red bg-default");
    assert_eq!(style.background, None);

    // unparsable words are skipped
    let style = Style::parse("bold nosuchthing");
    assert!(style.bold);
    assert_eq!(style.foreground, None);
}

#[test]
fn sgr_covers_all_attributes() {
    assert_eq!(Style::parse("bold dim italic").sgr(), "1;2;3");
    assert_eq!(Style::parse("underlined blink inverse").sgr(), "4;5;7");
    assert_eq!(Style::parse("blue").sgr(), "34");
    assert_eq!(Style::parse("bright-blue").sgr(), "94");
    assert_eq!(Style::parse("bg-blue").sgr(), "44");
    assert_eq!(Style::parse("bg-bright-blue").sgr(), "104");
    assert_eq!(Style::parse("color123").sgr(), "38;5;123");
    assert_eq!(Style::parse("bg-color123").sgr(), "48;5;123");
    assert_eq!(Style::parse("#0a0b0c").sgr(), "38;2;10;11;12");
    assert_eq!(Style::parse("bg-#0a0b0c").sgr(), "48;2;10;11;12");
    assert_eq!(Style::parse("bold blue bg-red").sgr(), "1;34;41");
}

#[test]
fn sgr_round_trips() {
    for sgr in ["1;34", "0;31", "38;5;123", "48;2;10;11;12", "4;90;100", ""] {
        let style = Style::from_sgr(sgr);
        if !sgr.is_empty() && sgr != "0;31" {
            assert_eq!(style.sgr(), sgr, "round trip for {sgr}");
        }
    }
    assert_eq!(Style::from_sgr("0;31").foreground.as_deref(), Some("red"));
    assert!(!Style::from_sgr("0;31").bold);
}

#[test]
fn for_path_styles_from_ls_colors() {
    let context = Context::default().env("LS_COLORS", "*.md=01;35:di=01;34");
    // an existing directory gets its feature style, a nonexistent file none
    assert_eq!(style::for_path(&context, "."), "blue bold");
    assert_eq!(style::for_path(&context, "nonexistent-file.md"), "");
    // extension styles don't touch the filesystem
    assert_eq!(style::for_path_ext(&context, "readme.md"), "magenta bold");
    assert_eq!(style::for_path_ext(&context, "readme.txt"), "blue bold");
    assert_eq!(style::for_extension(&context, "md"), "magenta bold");
}

#[test]
fn is_parsable_is_all_or_nothing() {
    // Go ui.ParseStyling: nil when any word is invalid, nil for empty
    assert!(style::is_parsable("bold magenta"));
    assert!(style::is_parsable("default"));
    assert!(!style::is_parsable("bold not-a-style"));
    assert!(!style::is_parsable(""));
}
