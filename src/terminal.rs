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
    os::fd::AsRawFd,
};

pub type NativeTerminal = Terminal<CrosstermBackend<File>>;
pub struct TerminalSession {
    terminal: NativeTerminal,
    cleanup: File,
    original_modes: libc::termios,
}
impl TerminalSession {
    pub fn open() -> io::Result<Self> {
        let writer = OpenOptions::new().read(true).write(true).open("/dev/tty")?;
        let cleanup = writer.try_clone()?;
        let mut original_modes = std::mem::MaybeUninit::uninit();
        // SAFETY: tcgetattr initializes the termios value on success. The
        // descriptor is owned by the controlling-terminal File above.
        if unsafe { libc::tcgetattr(cleanup.as_raw_fd(), original_modes.as_mut_ptr()) } != 0 {
            return Err(io::Error::last_os_error());
        }
        let original_modes = unsafe { original_modes.assume_init() };
        let mut session = Self {
            terminal: Terminal::new(CrosstermBackend::new(writer))?,
            cleanup,
            original_modes,
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
            crate::action::Action::CopyPath { .. }
            | crate::action::Action::ChangeDirectory { .. } => action.write_to(&mut self.cleanup),
            crate::action::Action::OpenEditor { path, line } => {
                let command =
                    crate::action::EditorCommand::from_environment(path, *line, *line > 1)?;
                // Canonical tty interrupts reach the entire foreground process
                // group. Catch them in the picker while the child runs; exec
                // resets caught handlers so the editor still receives Ctrl-C.
                let _signals = EditorSignals::capture()?;
                let result = (|| {
                    self.cleanup.flush()?;
                    execute!(self.cleanup, LeaveAlternateScreen, Show)?;
                    disable_raw_mode()?;
                    self.restore_modes()?;
                    command.run(&self.cleanup)
                })();
                // Restore on both failed spawn and successful/failed child return.
                let restored = (|| {
                    // A failed editor may leave the tty raw. Restore our launch
                    // snapshot before Crossterm captures its next original mode.
                    self.restore_modes()?;
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
    fn restore_modes(&self) -> io::Result<()> {
        // SAFETY: the descriptor remains open and original_modes came from a
        // successful tcgetattr on this controlling terminal.
        if unsafe {
            libc::tcsetattr(
                self.cleanup.as_raw_fd(),
                libc::TCSANOW,
                &self.original_modes,
            )
        } == 0
        {
            Ok(())
        } else {
            Err(io::Error::last_os_error())
        }
    }
}
impl Drop for TerminalSession {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = self.restore_modes();
        let _ = execute!(self.cleanup, LeaveAlternateScreen, Show);
        let _ = self.cleanup.flush();
    }
}

struct EditorSignals {
    saved: Vec<(libc::c_int, libc::sigaction)>,
}
extern "C" fn keep_picker_alive(_signal: libc::c_int) {}
impl EditorSignals {
    fn capture() -> io::Result<Self> {
        let mut guard = Self { saved: Vec::new() };
        for signal in [libc::SIGINT, libc::SIGQUIT] {
            // SAFETY: zeroed sigaction is initialized below, and all pointers
            // passed to libc refer to valid local sigaction/sigset_t values.
            let mut action: libc::sigaction = unsafe { std::mem::zeroed() };
            let mut previous: libc::sigaction = unsafe { std::mem::zeroed() };
            action.sa_sigaction = keep_picker_alive as *const () as libc::sighandler_t;
            action.sa_flags = libc::SA_RESTART;
            unsafe { libc::sigemptyset(&mut action.sa_mask) };
            if unsafe { libc::sigaction(signal, &action, &mut previous) } != 0 {
                return Err(io::Error::last_os_error());
            }
            guard.saved.push((signal, previous));
        }
        Ok(guard)
    }
}
impl Drop for EditorSignals {
    fn drop(&mut self) {
        for (signal, previous) in self.saved.iter().rev() {
            // SAFETY: restore each previously captured disposition. The main
            // event thread owns this scoped guard through terminal reacquisition.
            unsafe { libc::sigaction(*signal, previous, std::ptr::null_mut()) };
        }
    }
}
