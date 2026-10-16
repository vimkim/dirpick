//! Requested external effects. Native paths remain data until the transport.
use base64::{Engine, engine::general_purpose::STANDARD};
use std::{
    io::{self, Write},
    path::PathBuf,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    CopyPath { path: PathBuf },
    ChangeDirectory { path: PathBuf },
}
impl Action {
    /// Only the explicit directory action has a JSON representation. Validate
    /// before writing so unsupported native paths never produce a partial record.
    pub fn write_json(&self, output: &mut impl Write) -> io::Result<()> {
        if let Self::ChangeDirectory { path } = self {
            let text = path
                .to_str()
                .filter(|_| path.is_absolute() && path.is_dir())
                .ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "Directory action requires an existing absolute UTF-8 directory",
                    )
                })?;
            serde_json::to_writer(
                &mut *output,
                &serde_json::json!({"action": "cd", "path": text}),
            )?;
            output.write_all(b"\n")?;
            output.flush()?;
        }
        Ok(())
    }
    /// Emit to the terminal channel, never the JSON/stdout channel. Success
    /// proves a flushed write, not acceptance by the terminal's clipboard.
    pub fn write_to(&self, terminal: &mut impl Write) -> io::Result<()> {
        match self {
            Self::ChangeDirectory { .. } => Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "Directory actions require the JSON integration channel",
            )),
            Self::CopyPath { path } => {
                let text = path
                    .to_str()
                    .filter(|_| path.is_absolute())
                    .ok_or_else(|| {
                        io::Error::new(
                            io::ErrorKind::InvalidInput,
                            "OSC 52 requires an absolute UTF-8 path",
                        )
                    })?;
                let encoded = STANDARD.encode(text.as_bytes());
                write!(terminal, "\x1b]52;c;{encoded}\x07")?;
                terminal.flush()
            }
        }
    }
}
