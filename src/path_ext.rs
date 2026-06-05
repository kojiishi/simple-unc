use std::{
    os::windows::ffi::OsStrExt,
    path::{Path, PathBuf},
};

const WIN32_FILE_NAMESPACE_UNC: &[u8] = br"\\?\unc\";

pub(crate) trait PathExt {
    /// Check or strip [Win32 File Namespaces].
    ///
    /// [Win32 File Namespaces]: https://learn.microsoft.com/en-us/windows/win32/fileio/naming-a-file#win32-file-namespaces
    fn is_win32_file_namespace_unc(&self) -> bool;
    fn strip_win32_file_namespace_unc(&self) -> Option<PathBuf>;
    fn to_wide_vec_with_nul(&self) -> Vec<u16>;
}

impl PathExt for Path {
    fn is_win32_file_namespace_unc(&self) -> bool {
        let bytes = self.as_os_str().as_encoded_bytes();
        bytes
            .to_ascii_lowercase()
            .starts_with(WIN32_FILE_NAMESPACE_UNC)
    }

    fn strip_win32_file_namespace_unc(&self) -> Option<PathBuf> {
        if self.is_win32_file_namespace_unc() {
            let bytes = self.as_os_str().as_encoded_bytes();
            let mut result_bytes =
                Vec::with_capacity(bytes.len() - WIN32_FILE_NAMESPACE_UNC.len() + 2);
            result_bytes.extend_from_slice(br"\\");
            result_bytes.extend_from_slice(&bytes[WIN32_FILE_NAMESPACE_UNC.len()..]);
            let os_str = unsafe { std::ffi::OsStr::from_encoded_bytes_unchecked(&result_bytes) };
            return Some(Path::new(os_str).to_path_buf());
        }
        None
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
    fn strip_win32_file_namespace_unc() {
        assert_eq!(
            Path::new(r"\\?\UNC\server\share\dir").strip_win32_file_namespace_unc(),
            Some(PathBuf::from(r"\\server\share\dir"))
        );
        assert_eq!(Path::new(r"C:\foo").strip_win32_file_namespace_unc(), None);
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
