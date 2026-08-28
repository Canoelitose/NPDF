//! Error type shared by every core module.
//!
//! The core never panics on malformed input. Everything a user can trigger by
//! opening a broken file has to end up as one of these variants so the shell can
//! show a helpful message instead of crashing.

use std::path::PathBuf;

/// Convenience alias used throughout the core.
pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("input/output error at {path:?}: {source}")]
    Io {
        path: Option<PathBuf>,
        #[source]
        source: std::io::Error,
    },

    #[error("the file is not a PDF or its structure is broken: {0}")]
    BrokenDocument(String),

    #[error("the document is encrypted and needs a password")]
    PasswordRequired,

    #[error("the supplied password was rejected")]
    WrongPassword,

    #[error("no document is open with id {0}")]
    UnknownDocument(u64),

    #[error("page {index} does not exist, the document has {count} pages")]
    UnknownPage { index: usize, count: usize },

    #[error("object {0}:{1} was not found")]
    MissingObject(u32, u16),

    #[error("the content stream of this page could not be read: {0}")]
    ContentStream(String),

    #[error("font error: {0}")]
    Font(String),

    #[error("the renderer is not available: {0}")]
    RendererUnavailable(String),

    #[error("rendering failed: {0}")]
    Render(String),

    #[error("saving failed: {0}")]
    Save(String),

    #[error("nothing to undo")]
    NothingToUndo,

    #[error("nothing to redo")]
    NothingToRedo,

    #[error("not implemented yet: {0}")]
    NotImplemented(&'static str),

    #[error("invalid argument: {0}")]
    InvalidArgument(String),
}

impl Error {
    /// Stable, machine readable identifier so the frontend can pick a German
    /// message without parsing the English text.
    pub fn code(&self) -> &'static str {
        match self {
            Error::Io { .. } => "io",
            Error::BrokenDocument(_) => "broken_document",
            Error::PasswordRequired => "password_required",
            Error::WrongPassword => "wrong_password",
            Error::UnknownDocument(_) => "unknown_document",
            Error::UnknownPage { .. } => "unknown_page",
            Error::MissingObject(_, _) => "missing_object",
            Error::ContentStream(_) => "content_stream",
            Error::Font(_) => "font",
            Error::RendererUnavailable(_) => "renderer_unavailable",
            Error::Render(_) => "render",
            Error::Save(_) => "save",
            Error::NothingToUndo => "nothing_to_undo",
            Error::NothingToRedo => "nothing_to_redo",
            Error::NotImplemented(_) => "not_implemented",
            Error::InvalidArgument(_) => "invalid_argument",
        }
    }

    /// Build an input/output error that remembers which path failed. Used by
    /// the platform implementations in the shell as well as by the core.
    pub fn io(path: impl Into<Option<PathBuf>>, source: std::io::Error) -> Self {
        Error::Io {
            path: path.into(),
            source,
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(source: std::io::Error) -> Self {
        Error::Io { path: None, source }
    }
}

/// Translate a lopdf error. Password handling is special cased because the shell
/// has to react to it by asking the user, not by showing a failure.
impl From<lopdf::Error> for Error {
    fn from(value: lopdf::Error) -> Self {
        use lopdf::Error as L;
        match value {
            L::InvalidPassword => Error::WrongPassword,
            L::Decryption(_) => Error::PasswordRequired,
            L::ObjectNotFound(id) => Error::MissingObject(id.0, id.1),
            L::IO(err) => Error::Io {
                path: None,
                source: err,
            },
            L::Syntax(msg) => Error::ContentStream(msg),
            other => Error::BrokenDocument(other.to_string()),
        }
    }
}
