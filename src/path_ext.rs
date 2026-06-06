use std::{
    os::windows::ffi::OsStrExt,
    path::{Path, PathBuf},
};
use windows::Win32::Foundation::MAX_PATH;

const WIN32_FILE_NAMESPACE_UNC: &[u8] = br"\\?\unc\";

pub(crate) trait PathExt {
    /// Check if the `path` is the [Win32 File Namespaces].
    ///
    /// [Win32 File Namespaces]: https://learn.microsoft.com/en-us/windows/win32/fileio/naming-a-file#win32-file-namespaces
    fn is_win32_file_namespace_unc(&self) -> bool;

    /// Convert the [Win32 File Namespaces] to UNC.
    ///
    /// [Win32 File Namespaces]: https://learn.microsoft.com/en-us/windows/win32/fileio/naming-a-file#win32-file-namespaces
    fn unc_from_win32_file_namespace(&self, disallow_long: bool) -> Option<PathBuf>;

    fn is_wide_longer_than(&self, max: u32) -> bool;

    fn to_wide_vec_with_nul(&self) -> Vec<u16>;
}

impl PathExt for Path {
    fn is_win32_file_namespace_unc(&self) -> bool {
        let bytes = self.as_os_str().as_encoded_bytes();
        bytes
            .to_ascii_lowercase()
            .starts_with(WIN32_FILE_NAMESPACE_UNC)
    }

    fn unc_from_win32_file_namespace(&self, disallow_long: bool) -> Option<PathBuf> {
        if !self.is_win32_file_namespace_unc() {
            return None;
        }
        const PREFIX: &[u8] = br"\\";
        const LEN_SUB: usize = WIN32_FILE_NAMESPACE_UNC.len() - PREFIX.len();
        if disallow_long && self.is_wide_longer_than(MAX_PATH + LEN_SUB as u32) {
            return None;
        }
        let bytes = self.as_os_str().as_encoded_bytes();
        let mut result_bytes = Vec::with_capacity(bytes.len() - LEN_SUB);
        result_bytes.extend_from_slice(PREFIX);
        result_bytes.extend_from_slice(&bytes[WIN32_FILE_NAMESPACE_UNC.len()..]);
        assert_eq!(result_bytes.len(), bytes.len() - LEN_SUB);
        let os_str = unsafe { std::ffi::OsStr::from_encoded_bytes_unchecked(&result_bytes) };
        Some(Path::new(os_str).to_path_buf())
    }

    fn is_wide_longer_than(&self, mut max: u32) -> bool {
        for _ in self.as_os_str().encode_wide() {
            if max == 0 {
                return true;
            }
            max -= 1;
        }
        false
    }

    fn to_wide_vec_with_nul(&self) -> Vec<u16> {
        self.as_os_str().encode_wide().chain(Some(0)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_win32_file_namespace_unc() {
        assert!(Path::new(r"\\?\UNC\server\share\dir").is_win32_file_namespace_unc());
        assert!(Path::new(r"\\?\unc\server\share\dir").is_win32_file_namespace_unc());
        assert!(Path::new(r"\\?\UnC\server\share\dir").is_win32_file_namespace_unc());

        assert!(!Path::new(r"\\?\UNC").is_win32_file_namespace_unc());
        assert!(!Path::new(r"\\?\server\share\dir").is_win32_file_namespace_unc());
        assert!(!Path::new(r"\\server\share\dir").is_win32_file_namespace_unc());
        assert!(!Path::new("/a/b").is_win32_file_namespace_unc());
    }

    #[test]
    fn unc_from_win32_file_namespace() {
        assert_eq!(
            Path::new(r"C:\foo").unc_from_win32_file_namespace(false),
            None
        );
        assert_eq!(
            Path::new(r"\\?\UNC\server\share\dir").unc_from_win32_file_namespace(false),
            Some(PathBuf::from(r"\\server\share\dir"))
        );
    }

    #[test]
    fn unc_from_win32_file_namespace_long() {
        const PREFIX: &str = r"\\?\UNC\";
        const UNC_PREFIX: &str = r"\\";
        const SERVER_SHARE: &str = r"server\share\";
        const PATH_MAX: usize = MAX_PATH as usize - UNC_PREFIX.len() - SERVER_SHARE.len();
        let max_src = PREFIX.to_string() + SERVER_SHARE + &"1".repeat(PATH_MAX);
        assert_eq!(
            Path::new(&max_src).unc_from_win32_file_namespace(true),
            Some(PathBuf::from(
                &(UNC_PREFIX.to_string() + SERVER_SHARE + &"1".repeat(PATH_MAX))
            ))
        );
        let too_long_src = PREFIX.to_string() + SERVER_SHARE + &"1".repeat(PATH_MAX + 1);
        assert_eq!(
            Path::new(&too_long_src).unc_from_win32_file_namespace(true),
            None
        );
        assert_eq!(
            Path::new(&too_long_src).unc_from_win32_file_namespace(false),
            Some(PathBuf::from(
                &(UNC_PREFIX.to_string() + SERVER_SHARE + &"1".repeat(PATH_MAX + 1))
            ))
        );
    }

    #[test]
    fn is_wide_longer_than() {
        assert!(!Path::new("").is_wide_longer_than(10));

        assert!(!Path::new(&"1".repeat(9)).is_wide_longer_than(10));
        assert!(!Path::new(&"1".repeat(10)).is_wide_longer_than(10));
        assert!(Path::new(&"1".repeat(11)).is_wide_longer_than(10));

        assert!(!Path::new(&"\u{3042}".repeat(9)).is_wide_longer_than(10));
        assert!(!Path::new(&"\u{3042}".repeat(10)).is_wide_longer_than(10));
        assert!(Path::new(&"\u{3042}".repeat(11)).is_wide_longer_than(10));
    }

    #[test]
    fn to_wide_vec_with_nul() {
        assert_eq!(Path::new("AB").to_wide_vec_with_nul(), vec![0x41, 0x42, 0]);
        assert_eq!(
            Path::new("\u{3042}\u{3043}").to_wide_vec_with_nul(),
            vec![0x3042, 0x3043, 0]
        );
    }
}
