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
        session.terminal.clear()?;
        Ok(session)
    }
    pub fn terminal(&mut self) -> &mut NativeTerminal {
        &mut self.terminal
    }
}
impl Drop for TerminalSession {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(self.cleanup, LeaveAlternateScreen, Show);
        let _ = self.cleanup.flush();
    }
}
