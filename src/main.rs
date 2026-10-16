use crossterm::event::{self, Event, KeyEventKind};
use dirpick::{
    action::Action,
    session::{Options, SearchSession},
    terminal::TerminalSession,
    ui::Workbench,
};
use std::{
    io::{self, Write},
    time::Duration,
};

fn run() -> io::Result<()> {
    let mut shell_integration = false;
    match std::env::args_os().nth(1) {
        Some(argument) if argument == "--help" || argument == "-h" => {
            println!(
                "dirpick — terminal path search and preview\n\nUsage: dirpick [--json]\n\nF1 Files; F2 Directories; F3 Contents; F4 Regex.\nEnter browses directories or opens a file/match in VISUAL/EDITOR; Alt-Left goes Back.\nLaunch in a directory; Project discovers the enclosing Git working tree.\nF7 switches to launch CWD. F12 shows keyboard help and full status details.\nEsc or Ctrl-C quits. Ctrl-G cd & exit requires the Nushell wrapper and --json.\n--print-nu-wrapper prints the packaged Nu module; see README for setup.\n--json reserves stdout for a cd action; quit/cancel produces no output.\n--third-party-notices prints bundled dependency and syntax license notices."
            );
            return Ok(());
        }
        Some(argument) if argument == "--version" || argument == "-V" => {
            println!("dirpick {}", env!("CARGO_PKG_VERSION"));
            return Ok(());
        }
        Some(argument) if argument == "--third-party-notices" => {
            print!("{}", include_str!("../THIRD_PARTY_NOTICES.txt"));
            return Ok(());
        }
        Some(argument) if argument == "--print-nu-wrapper" => {
            print!("{}", include_str!("../integrations/dirpick.nu"));
            return Ok(());
        }
        Some(argument) if argument == "--json" && std::env::args_os().len() == 2 => {
            shell_integration = true;
        }
        Some(_) => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Unknown argument; see dirpick --help",
            ));
        }
        None => {}
    }
    let mut session = SearchSession::with_options(
        std::env::current_dir()?,
        Options {
            shell_integration,
            ..Options::default()
        },
    )?;
    let mut ui = Workbench::new(&session);
    let mut terminal = TerminalSession::open()?;
    loop {
        session.poll();
        while let Some(action) = session.take_action() {
            if matches!(action, Action::ChangeDirectory { .. }) {
                let mut json = Vec::new();
                match action.write_json(&mut json) {
                    Ok(()) => {
                        drop(terminal);
                        let mut output = io::stdout().lock();
                        output.write_all(&json)?;
                        output.flush()?;
                        return Ok(());
                    }
                    Err(error) => session.complete_action(action, Err(error)),
                }
                continue;
            }
            let result = terminal.perform(&action);
            session.complete_action(action, result);
        }
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
                    terminal.redraw()?;
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
