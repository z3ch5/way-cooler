//! Wrapper around a wl_compositor.

use wayland_client::{
    protocol::{wl_compositor::WlCompositor, wl_surface::WlSurface},
    GlobalImplementor, NewProxy
};

/// The minimum version of the wl_compositor global to bind to.
pub(crate) const WL_COMPOSITOR_VERSION: u32 = 3;

pub(crate) type Surface = WlSurface;

#[derive(Default)]
pub(crate) struct WlCompositorManager {
    compositor: Option<WlCompositor>
}

impl WlCompositorManager {
    pub fn create_surface(&self) -> Result<Surface, ()> {
        use wayland_client::Interface; // for ::NAME
        let compositor = self.compositor.as_ref().expect(&format!(
            "No WlCompositor registered. Make sure your compositor supports the {} protocol",
            WlCompositor::NAME
        ));
        compositor.create_surface(NewProxy::implement_dummy)
    }
}

impl GlobalImplementor<WlCompositor> for WlCompositorManager {
    fn new_global(&mut self, new_proxy: NewProxy<WlCompositor>) -> WlCompositor {
        let res = new_proxy.implement_dummy();

        if self.compositor.replace(res.clone()).is_some() {
            use wayland_client::Interface; // for ::NAME
            panic!(
                "{} already registered. Multiple compositors are not supported",
                WlCompositor::NAME
            );
        }

        res
    }
}
