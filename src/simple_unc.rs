#![cfg_attr(not(target_os = "windows"), allow(unused))]
#[cfg(windows)]
use crate::Drives;
use std::{
    borrow::Cow,
    fs,
    path::{Path, PathBuf},
};

#[derive(Default)]
pub struct SimpleUnc {}

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
                return Ok(Some(Cow::Owned(drive_path.to_path_buf())));
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

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn simplify_none() {
        let unc = SimpleUnc::default();
        assert_eq!(unc.simplify(Path::new(r"C:\foo")).unwrap(), None);
    }

    #[test]
    fn simplify_dunce() {
        let unc = SimpleUnc::default();
        assert_eq!(
            unc.simplify(Path::new(r"\\?\C:\foo")).unwrap(),
            Some(Cow::Borrowed(Path::new(r"C:\foo")))
        );
    }
}
