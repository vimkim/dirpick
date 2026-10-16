//! Requested external effects. Native paths remain data until the transport.
use base64::{Engine, engine::general_purpose::STANDARD};
use std::{
    ffi::{OsStr, OsString},
    fs::File,
    io::{self, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    CopyPath { path: PathBuf },
    OpenEditor { path: PathBuf, line: usize },
}
impl Action {
    /// Emit to the terminal channel, never the JSON/stdout channel. Success
    /// proves a flushed write, not acceptance by the terminal's clipboard.
    pub fn write_to(&self, terminal: &mut impl Write) -> io::Result<()> {
        match self {
            Self::OpenEditor { .. } => Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Editor actions require terminal handoff",
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

/// A parsed editor invocation: every argument remains literal process data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorCommand {
    pub program: OsString,
    pub args: Vec<OsString>,
}
impl EditorCommand {
    pub fn from_environment(path: &Path, line: usize, require_line: bool) -> io::Result<Self> {
        Self::resolve(
            std::env::var_os("VISUAL").as_deref(),
            std::env::var_os("EDITOR").as_deref(),
            path,
            line,
            require_line,
        )
    }
    /// Generic commands can open file rows. Content actions require a supported
    /// location syntax even at line one, rather than relying on editor history.
    pub fn resolve(
        visual: Option<&OsStr>,
        editor: Option<&OsStr>,
        path: &Path,
        line: usize,
        require_line: bool,
    ) -> io::Result<Self> {
        let configured = visual
            .filter(|value| !value.is_empty())
            .or(editor.filter(|value| !value.is_empty()))
            .unwrap_or(OsStr::new("nvim"));
        let configured = configured.to_str().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "Editor command must be UTF-8; set VISUAL or EDITOR",
            )
        })?;
        let words = shlex::split(configured).filter(|words| !words.is_empty() && !words[0].is_empty()).ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "Invalid editor quoting or empty command; set VISUAL or EDITOR to an executable with literal arguments"))?;
        let program = OsString::from(&words[0]);
        let vim = Path::new(&program)
            .file_name()
            .is_some_and(|name| name == "vim" || name == "nvim");
        if require_line && !vim {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "Content locations require Vim/Neovim line syntax; set VISUAL or EDITOR to vim or nvim (literal arguments allowed)",
            ));
        }
        let mut args: Vec<OsString> = words[1..].iter().map(OsString::from).collect();
        if vim {
            args.extend([format!("+{}", line.max(1)).into(), "--".into()]);
        }
        args.push(path.as_os_str().to_owned());
        Ok(Self { program, args })
    }
    /// All three child streams attach to the controlling terminal. stdout is
    /// reserved for the shell action protocol, even when the editor prints.
    pub fn run(&self, terminal: &File) -> io::Result<()> {
        let status = Command::new(&self.program)
            .args(&self.args)
            .stdin(Stdio::from(terminal.try_clone()?))
            .stdout(Stdio::from(terminal.try_clone()?))
            .stderr(Stdio::from(terminal.try_clone()?))
            .status()?;
        if status.success() {
            Ok(())
        } else {
            Err(io::Error::other(format!("Editor exited with {status}")))
        }
    }
}
