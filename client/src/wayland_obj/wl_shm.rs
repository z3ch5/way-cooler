//! Wrapper around a wl_shm.

use std::{fs::File, io::Write};

use wayland_client::{
    protocol::{
        wl_buffer::WlBuffer,
        wl_shm::{Format, WlShm}
    },
    NewProxy
};

use crate::area::Size;

// TODO(ried): the WlBuffer internal should be internal and Buffer should be Clone
pub struct Buffer {
    pub(crate) buffer: WlBuffer,
    shared_memory: File
}

impl Buffer {
    pub fn write(&mut self, data: &[u8]) {
        self.shared_memory
            .write(&*data)
            .expect("Could not write data to buffer");
        self.shared_memory.flush().expect("Could not flush buffer");
    }
}

impl AsRef<WlBuffer> for Buffer {
    fn as_ref(&self) -> &WlBuffer {
        &self.buffer
    }
}

/// The minimum version of the wl_shm global to bind to.
pub(crate) const WL_SHM_VERSION: u32 = 1;

pub(crate) struct WlShmManager {
    shm: Option<WlShm>
}

impl WlShmManager {
    pub fn new() -> Self {
        WlShmManager { shm: None }
    }

    /// Create a buffer with the given size. May allocate a temporary file.
    pub fn create_buffer(&self, size: Size) -> Result<Buffer, ()> {
        let Size { width, height } = size;
        let width = width as i32;
        let height = height as i32;
        let shm = self
            .shm
            .as_ref()
            .expect("WlShm was not initialized. Make sure your compositor supports the WlShm protocol");

        let temp_file = tempfile::tempfile().expect("Could not make new temp file");
        temp_file
            .set_len(size.width as u64 * size.height as u64 * 4)
            .expect("Could not set file length");
        let fd = std::os::unix::io::AsRawFd::as_raw_fd(&temp_file);

        let pool = shm.create_pool(fd, width * height * 4, NewProxy::implement_dummy)?;
        // TODO ARb32 instead
        pool.create_buffer(
            0,
            width,
            height,
            width * 4,
            Format::Argb8888,
            NewProxy::implement_dummy
        )
        .map(|buffer| Buffer {
            shared_memory: temp_file,
            buffer
        })
    }
}

impl wayland_client::GlobalImplementor<WlShm> for WlShmManager {
    fn new_global(&mut self, new_proxy: NewProxy<WlShm>) -> WlShm {
        let shm = new_proxy.implement_dummy();
        if self.shm.replace(shm.clone()).is_some() {
            panic!("Shm already registered. Multiple shm are not supported");
        }

        shm
    }
}
