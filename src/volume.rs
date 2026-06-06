use crate::{DrivePath, PathExt};
use std::{
    borrow::Cow,
    cmp::Ordering,
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::{LazyLock, Mutex},
    time::Instant,
};
use windows::{
    Win32::{
        Foundation::{ERROR_MORE_DATA, ERROR_NO_MORE_ITEMS, HANDLE, NO_ERROR},
        NetworkManagement::WNet::{
            NETRESOURCEW, RESOURCE_CONNECTED, RESOURCETYPE_DISK, WNET_OPEN_ENUM_USAGE,
            WNetCloseEnum, WNetEnumResourceW, WNetOpenEnumW,
        },
        Storage::FileSystem::{GetDriveTypeW, GetLogicalDrives},
        System::WindowsProgramming::DRIVE_REMOTE,
    },
    core::PCWSTR,
};

static VOLUMES: LazyLock<Mutex<Option<Volumes>>> = LazyLock::new(|| Mutex::new(None));

#[derive(Clone, Debug)]
pub(crate) struct Volumes {
    volumes: Vec<Volume>,
}

impl Volumes {
    fn new() -> anyhow::Result<Self> {
        Ok(Self::with_volumes(Volume::get_remote_volumes()?))
    }

    pub(crate) fn with_volumes(mut volumes: Vec<Volume>) -> Self {
        volumes = Volume::sort(volumes);
        Self { volumes }
    }

    #[cfg(all(test, windows))]
    pub(crate) fn mock() -> Self {
        Self::with_volumes(vec![
            Volume::new('X', PathBuf::from(r"\\?\UNC\server\share")),
            Volume::new('Z', PathBuf::from(r"\\?\UNC\server2\share2")),
            Volume::new('\0', PathBuf::from(r"\\?\UNC\server0\share0")),
        ])
    }

    pub(crate) fn refresh() -> anyhow::Result<()> {
        Volumes::new()?.set_to_cache();
        Ok(())
    }

    fn set_to_cache(self) {
        let mut cache = VOLUMES.lock().unwrap();
        *cache = Some(self);
    }

    pub(crate) fn drive_path<'a>(path: &'a Path) -> anyhow::Result<Option<DrivePath<'a>>> {
        let drives = {
            let mut cache = VOLUMES.lock().unwrap();
            if cache.is_none() {
                *cache = Some(Volumes::new()?);
            }
            cache.as_ref().cloned().unwrap()
        };
        Ok(drives._drive_path(path))
    }

    pub(crate) fn _drive_path<'a>(&self, path: &'a Path) -> Option<DrivePath<'a>> {
        for volume in &self.volumes {
            if let Ok(suffix) = path.strip_prefix_fix(&volume.path) {
                return Some(DrivePath::new(volume.drive_letter, suffix));
            }
        }
        None
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Volume {
    drive_letter: char,
    path: PathBuf,
}

impl Volume {
    pub(crate) fn new(drive_letter: char, path: impl AsRef<Path>) -> Self {
        Self {
            drive_letter,
            path: path.as_ref().into(),
        }
    }

    fn has_drive(&self) -> bool {
        self.drive_letter != '\0'
    }

    /// The shortest path wins.
    /// If paths are the same, the lowest drive letter wins.
    fn sort(mut volumes: Vec<Volume>) -> Vec<Volume> {
        volumes.sort_by(|a, b| {
            let mut cmp = a.path.as_os_str().len().cmp(&b.path.as_os_str().len());
            if cmp == Ordering::Equal {
                cmp = a.path.cmp(&b.path);
                if cmp == Ordering::Equal && a.drive_letter != b.drive_letter {
                    if !a.has_drive() {
                        cmp = Ordering::Greater;
                    } else if !b.has_drive() {
                        cmp = Ordering::Less;
                    } else {
                        cmp = a.drive_letter.cmp(&b.drive_letter);
                    }
                }
            }
            cmp
        });
        volumes
    }

    fn get_remote_volumes() -> anyhow::Result<Vec<Self>> {
        // Get both drives and net resources and unify them.
        // They often match, but there are cases where they don't, such as:
        // 1. Third-Party Redirectors and Virtual Filesystems.
        // 2. Offline/Disconnected Mapped Drives.
        // 3. When network services are disabled, unresponsive, or in a
        //    restricted context.
        let drives = Self::get_remote_drives()?;
        let net_resources = Self::get_net_resources()?;
        let volumes = drives.into_iter().chain(net_resources);
        let mut map: HashMap<PathBuf, Volume> = HashMap::new();
        for volume in volumes {
            match map.entry(volume.path.clone()) {
                std::collections::hash_map::Entry::Occupied(mut occ) => {
                    if !occ.get().has_drive() && volume.has_drive() {
                        *occ.get_mut() = volume;
                    }
                }
                std::collections::hash_map::Entry::Vacant(vac) => {
                    vac.insert(volume);
                }
            }
        }
        Ok(map.values().cloned().collect())
    }

    fn get_remote_drives() -> anyhow::Result<Vec<Self>> {
        let start = Instant::now();
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
                    log::trace!("Drive: {drive_letter:?} {path:?} {canonicalized:?}");
                    drives.push(Self::new(drive_letter, canonicalized));
                }
            }
            drive_mask >>= 1;
            if drive_mask == 0 {
                break;
            }
        }
        log::trace!("Drive: elapsed {:?}", start.elapsed());
        Ok(drives)
    }

    fn get_net_resources() -> anyhow::Result<Vec<Volume>> {
        let start = Instant::now();
        let resources = unsafe { Self::_get_net_resources() }?;
        let mut volumes = Vec::with_capacity(resources.len());
        for (local, remote) in resources {
            if remote.is_empty() {
                log::warn!("Remote is empty for {local:?}");
                continue;
            }
            let remote = Self::normalize_remote(&remote);
            let drive_letter = Self::drive_letter_from_local(&local);
            volumes.push(Volume::new(drive_letter, remote.as_ref()));
        }
        log::trace!("Net: elapsed {:?}", start.elapsed());
        Ok(volumes)
    }

    unsafe fn _get_net_resources() -> windows::core::Result<Vec<(String, String)>> {
        let mut shares = Vec::new();
        // for scope in [RESOURCE_CONNECTED, RESOURCE_REMEMBERED] {
        let mut henum = HANDLE::default();
        let res = unsafe {
            WNetOpenEnumW(
                RESOURCE_CONNECTED,
                RESOURCETYPE_DISK,
                WNET_OPEN_ENUM_USAGE(0),
                None,
                &mut henum,
            )
        };
        if res != NO_ERROR {
            return Err(windows::core::Error::from_hresult(res.to_hresult()));
        }
        let mut buffer = vec![0u8; 16384];
        loop {
            let mut count = 0xFFFFFFFF;
            let mut buffer_size = buffer.len() as u32;
            let res = unsafe {
                WNetEnumResourceW(
                    henum,
                    &mut count,
                    buffer.as_mut_ptr() as *mut _,
                    &mut buffer_size,
                )
            };
            match res {
                NO_ERROR => {}
                ERROR_NO_MORE_ITEMS => break,
                ERROR_MORE_DATA => {
                    buffer_size *= 2;
                    buffer.resize(buffer_size as usize, 0);
                    continue;
                }
                _ => return Err(windows::core::Error::from_hresult(res.to_hresult())),
            }
            let entries = count as usize;
            let ptr = buffer.as_ptr() as *const NETRESOURCEW;
            for i in 0..entries {
                let resource = unsafe { &*ptr.add(i) };
                let remote_name = if resource.lpRemoteName.is_null() {
                    log::warn!("lpRemoteName null");
                    continue;
                } else {
                    match unsafe { resource.lpRemoteName.to_string() } {
                        Ok(str) => str,
                        Err(error) => {
                            log::warn!("lpRemoteName invalid: {error}");
                            continue;
                        }
                    }
                };
                let local_name = if resource.lpLocalName.is_null() {
                    String::new()
                } else {
                    match unsafe { resource.lpLocalName.to_string() } {
                        Ok(str) => str,
                        Err(error) => {
                            log::warn!("lpLocalName for {remote_name:?} invalid: {error}");
                            String::new()
                        }
                    }
                };
                log::trace!("Net: {local_name:?} {remote_name:?}");
                shares.push((local_name, remote_name));
            }
        }
        let _ = unsafe { WNetCloseEnum(henum) };
        Ok(shares)
    }

    fn drive_letter_from_local(local: &str) -> char {
        if local.len() == 2 && local.as_bytes()[1] == b':' {
            local.as_bytes()[0] as char
        } else {
            '\0'
        }
    }

    fn normalize_remote<'a>(mut remote: &'a str) -> Cow<'a, str> {
        remote = remote.trim_end_matches('\\');
        if let Some(stripped) = remote.strip_prefix(r"\\") {
            return Cow::Owned(format!(r"\\?\UNC\{}", stripped));
        }
        Cow::Borrowed(remote)
    }
}

#[cfg(test)]
static LOG_INIT: LazyLock<bool> = LazyLock::new(|| {
    env_logger::init();
    true
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_remote_volumes() {
        assert!(*LOG_INIT);

        let volumes = Volume::get_remote_volumes().unwrap();
        // As the result depends on the machine configuration, all this test can
        // check is if it doesn't fail.
        // You can check the result manually by:
        // ```
        // cargo test -- get_remote_volumes --nocapture
        // ```
        for volume in volumes {
            println!("{volume:?}");
        }
    }

    #[test]
    fn sort() {
        assert_eq!(Volume::sort(vec![]), vec![]);
        assert_eq!(
            Volume::sort(vec![Volume::new('A', PathBuf::from("1"))]),
            vec![Volume::new('A', PathBuf::from("1"))]
        );
        assert_eq!(
            Volume::sort(vec![
                Volume::new('\0', PathBuf::from("124")),
                Volume::new('\0', PathBuf::from("123")),
                Volume::new('C', PathBuf::from("12")),
                Volume::new('A', PathBuf::from("123")),
                Volume::new('B', PathBuf::from("12")),
            ]),
            vec![
                Volume::new('B', PathBuf::from("12")),
                Volume::new('C', PathBuf::from("12")),
                Volume::new('A', PathBuf::from("123")),
                Volume::new('\0', PathBuf::from("123")),
                Volume::new('\0', PathBuf::from("124")),
            ]
        );
    }

    // [`fs::canonicalize`] emits the [Win32 File Namespaces].
    //
    // [`fs::canonicalize`]: https://doc.rust-lang.org/std/fs/fn.canonicalize.html
    // [Win32 File Namespaces]: https://learn.microsoft.com/en-us/windows/win32/fileio/naming-a-file#win32-file-namespaces
    #[test]
    fn drive_path_win32_file_namespaces() {
        let volumes = Volumes::with_volumes(vec![
            Volume::new('H', PathBuf::from(r"\\?\UNC\server\share\dir")),
            Volume::new('X', PathBuf::from(r"\\?\UNC\server\share")),
            Volume::new('Z', PathBuf::from(r"\\?\UNC\server2\share2")),
        ]);
        assert_eq!(
            volumes._drive_path(Path::new(r"\\?\UNC\server\share\dir\file.txt")),
            Some(DrivePath::new('X', Path::new(r"dir\file.txt")))
        );
        assert_eq!(
            volumes._drive_path(Path::new(r"\\?\UNC\server2\share2\dir2\file2.txt")),
            Some(DrivePath::new('Z', Path::new(r"dir2\file2.txt"))),
        );
        assert_eq!(
            volumes._drive_path(Path::new(r"\\?\UNC\server3\share3\dir3\file3.txt")),
            None,
        );
        assert_eq!(volumes._drive_path(Path::new(r"C:\Windows\System32")), None);

        let unmapped_drives = Volumes::with_volumes(vec![Volume::new(
            '\0',
            PathBuf::from(r"\\?\UNC\server\share"),
        )]);
        assert_eq!(
            unmapped_drives._drive_path(Path::new(r"\\?\UNC\server\share\dir\file.txt")),
            Some(DrivePath::new('\0', Path::new(r"dir\file.txt")))
        );
    }
}
