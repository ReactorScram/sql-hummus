/// Public-facing opaque error
#[derive(Debug, thiserror::Error)]
#[error(transparent)]
pub struct Error(#[from] ErrorRepr);

pub type Result<T> = std::result::Result<T, Error>;

// FIXME: I'm not 100% about this error setup. If you have gdb, it's redundant. If you don't have gdb, it's better than nothing.
// Unlike anyhow it's only safe Rust, and it's very simple, and it's my code.
#[derive(Debug, thiserror::Error)]
#[error("kind={kind}; cookies={cookies:?}")]
struct ErrorRepr {
    cookies: Vec<&'static str>,
    #[source]
    kind: ErrorKind,
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum ErrorKind {
    #[error(transparent)]
    FromPathBufError(#[from] camino::FromPathBufError),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Error: {0}")]
    Str(&'static str),
    #[error("SQLite error: {0}")]
    Sqlite(#[from] sql_peas::Error),
    #[error("ULID decode error: {0}")]
    UlidDecode(#[from] ulid::DecodeError),
}

impl From<&'static str> for ErrorKind {
    fn from(s: &'static str) -> Self {
        Self::Str(s)
    }
}

impl<T> From<T> for Error
where
    ErrorKind: From<T>,
{
    fn from(value: T) -> Self {
        ErrorRepr {
            cookies: vec![],
            kind: ErrorKind::from(value),
        }
        .into()
    }
}

/// Just like `anyhow::Context` but simpler
///
/// "Type 'cookie', you idiot." -- The Plague, Hackers (film)
pub(crate) trait Cookie<T> {
    fn cookie(self, s: &'static str) -> Result<T>;
}

impl<T, E> Cookie<T> for std::result::Result<T, E>
where
    Error: From<E>,
{
    fn cookie(self, s: &'static str) -> Result<T> {
        match self {
            Ok(x) => Ok(x),
            Err(e) => {
                let mut e = Error::from(e);
                e.0.cookies.push(s);
                Err(e)
            }
        }
    }
}
