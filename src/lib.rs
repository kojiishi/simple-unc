//! Please see the [README] for a high-level overview,
//! and the [`SimpleUnc`] struct for the detailed features.
//!
//! [README]: https://github.com/kojiishi/simple-unc

mod display;
#[cfg(windows)]
mod drive_path;
#[cfg(windows)]
mod drives;
#[cfg(windows)]
mod path_ext;
mod simple_unc;

pub use display::Display;
#[cfg(windows)]
pub(crate) use drive_path::DrivePath;
#[cfg(windows)]
pub(crate) use drives::Drives;
#[cfg(windows)]
pub(crate) use path_ext::PathExt;
pub use simple_unc::SimpleUnc;
