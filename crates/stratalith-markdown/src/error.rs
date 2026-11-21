use std::fmt;
use stratalith_core::{DocumentError, PositionError};

#[derive(Debug)]
pub enum MarkdownError {
    Document(DocumentError),
    Position(PositionError),
    NestingLimit,
    Unrepresentable(&'static str),
}

impl From<DocumentError> for MarkdownError {
    fn from(error: DocumentError) -> Self {
        Self::Document(error)
    }
}

impl fmt::Display for MarkdownError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Document(error) => error.fmt(f),
            Self::Position(error) => error.fmt(f),
            Self::NestingLimit => f.write_str("Markdown nesting exceeds 128 levels"),
            Self::Unrepresentable(reason) => f.write_str(reason),
        }
    }
}

impl std::error::Error for MarkdownError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Document(error) => Some(error),
            Self::Position(error) => Some(error),
            _ => None,
        }
    }
}

pub(crate) fn check_depth(depth: usize) -> Result<(), MarkdownError> {
    if depth > 128 {
        Err(MarkdownError::NestingLimit)
    } else {
        Ok(())
    }
}
