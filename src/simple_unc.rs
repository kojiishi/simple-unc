#![cfg_attr(not(target_os = "windows"), allow(unused))]
#[cfg(windows)]
use crate::{Drives, PathExt};
use std::{
    borrow::Cow,
    fs, io,
    path::{Path, PathBuf},
};

#[derive(Default)]
pub struct SimpleUnc {
    /// Map to the network share drive when possible.
    /// ```
    /// # use simple_unc::SimpleUnc;
    /// let path = "file.txt";
    /// let unc = SimpleUnc { map_to_drive: true, ..Default::default() };
    /// let canonicalized = unc.canonicalize(path);
    /// ```
    /// If the `file.txt` is in a network drive,
    /// the result should be `Z:\dir\file.txt`
    /// instead of `\\server\share\dir\file.txt`.
    pub map_to_drive: bool,

    /// Skip the [`dunce`] crate simplification.
    ///
    /// [`dunce`]: https://crates.io/crates/dunce
    pub skip_dunce: bool,

    /// It is highly recommended to always use `, ..Default::default()`.
    /// Otherwise builds fail when new fields are added.
    ///
    /// This field is not used in any ways,
    /// but exists to allow using `, ..Default::default()`
    /// even when all other fields are specified.
    pub _unused: bool,
}

impl SimpleUnc {
    pub fn canonicalize(&self, path: impl AsRef<Path>) -> io::Result<PathBuf> {
        let canonicalized = fs::canonicalize(path)?;
        #[cfg(windows)]
        if let Some(simplified) = self.simplify(&canonicalized)? {
            return Ok(simplified.into_owned());
        }
        Ok(canonicalized)
    }

    pub fn simplify<'a>(&self, path: &'a Path) -> io::Result<Option<Cow<'a, Path>>> {
        #[cfg(windows)]
        return self._simplify(path).map_err(io_error_from_anyhow);
        #[cfg(not(windows))]
        Ok(None)
    }

    #[cfg(windows)]
    fn _simplify<'a>(&self, path: &'a Path) -> anyhow::Result<Option<Cow<'a, Path>>> {
        // Try mapped network share drives.
        if let Some(drive_path) = Drives::drive_path(path)? {
            if self.map_to_drive {
                return Ok(Some(Cow::Owned(drive_path.to_path_buf())));
            }
            if let Some(unc) = path.unc_from_win32_file_namespace() {
                return Ok(Some(Cow::Owned(unc)));
            }
        }

        if !self.skip_dunce {
            // Try `dunce::simplified`.
            let simplified = dunce::simplified(path);
            if !std::ptr::eq(path, simplified) {
                return Ok(Some(Cow::Borrowed(simplified)));
            }
        }
        Ok(None)
    }

    /// Refreshes the cached information.
    pub fn refresh() -> io::Result<()> {
        #[cfg(windows)]
        Drives::refresh().map_err(io_error_from_anyhow)?;
        Ok(())
    }
}

fn io_error_from_anyhow(error: anyhow::Error) -> io::Error {
    match error.downcast::<io::Error>() {
        Ok(io_error) => io_error,
        Err(other_error) => io::Error::other(other_error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simplify_none() {
        let unc = SimpleUnc::default();
        assert_eq!(unc.simplify(Path::new(r"C:\foo")).unwrap(), None);
    }

    #[cfg(windows)]
    #[test]
    fn simplify_dunce() {
        let unc = SimpleUnc::default();
        assert_eq!(
            unc.simplify(Path::new(r"\\?\C:\foo")).unwrap(),
            Some(Cow::Borrowed(Path::new(r"C:\foo")))
        );
    }

    #[cfg(windows)]
    #[test]
    fn simplify_dunce_skip() {
        let unc = SimpleUnc {
            skip_dunce: true,
            ..Default::default()
        };
        assert_eq!(unc.simplify(Path::new(r"\\?\C:\foo")).unwrap(), None);
    }
}
