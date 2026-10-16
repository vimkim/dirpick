//! The controlling terminal is separate from stdout, reserved for future actions.
use crossterm::{
    cursor::{Hide, Show},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};
use std::{
    fs::{File, OpenOptions},
    io::{self, Write},
};

pub type NativeTerminal = Terminal<CrosstermBackend<File>>;
pub struct TerminalSession {
    terminal: NativeTerminal,
    cleanup: File,
}
impl TerminalSession {
    pub fn open() -> io::Result<Self> {
        let writer = OpenOptions::new().read(true).write(true).open("/dev/tty")?;
        let cleanup = writer.try_clone()?;
        let mut session = Self {
            terminal: Terminal::new(CrosstermBackend::new(writer))?,
            cleanup,
        };
        enable_raw_mode()?;
        execute!(session.cleanup, EnterAlternateScreen, Hide)?;
        session.redraw()?;
        Ok(session)
    }
    pub fn terminal(&mut self) -> &mut NativeTerminal {
        &mut self.terminal
    }
    pub fn perform(&mut self, action: &crate::action::Action) -> io::Result<()> {
        match action {
            crate::action::Action::CopyPath { .. } => action.write_to(&mut self.cleanup),
            crate::action::Action::OpenEditor { path, line } => {
                let command =
                    crate::action::EditorCommand::from_environment(path, *line, *line > 1)?;
                let result = (|| {
                    self.cleanup.flush()?;
                    execute!(self.cleanup, LeaveAlternateScreen, Show)?;
                    disable_raw_mode()?;
                    command.run(&self.cleanup)
                })();
                // Restore on both failed spawn and successful/failed child return.
                let restored = (|| {
                    enable_raw_mode()?;
                    execute!(self.cleanup, EnterAlternateScreen, Hide)?;
                    self.redraw()
                })();
                match (result, restored) {
                    (_, Err(error)) => Err(io::Error::other(format!(
                        "Could not restore terminal after editor: {error}"
                    ))),
                    (result, Ok(())) => result,
                }
            }
        }
    }
    /// Fullscreen resize clears the back buffer without Crossterm's global
    /// cursor query, which writes to stdout instead of the backend's tty.
    pub fn redraw(&mut self) -> io::Result<()> {
        let area = self.terminal.size()?.into();
        self.terminal.resize(area)
    }
}
impl Drop for TerminalSession {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(self.cleanup, LeaveAlternateScreen, Show);
        let _ = self.cleanup.flush();
    }
}
