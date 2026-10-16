//! Requested external effects. Native paths remain data until the transport.
use base64::{Engine, engine::general_purpose::STANDARD};
use std::{
    io::{self, Write},
    path::PathBuf,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    CopyPath { path: PathBuf },
}
impl Action {
    /// Emit to the terminal channel, never the JSON/stdout channel. Success
    /// proves a flushed write, not acceptance by the terminal's clipboard.
    pub fn write_to(&self, terminal: &mut impl Write) -> io::Result<()> {
        match self {
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
