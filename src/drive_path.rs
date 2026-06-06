use crate::PathExt;
use std::path::{Path, PathBuf};
use windows::Win32::Foundation::MAX_PATH;

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct DrivePath<'a> {
    drive_letter: char,
    path: &'a Path,
}

impl<'a> DrivePath<'a> {
    pub(crate) fn new(drive_letter: char, path: &'a Path) -> Self {
        Self { drive_letter, path }
    }

    pub(crate) fn has_drive(&self) -> bool {
        self.drive_letter != '\0'
    }

    pub(crate) fn is_win32_long_path(&self) -> bool {
        const PREFIX_LEN: u32 = r"A:\".len() as u32;
        self.path.is_wide_longer_than(MAX_PATH - PREFIX_LEN)
    }

    pub(crate) fn to_path_buf(&self) -> PathBuf {
        assert!(self.has_drive());
        let mut drive_path = PathBuf::from(format!(r"{}:\", self.drive_letter));
        drive_path.push(self.path);
        drive_path
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn to_path_buf() {
        assert_eq!(
            DrivePath::new('X', Path::new(r"dir\file.txt")).to_path_buf(),
            PathBuf::from(r"X:\dir\file.txt")
        );
    }

    #[test]
    fn to_path_buf_abs() {
        // `strip_prefix` may leave the leading `\`.
        // https://github.com/rust-lang/rust/issues/155183
        assert_eq!(
            DrivePath::new('X', Path::new(r"\dir\file.txt")).to_path_buf(),
            PathBuf::from(r"X:\dir\file.txt")
        );
    }
}
