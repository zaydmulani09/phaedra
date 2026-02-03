use anyhow::Result;

pub struct SharedMemory {
    pub id: String,
    ptr: *mut u8,
    size: usize,
}

unsafe impl Send for SharedMemory {}
unsafe impl Sync for SharedMemory {}

impl SharedMemory {
    pub fn new() -> Result<Self> {
        platform::create(super::bitmap::MAP_SIZE)
    }

    pub fn read_map(&self) -> super::bitmap::CoverageMap {
        let mut map = super::bitmap::CoverageMap::new();
        unsafe {
            std::ptr::copy_nonoverlapping(
                self.ptr,
                map.data.as_mut_ptr(),
                self.size.min(super::bitmap::MAP_SIZE),
            );
        }
        map
    }

    pub fn clear(&self) {
        unsafe {
            std::ptr::write_bytes(self.ptr, 0, self.size);
        }
    }
}

#[cfg(unix)]
mod platform {
    use super::*;

    pub(super) fn create(size: usize) -> Result<SharedMemory> {
        let shm_id = unsafe { libc::shmget(libc::IPC_PRIVATE, size, libc::IPC_CREAT | 0o600) };
        if shm_id < 0 {
            anyhow::bail!("shmget failed: {}", std::io::Error::last_os_error());
        }

        let ptr = unsafe { libc::shmat(shm_id, std::ptr::null(), 0) };
        if ptr as isize == -1 {
            anyhow::bail!("shmat failed: {}", std::io::Error::last_os_error());
        }

        unsafe { std::ptr::write_bytes(ptr as *mut u8, 0, size) };

        Ok(SharedMemory {
            id: shm_id.to_string(),
            ptr: ptr as *mut u8,
            size,
        })
    }

    impl Drop for SharedMemory {
        fn drop(&mut self) {
            unsafe {
                libc::shmdt(self.ptr as *const libc::c_void);
            }
        }
    }
}

#[cfg(windows)]
mod platform {
    use super::*;

    const PAGE_READWRITE: u32 = 0x04;
    const FILE_MAP_ALL_ACCESS: u32 = 0xF001F;
    const INVALID_HANDLE_VALUE: *mut std::ffi::c_void = usize::MAX as *mut std::ffi::c_void;

    #[link(name = "kernel32")]
    extern "system" {
        fn CreateFileMappingA(
            hFile: *mut std::ffi::c_void,
            lpAttr: *mut std::ffi::c_void,
            flProtect: u32,
            dwMaxSizeHigh: u32,
            dwMaxSizeLow: u32,
            lpName: *const u8,
        ) -> *mut std::ffi::c_void;

        fn MapViewOfFile(
            hObject: *mut std::ffi::c_void,
            dwAccess: u32,
            dwOffHigh: u32,
            dwOffLow: u32,
            nBytes: usize,
        ) -> *mut std::ffi::c_void;
    }

    pub(super) fn create(size: usize) -> Result<SharedMemory> {
        let name = format!("phaedra_shm_{}", std::process::id());
        let mut name_bytes = name.as_bytes().to_vec();
        name_bytes.push(0);

        let handle = unsafe {
            CreateFileMappingA(
                INVALID_HANDLE_VALUE,
                std::ptr::null_mut(),
                PAGE_READWRITE,
                0,
                size as u32,
                name_bytes.as_ptr(),
            )
        };
        if handle.is_null() {
            anyhow::bail!("CreateFileMappingA failed: {}", std::io::Error::last_os_error());
        }

        let ptr = unsafe { MapViewOfFile(handle, FILE_MAP_ALL_ACCESS, 0, 0, size) };
        if ptr.is_null() {
            anyhow::bail!("MapViewOfFile failed: {}", std::io::Error::last_os_error());
        }

        unsafe { std::ptr::write_bytes(ptr as *mut u8, 0, size) };

        Ok(SharedMemory {
            id: name,
            ptr: ptr as *mut u8,
            size,
        })
    }

    impl Drop for SharedMemory {
        fn drop(&mut self) {
            // Released when process exits.
        }
    }
}

#[cfg(not(any(unix, windows)))]
mod platform {
    use super::*;

    pub(super) fn create(size: usize) -> Result<SharedMemory> {
        anyhow::bail!("SharedMemory not supported on this platform");
    }

    impl Drop for SharedMemory {
        fn drop(&mut self) {}
    }
}
