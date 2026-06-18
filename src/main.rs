use std::io::{stdin, stdout};

use pinentry::Pinentry;
use ui::LayerShell;

mod ui;

fn main() {
    let mut pinentry = Pinentry::new(stdin().lock(), stdout().lock(), LayerShell);

    if let Err(error) = pinentry.run() {
        eprintln!("pinentry: {error}");
        std::process::exit(1);
    }
}
