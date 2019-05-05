//! Wrappers around Wayland objects

mod layer_shell;
mod manager;
mod output;
mod wl_compositor;
mod wl_seat;
mod wl_shm;

pub(crate) use self::{
    output::{Output, OutputEventHandler},
    wl_compositor::Surface,
    wl_shm::Buffer
};
mod xdg_shell;

pub use self::{
    layer_shell::LayerSurface,
    manager::{create_buffer, create_layer_surface, global_callback, init_wayland}
};
