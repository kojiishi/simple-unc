use crate::SimpleUnc;
use std::{
    fmt,
    path::{self, Path},
};

/// Helper struct for safely printing paths with [`format!`] and `{}`.
///
/// Please see [`SimpleUnc::display`].
#[derive(Debug)]
pub struct Display<'a> {
    #[cfg(windows)]
    unc: &'a SimpleUnc,
    path: &'a Path,
}

impl<'a> Display<'a> {
    #[cfg(windows)]
    pub(crate) fn new(unc: &'a SimpleUnc, path: &'a Path) -> Self {
        Self { unc, path }
    }
    #[cfg(not(windows))]
    pub(crate) fn new(_unc: &'a SimpleUnc, path: &'a Path) -> Self {
        Self { path }
    }
}

impl fmt::Display for Display<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        #[cfg(windows)]
        if let Ok(Some(simplified)) = self.unc.simplify(self.path) {
            return path::Display::fmt(&simplified.display(), f);
        };
        path::Display::fmt(&self.path.display(), f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_not_simplified() {
        let unc = SimpleUnc::default();
        let path = Path::new("foo");
        assert_eq!(format!("{}", unc.display(path)), "foo");
    }

    #[cfg(windows)]
    #[test]
    fn display_drive_unc() {
        let mut unc = SimpleUnc::mock_with_drive();
        let path = Path::new(r"\\?\UNC\server\share\foo");
        assert_eq!(format!("{}", unc.display(path)), r"\\server\share\foo");

        unc.map_to_drive = true;
        assert_eq!(format!("{}", unc.display(path)), r"X:\foo");
    }
}
