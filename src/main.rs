use crossterm::event::{self, Event, KeyEventKind};
use dirpick::{session::SearchSession, terminal::TerminalSession, ui::Workbench};
use std::{io, time::Duration};

fn run() -> io::Result<()> {
    match std::env::args_os().nth(1) {
        Some(argument) if argument == "--help" || argument == "-h" => {
            println!(
                "dirpick — terminal Files search and preview\n\nUsage: dirpick\n\nLaunch in a directory; Project discovers the enclosing Git working tree.\nF7 switches to launch CWD. F12 shows keyboard help. Esc or Ctrl-C quits."
            );
            return Ok(());
        }
        Some(argument) if argument == "--version" || argument == "-V" => {
            println!("dirpick {}", env!("CARGO_PKG_VERSION"));
            return Ok(());
        }
        Some(_) => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Unknown argument; see dirpick --help",
            ));
        }
        None => {}
    }
    let mut session = SearchSession::open(std::env::current_dir()?)?;
    let mut ui = Workbench::new(&session);
    let mut terminal = TerminalSession::open()?;
    loop {
        session.poll();
        ui.observe(&session);
        terminal.terminal().draw(|frame| ui.draw(frame, &session))?;
        if event::poll(Duration::from_millis(25))? {
            match event::read()? {
                Event::Key(key) if key.kind != KeyEventKind::Release => {
                    if ui.key(&mut session, key) {
                        break;
                    }
                }
                Event::Resize(_, _) => {
                    terminal.terminal().clear()?;
                }
                _ => {}
            }
        }
    }
    Ok(())
}
fn main() -> std::process::ExitCode {
    match run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("dirpick: {error}");
            std::process::ExitCode::FAILURE
        }
    }
}
