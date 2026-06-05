#![cfg_attr(not(target_os = "windows"), allow(unused))]
#[cfg(windows)]
use crate::{Drives, PathExt};
use std::{
    borrow::Cow,
    fs,
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
}

impl SimpleUnc {
    pub fn canonicalize(&self, path: impl AsRef<Path>) -> anyhow::Result<PathBuf> {
        let canonicalized = fs::canonicalize(path)?;
        #[cfg(windows)]
        if let Some(simplified) = self.simplify(&canonicalized)? {
            return Ok(simplified.into_owned());
        }
        Ok(canonicalized)
    }

    pub fn simplify<'a>(&self, path: &'a Path) -> anyhow::Result<Option<Cow<'a, Path>>> {
        #[cfg(windows)]
        {
            // Try mapped network share drives.
            if let Some(drive_path) = Drives::drive_path(path)? {
                if self.map_to_drive {
                    return Ok(Some(Cow::Owned(drive_path.to_path_buf())));
                }
                if let Some(stripped) = path.strip_win32_file_namespace_unc() {
                    return Ok(Some(Cow::Owned(stripped)));
                }
            }

            // Try `dunce::simplified`.
            let simplified = dunce::simplified(path);
            if !std::ptr::eq(path, simplified) {
                return Ok(Some(Cow::Borrowed(simplified)));
            }
        }
        Ok(None)
    }

    /// Refreshes the cached information.
    pub fn refresh() -> anyhow::Result<()> {
        #[cfg(windows)]
        Drives::refresh()?;
        Ok(())
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
}
