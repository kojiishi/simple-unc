#![cfg_attr(not(target_os = "windows"), allow(unused))]
use crate::Display;
#[cfg(windows)]
use crate::{PathExt, Volumes};
use std::{
    borrow::Cow,
    fs, io,
    path::{Path, PathBuf, StripPrefixError},
};

/// Simplifies [Win32 File Namespaces] paths (the "`\\?\`" prefix)
/// for better readability and compatibility.
///
/// ```no_run
/// # use simple_unc::SimpleUnc;
/// # let path = "";
/// SimpleUnc::default().canonicalize(path);
/// ```
/// is a snap-in replacement of [`fs::canonicalize`].
///
/// | | `C:\dir` | `Z:\x` (network) |
/// | --- | --- | --- |
/// | [`fs::canonicalize`] | `\\?\C:\dir` | `\\?\UNC\server\share\x` |
/// | `SimpleUnc` | `C:\dir` | `\\server\share\x` |
/// | `SimpleUnc` with [`map_to_drive`] | `C:\dir` | `Z:\x` |
///
/// [`map_to_drive`]: `SimpleUnc::map_to_drive`
/// [Win32 File Namespaces]: https://learn.microsoft.com/en-us/windows/win32/fileio/naming-a-file#win32-file-namespaces
#[derive(Debug, Default)]
pub struct SimpleUnc {
    /// When set to `true`,
    /// the simplification is disabled
    /// if the result is a "long path" (longer than 260 characters).
    ///
    /// Long paths may not be supported by some programs and APIs.
    /// In such cases, the [Win32 File Namespaces] (the "`\\?\`" prefix)
    /// may be able to work around the limitation.
    ///
    /// On the other hand,
    /// other programs such as PowerShell v7 can't handle the "`\\?\`" prefix,
    /// but it can handle long paths.
    ///
    /// [Win32 File Namespaces]: https://learn.microsoft.com/en-us/windows/win32/fileio/naming-a-file#win32-file-namespaces
    pub disallow_long: bool,

    /// Map to network share drive names when possible.
    /// ```
    /// # use simple_unc::SimpleUnc;
    /// # fn test() -> std::io::Result<()> {
    /// let path = "file.txt";
    /// let unc = SimpleUnc { map_to_drive: true, ..Default::default() };
    /// let canonicalized = unc.canonicalize(path)?;
    /// # Ok(())
    /// # }
    /// ```
    /// If the `file.txt` is in a network drive,
    /// the result is `Z:\dir\file.txt`
    /// instead of `\\server\share\dir\file.txt`.
    ///
    /// The following code tries to preserve the original form of the `path`.
    /// ```
    /// # use simple_unc::SimpleUnc;
    /// # fn test(path: &std::path::Path) -> std::io::Result<()> {
    /// SimpleUnc {
    ///     map_to_drive: !path.as_os_str().as_encoded_bytes().starts_with(br"\\"),
    ///     ..Default::default()
    /// }.canonicalize(path)?;
    /// # Ok(())
    /// # }
    /// ```
    pub map_to_drive: bool,

    /// The [`dunce`] simplification is applied by default.
    /// Set to `true` to skip it.
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

    #[cfg(all(test, windows))]
    volumes: Option<Volumes>,
}

impl SimpleUnc {
    /// Calls [`fs::canonicalize`] and [`simplify`].
    ///
    /// On other platforms than Windows,
    /// this is equivalent to [`fs::canonicalize`].
    ///
    /// [`fs::canonicalize`]: https://doc.rust-lang.org/std/fs/fn.canonicalize.html
    /// [`simplify`]: SimpleUnc::simplify
    pub fn canonicalize(&self, path: impl AsRef<Path>) -> io::Result<PathBuf> {
        let canonicalized = fs::canonicalize(path)?;
        #[cfg(windows)]
        if let Some(simplified) = self.simplify(&canonicalized)? {
            return Ok(simplified.into_owned());
        }
        Ok(canonicalized)
    }

    /// Try to simplify the given `path`.
    ///
    /// Returns `Ok(None)`
    /// if no simplification is applied,
    /// or on other platforms than Windows.
    pub fn simplify<'a>(&self, path: &'a Path) -> io::Result<Option<Cow<'a, Path>>> {
        #[cfg(windows)]
        return self._simplify(path).map_err(io_error_from_anyhow);
        #[cfg(not(windows))]
        Ok(None)
    }

    #[cfg(windows)]
    fn _simplify<'a>(&self, path: &'a Path) -> anyhow::Result<Option<Cow<'a, Path>>> {
        // Try mapped network share drives.
        if let Some(drive_path) = self.drive_path(path)? {
            if self.map_to_drive
                && drive_path.has_drive()
                && (!self.disallow_long || !drive_path.is_win32_long_path())
            {
                return Ok(Some(Cow::Owned(drive_path.to_path_buf())));
            }
            if let Some(unc) = path.unc_from_win32_file_namespace(self.disallow_long) {
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

    #[cfg(windows)]
    fn drive_path<'a>(&self, path: &'a Path) -> anyhow::Result<Option<crate::DrivePath<'a>>> {
        #[cfg(test)]
        if let Some(volumes) = &self.volumes {
            return Ok(volumes._drive_path(path));
        }
        Volumes::drive_path(path)
    }

    /// Refreshes the cached information.
    pub fn refresh() -> io::Result<()> {
        #[cfg(windows)]
        Volumes::refresh().map_err(io_error_from_anyhow)?;
        Ok(())
    }

    /// Returns an object that implements [`Display`][`core::fmt::Display`].
    ///
    /// # Examples
    ///
    /// ```
    /// # use std::path::Path;
    /// # use simple_unc::SimpleUnc;
    /// # fn test() -> std::io::Result<()> {
    /// let path = Path::new("file").canonicalize()?;
    /// println!("{}", SimpleUnc::default().display(&path));
    /// # Ok(())
    /// # }
    /// ```
    pub fn display<'a>(&'a self, path: &'a Path) -> Display<'a> {
        Display::new(self, path)
    }

    /// A snap-in replacement for [`Path::strip_prefix`]
    /// with a fix for [a leading "`\`" left for UNC paths
    /// on Windows](https://github.com/rust-lang/rust/issues/155183).
    pub fn strip_prefix(path: &Path, base: impl AsRef<Path>) -> Result<&Path, StripPrefixError> {
        #[cfg(windows)]
        return PathExt::strip_prefix_fix(path, base);
        #[cfg(not(windows))]
        path.strip_prefix(base)
    }

    #[cfg(all(test, windows))]
    pub(crate) fn mock_with_drive() -> SimpleUnc {
        SimpleUnc {
            volumes: Some(Volumes::mock()),
            ..Default::default()
        }
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
    fn simplify_not_simplified() {
        let unc = SimpleUnc::default();
        assert_eq!(unc.simplify(Path::new(r"C:\foo")).unwrap(), None);
    }

    #[cfg(windows)]
    #[test]
    fn simplify_drive_not_simplified() {
        let unc = SimpleUnc::mock_with_drive();
        assert_eq!(unc.simplify(Path::new(r"C:\foo")).unwrap(), None);
    }

    #[cfg(windows)]
    #[test]
    fn simplify_drive_unc() {
        let mut unc = SimpleUnc::mock_with_drive();
        let path = Path::new(r"\\?\UNC\server\share\foo");
        let path2 = Path::new(r"\\?\UNC\server2\share2\foo2");
        assert_eq!(
            unc.simplify(path).unwrap(),
            Some(Cow::Owned(PathBuf::from(r"\\server\share\foo")))
        );
        assert_eq!(
            unc.simplify(path2).unwrap(),
            Some(Cow::Owned(PathBuf::from(r"\\server2\share2\foo2")))
        );

        unc.map_to_drive = true;
        assert_eq!(
            unc.simplify(path).unwrap(),
            Some(Cow::Owned(PathBuf::from(r"X:\foo")))
        );
        assert_eq!(
            unc.simplify(path2).unwrap(),
            Some(Cow::Owned(PathBuf::from(r"Z:\foo2")))
        );
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

    #[cfg(windows)]
    #[test]
    fn simplify_unmapped_connected_share() {
        let mut unc = SimpleUnc::mock_with_drive();
        let path = Path::new(r"\\?\UNC\server0\share0\foo");
        assert_eq!(
            unc.simplify(path).unwrap(),
            Some(Cow::Owned(PathBuf::from(r"\\server0\share0\foo")))
        );

        // Even with map_to_drive = true, it should simplify to the UNC path,
        // because the drive letter is '\0'.
        unc.map_to_drive = true;
        assert_eq!(
            unc.simplify(path).unwrap(),
            Some(Cow::Owned(PathBuf::from(r"\\server0\share0\foo")))
        );
    }
}
