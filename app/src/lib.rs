//! Terminal outliner for TaskPaper files.
//!
//! Three layers with dependencies pointing inward: [`ui`] depends on [`app`], which depends on
//! [`document`]. The document and app layers form a core that knows nothing about the terminal.

pub mod app;
pub mod document;
pub mod ui;
