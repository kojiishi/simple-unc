//! Please see the [README] for a high-level overview,
//! and the [`SimpleUnc`] struct for the detailed features.
//!
//! [README]: https://github.com/kojiishi/simple-unc

mod display;
#[cfg(windows)]
mod drive_path;
#[cfg(windows)]
mod path_ext;
mod simple_unc;
#[cfg(windows)]
mod volume;

pub use display::Display;
#[cfg(windows)]
pub(crate) use drive_path::DrivePath;
#[cfg(windows)]
pub(crate) use path_ext::PathExt;
pub use simple_unc::SimpleUnc;
#[cfg(windows)]
pub(crate) use volume::Volumes;
