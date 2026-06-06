use crate::{DrivePath, PathExt};
use std::{
    cmp::Ordering,
    fs,
    path::{Path, PathBuf},
    sync::{LazyLock, Mutex},
};
use windows::{
    Win32::Storage::FileSystem::{GetDriveTypeW, GetLogicalDrives},
    core::PCWSTR,
};

const DRIVE_REMOTE: u32 = 4;

static DRIVES: LazyLock<Mutex<Option<Drives>>> = LazyLock::new(|| Mutex::new(None));

#[derive(Clone, Debug)]
pub(crate) struct Drives {
    drives: Vec<(char, PathBuf)>,
}

impl Drives {
    fn new() -> anyhow::Result<Self> {
        Ok(Self::with_drives(Self::get_remote_drives()?))
    }

    pub(crate) fn with_drives(mut drives: Vec<(char, PathBuf)>) -> Self {
        drives = Self::sort_drives(drives);
        Self { drives }
    }

    /// The shortest path wins.
    /// If the lengths are the same, the lowest drive letter wins.
    fn sort_drives(mut drives: Vec<(char, PathBuf)>) -> Vec<(char, PathBuf)> {
        drives.sort_by(|a, b| {
            let mut cmp = a.1.as_os_str().len().cmp(&b.1.as_os_str().len());
            if cmp == Ordering::Equal {
                cmp = a.0.cmp(&b.0);
            }
            cmp
        });
        drives
    }

    pub(crate) fn refresh() -> anyhow::Result<()> {
        Drives::new()?.set_to_cache();
        Ok(())
    }

    fn set_to_cache(self) {
        let mut cache = DRIVES.lock().unwrap();
        *cache = Some(self);
    }

    pub(crate) fn drive_path<'a>(path: &'a Path) -> anyhow::Result<Option<DrivePath<'a>>> {
        let drives = {
            let mut cache = DRIVES.lock().unwrap();
            if cache.is_none() {
                *cache = Some(Drives::new()?);
            }
            cache.as_ref().cloned().unwrap()
        };
        Ok(drives._drive_path(path))
    }

    pub(crate) fn _drive_path<'a>(&self, path: &'a Path) -> Option<DrivePath<'a>> {
        for (drive_letter, root) in &self.drives {
            if let Ok(suffix) = path.strip_prefix_fix(root) {
                return Some(DrivePath::new(*drive_letter, suffix));
            }
        }
        None
    }

    fn get_remote_drives() -> anyhow::Result<Vec<(char, PathBuf)>> {
        let mut drive_mask = unsafe { GetLogicalDrives() };
        if drive_mask == 0 {
            return Err(windows::core::Error::from_thread().into());
        }
        let mut drives = Vec::new();
        for drive_letter in 'A'..='Z' {
            if drive_mask & 1 != 0 {
                let path_str = format!(r"{drive_letter}:\");
                let path = Path::new(&path_str);
                let path_u16 = path.to_wide_vec_with_nul();
                let drive_type = unsafe { GetDriveTypeW(PCWSTR(path_u16.as_ptr())) };
                if drive_type == DRIVE_REMOTE {
                    // Use `fs::canonicalize` to match the expected inputs.
                    let canonicalized = fs::canonicalize(path)?;
                    drives.push((drive_letter, canonicalized));
                }
            }
            drive_mask >>= 1;
            if drive_mask == 0 {
                break;
            }
        }
        Ok(drives)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_remote_drives() {
        let drives = Drives::get_remote_drives().unwrap();
        // As the result depends on the machine configuration, all this test can
        // check is if it doesn't fail.
        // You can check the result manually by:
        // ```
        // cargo test -- get_remote_drives --nocapture
        // ```
        println!("{drives:?}");
    }

    #[test]
    fn sort_drives() {
        assert_eq!(Drives::sort_drives(vec![]), vec![]);
        assert_eq!(
            Drives::sort_drives(vec![('A', PathBuf::from("1"))]),
            vec![('A', PathBuf::from("1"))]
        );
        assert_eq!(
            Drives::sort_drives(vec![
                ('C', PathBuf::from("12")),
                ('A', PathBuf::from("123")),
                ('B', PathBuf::from("12"))
            ]),
            vec![
                ('B', PathBuf::from("12")),
                ('C', PathBuf::from("12")),
                ('A', PathBuf::from("123"))
            ]
        );
    }

    // [`fs::canonicalize`] emits the [Win32 File Namespaces].
    //
    // [`fs::canonicalize`]: https://doc.rust-lang.org/std/fs/fn.canonicalize.html
    // [Win32 File Namespaces]: https://learn.microsoft.com/en-us/windows/win32/fileio/naming-a-file#win32-file-namespaces
    #[test]
    fn drive_path_win32_file_namespaces() {
        let drives = Drives::with_drives(vec![
            ('H', PathBuf::from(r"\\?\UNC\server\share\dir")),
            ('X', PathBuf::from(r"\\?\UNC\server\share")),
            ('Z', PathBuf::from(r"\\?\UNC\server2\share2")),
        ]);
        assert_eq!(
            drives._drive_path(Path::new(r"\\?\UNC\server\share\dir\file.txt")),
            Some(DrivePath::new('X', Path::new(r"dir\file.txt")))
        );
        assert_eq!(
            drives._drive_path(Path::new(r"\\?\UNC\server2\share2\dir2\file2.txt")),
            Some(DrivePath::new('Z', Path::new(r"dir2\file2.txt"))),
        );
        assert_eq!(
            drives._drive_path(Path::new(r"\\?\UNC\server3\share3\dir3\file3.txt")),
            None,
        );
        assert_eq!(drives._drive_path(Path::new(r"C:\Windows\System32")), None);
    }
}
