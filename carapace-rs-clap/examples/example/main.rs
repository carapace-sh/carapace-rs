//! Example command tree entry point: `cargo run -p carapace-rs-clap
//! --example example -- _carapace bash example ...`

mod cmd;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Some(output) = carapace_rs_clap::complete(cmd::root_cmd, &args) {
        println!("{output}");
        return;
    }
    println!("example command tree (completion via `_carapace <shell>`)");
}
