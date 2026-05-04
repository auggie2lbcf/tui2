mod app;
mod launcher;
mod terminal;
mod ui;

use std::io;

use app::App;
use terminal::{restore_terminal, setup_terminal};

fn main() -> io::Result<()> {
    let mut terminal = setup_terminal()?;
    let app_result = App::new().run(&mut terminal);
    restore_terminal(&mut terminal)?;
    app_result
}
