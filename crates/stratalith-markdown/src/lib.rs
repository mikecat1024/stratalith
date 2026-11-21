//! Markdown I/O for the editor's document model.
//!
//! The block pass builds a tree and collects link reference definitions. The
//! inline pass resolves labels and emphasis inside paragraphs and headings.
//! Raw HTML and code are literal leaves. Serialization preserves the meaning
//! of the document, including link targets, and chooses fresh Markdown markers.
//! The test suite includes the 652 examples from CommonMark 0.31.2.
//!
//! ```
//! let source = "# Editor\n\n- **Hello *world***";
//! let expected = format!("{source}\n");
//! let doc = stratalith_markdown::parse(source)?;
//!
//! let markdown = stratalith_markdown::serialize(&doc)?;
//!
//! assert_eq!(markdown, expected);
//! # Ok::<(), stratalith_markdown::MarkdownError>(())
//! ```

mod error;
mod escape;
mod heading;
mod parser;
mod paste;
mod serialize;

pub use error::MarkdownError;
pub use parser::parse;
pub use paste::paste_command;
pub use serialize::{serialize, serialize_fragment};

#[cfg(test)]
mod spec;
