//! Collect a complete frame before writing it to the controlling terminal.
use crossterm::{
    cursor::{Hide, MoveTo, Show},
    execute, queue,
    terminal::{BeginSynchronizedUpdate, EndSynchronizedUpdate},
};
use ratatui::{
    Terminal,
    backend::{Backend, ClearType, CrosstermBackend, WindowSize},
    buffer::Cell,
    layout::{Position, Size},
};
use std::io::{self, Write};

pub(super) struct FrameBackend<W: Write> {
    commands: Vec<u8>,
    writer: W,
    cursor_visible: bool,
}

impl<W: Write> FrameBackend<W> {
    pub(super) fn new(writer: W) -> Self {
        Self {
            commands: Vec::new(),
            writer,
            cursor_visible: false,
        }
    }

    fn begin(&mut self) -> io::Result<()> {
        queue!(self.commands, BeginSynchronizedUpdate, Hide)
    }

    fn finish(&mut self, success: bool) -> io::Result<()> {
        // Ratatui requests Show before MoveTo. Defer Show until after the caret
        // has moved, including on terminals that ignore synchronized updates.
        if success && self.cursor_visible {
            queue!(self.commands, Show)?;
        }
        queue!(self.commands, EndSynchronizedUpdate)?;
        let result = self
            .writer
            .write_all(&self.commands)
            .and_then(|()| self.writer.flush());
        // Never replay a partial frame after an error or during writer drop.
        self.commands.clear();
        if result.is_err() {
            // A partial write may have enabled synchronization. Try to close it
            // immediately; the session's separate cleanup descriptor retries too.
            let _ = execute!(self.writer, EndSynchronizedUpdate);
        }
        result
    }
}

pub(super) fn present<W: Write>(
    terminal: &mut Terminal<FrameBackend<W>>,
    render: impl FnOnce(&mut Terminal<FrameBackend<W>>) -> io::Result<()>,
) -> io::Result<()> {
    terminal.backend_mut().begin()?;
    let result = render(terminal);
    let finished = terminal.backend_mut().finish(result.is_ok());
    // Finish and flush even when drawing fails; retain the original draw error.
    result.and(finished)
}

impl<W: Write> Backend for FrameBackend<W> {
    type Error = io::Error;

    fn draw<'a, I>(&mut self, content: I) -> io::Result<()>
    where
        I: Iterator<Item = (u16, u16, &'a Cell)>,
    {
        CrosstermBackend::new(&mut self.commands).draw(content)
    }

    fn hide_cursor(&mut self) -> io::Result<()> {
        self.cursor_visible = false;
        queue!(self.commands, Hide)
    }

    fn show_cursor(&mut self) -> io::Result<()> {
        self.cursor_visible = true;
        Ok(())
    }

    fn get_cursor_position(&mut self) -> io::Result<Position> {
        // Native fullscreen rendering must never issue a global cursor query,
        // which Crossterm would write to stdout rather than our /dev/tty.
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "Cursor queries are unavailable on the buffered terminal",
        ))
    }

    fn set_cursor_position<P: Into<Position>>(&mut self, position: P) -> io::Result<()> {
        let Position { x, y } = position.into();
        queue!(self.commands, MoveTo(x, y))
    }

    fn clear(&mut self) -> io::Result<()> {
        CrosstermBackend::new(&mut self.commands).clear()
    }

    fn clear_region(&mut self, clear_type: ClearType) -> io::Result<()> {
        CrosstermBackend::new(&mut self.commands).clear_region(clear_type)
    }

    fn size(&self) -> io::Result<Size> {
        CrosstermBackend::new(io::sink()).size()
    }

    fn window_size(&mut self) -> io::Result<WindowSize> {
        CrosstermBackend::new(io::sink()).window_size()
    }

    fn flush(&mut self) -> io::Result<()> {
        // Crossterm's execute! calls also flush. They only touch the Vec; the
        // complete frame, cursor, and synchronization end are sent by finish().
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{TerminalOptions, Viewport, layout::Rect};

    #[derive(Default)]
    struct Output {
        bytes: Vec<u8>,
        writes: usize,
        flushes: usize,
        fail_write: bool,
        fail_next_write: bool,
        fail_flush: bool,
    }

    impl Write for Output {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.writes += 1;
            if std::mem::take(&mut self.fail_next_write) {
                return Err(io::Error::other("frame write failed"));
            }
            if std::mem::take(&mut self.fail_write) {
                // Model a frame that reached the terminal only through its
                // synchronization header before the next write fails.
                let written = b"\x1b[?2026h".len().min(bytes.len());
                self.bytes.extend_from_slice(&bytes[..written]);
                self.fail_next_write = true;
                return Ok(written);
            }
            self.bytes.extend_from_slice(bytes);
            Ok(bytes.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            self.flushes += 1;
            if std::mem::take(&mut self.fail_flush) {
                return Err(io::Error::other("frame flush failed"));
            }
            Ok(())
        }
    }

    fn terminal(output: Output) -> Terminal<FrameBackend<Output>> {
        Terminal::with_options(
            FrameBackend::new(output),
            TerminalOptions {
                viewport: Viewport::Fixed(Rect::new(0, 0, 20, 3)),
            },
        )
        .unwrap()
    }

    #[test]
    fn frame_moves_caret_before_showing_and_writes_once() {
        let mut terminal = terminal(Output::default());
        present(&mut terminal, |terminal| {
            terminal.draw(|frame| {
                frame.render_widget("paint", Rect::new(0, 2, 20, 1));
                frame.set_cursor_position((3, 0));
            })?;
            // Ratatui and Crossterm flush during draw, but presentation waits
            // until the caret and synchronization end have joined the frame.
            assert_eq!(terminal.backend().writer.writes, 0);
            Ok(())
        })
        .unwrap();
        let output = &terminal.backend().writer;
        assert_eq!((output.writes, output.flushes), (1, 1));
        assert!(output.bytes.starts_with(b"\x1b[?2026h\x1b[?25l"));
        assert!(output.bytes.ends_with(b"\x1b[1;4H\x1b[?25h\x1b[?2026l"));
    }

    #[test]
    fn failed_render_closes_synchronization_and_keeps_its_error() {
        let mut terminal = terminal(Output {
            fail_flush: true,
            ..Output::default()
        });
        let error = present(&mut terminal, |terminal| {
            terminal.try_draw(|frame| {
                frame.render_widget("unfinished", Rect::new(0, 0, 20, 1));
                Err(io::Error::other("render failed"))
            })?;
            Ok(())
        })
        .unwrap_err();
        assert_eq!(error.to_string(), "render failed");
        let backend = terminal.backend();
        assert!(backend.writer.bytes.ends_with(b"\x1b[?2026l"));
        assert!(
            !backend
                .writer
                .bytes
                .windows(6)
                .any(|bytes| bytes == b"\x1b[?25h")
        );
        assert!(backend.writer.flushes > 0);
        assert!(backend.commands.is_empty());
    }

    #[test]
    fn failed_output_closes_synchronization_and_never_replays_frame() {
        for fail_write in [true, false] {
            let mut terminal = terminal(Output {
                fail_write,
                fail_flush: !fail_write,
                ..Output::default()
            });
            let error = present(&mut terminal, |terminal| {
                terminal.draw(|frame| {
                    frame.render_widget("old frame", Rect::new(0, 0, 20, 1));
                })?;
                Ok(())
            })
            .unwrap_err();
            assert_eq!(
                error.to_string(),
                if fail_write {
                    "frame write failed"
                } else {
                    "frame flush failed"
                }
            );
            let backend = terminal.backend();
            assert!(backend.commands.is_empty());
            assert!(backend.writer.bytes.ends_with(b"\x1b[?2026l"));
            let previous = backend.writer.bytes.len();
            present(&mut terminal, |_| Ok(())).unwrap();
            assert!(
                !terminal.backend().writer.bytes[previous..]
                    .windows(9)
                    .any(|bytes| bytes == b"old frame")
            );
        }
    }
}
