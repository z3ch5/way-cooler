use std::{cell::RefCell, rc::Rc};

use wayland_client::{protocol::wl_buffer::WlBuffer, GlobalImplementor, NewProxy, Proxy};

pub use wayland_protocols::wlr::unstable::layer_shell::v1::client::{
    zwlr_layer_shell_v1::{self as wlr_layer_shell, Layer, ZwlrLayerShellV1 as WlrLayerShell},
    zwlr_layer_surface_v1::{self as wlr_layer_surface, ZwlrLayerSurfaceV1 as WlrLayerSurface}
};

use crate::area::{Origin, Size};

use super::{Buffer, Output, Surface};

/// The minimum version of the wl_layer_shell protocol to bind to.
pub(crate) const WLR_LAYER_SHELL_VERSION: u32 = 1;

/// LayerShell
pub(crate) struct LayerShellManager {
    shell: Option<WlrLayerShell>
}

impl LayerShellManager {
    pub fn new() -> Self {
        LayerShellManager { shell: None }
    }

    pub fn create_layer_surface(
        &self,
        wl_surface: Surface,
        output: Option<&Output>,
        layer: Layer
    ) -> Result<LayerSurface, ()> {
        use wayland_client::Interface; // for ::NAME
        trace!("create_layer_surface");
        let shell = self.shell.as_ref().expect(&format!(
            "No LayerShell registered. Make sure your compositor supports the {} protocol",
            WlrLayerSurface::NAME
        ));

        let layer_surface = shell.get_layer_surface(
            &wl_surface,
            output.map(AsRef::as_ref),
            layer,
            Default::default(),
            |new_layer_surface| {
                let state = Rc::new(RefCell::new(LayerSurfaceState::new(wl_surface.clone())));
                new_layer_surface.implement(LayerSurfaceEventHandler {}, state)
            }
        )?;

        let res = LayerSurface::new(layer_surface);

        trace!("committing layersurface");
        res.commit();

        Ok(res)
    }
}

impl GlobalImplementor<WlrLayerShell> for LayerShellManager {
    fn new_global(&mut self, new_shell: NewProxy<WlrLayerShell>) -> WlrLayerShell {
        let res = new_shell.implement_dummy();
        if self.shell.replace(res.clone()).is_some() {
            use wayland_client::Interface;
            panic!(
                "{} already registered. Expected unique global",
                WlrLayerShell::NAME
            );
        }
        res
    }
}

/// LayerSurface
pub struct LayerSurface {
    layer_surface: WlrLayerSurface
}

// Since most properties of WlrLayerSurface are double-buffered, only forward requests
impl LayerSurface {
    pub fn new(layer_surface: WlrLayerSurface) -> Self {
        Self { layer_surface }
    }

    pub fn set_buffer(&mut self, buffer: &Buffer, origin: Origin) {
        let mut state = unwrap_state(self.as_ref()).borrow_mut();
        if let Some((_old_buffer, _)) = state.buffer.replace((buffer.buffer.clone(), origin)) {
            // TODO(ried): cleanup?
            warn!("Replacing uncommitted buffer, cleanup?");
        }
        // Assigning the Buffer before the first configure event is a protocol error.
        if state.configuration_serial.is_some() {
            state.attach_buffer();
        } else {
            info!("LayerSurface is not yet configured. Postponing layer attachment");
        }
    }

    pub fn set_size(&mut self, Size { width, height }: Size) {
        self.layer_surface.set_size(width, height);
    }

    pub fn commit(&self) {
        trace!("layersurface commit");
        self.state().borrow().wl_surface.commit();
    }

    fn state(&self) -> &RefCell<LayerSurfaceState> {
        unwrap_state(self.as_ref())
    }
}

impl AsRef<Proxy<WlrLayerSurface>> for LayerSurface {
    fn as_ref(&self) -> &Proxy<WlrLayerSurface> {
        &self.layer_surface.as_ref()
    }
}

struct LayerSurfaceState {
    wl_surface: Surface,
    buffer: Option<(WlBuffer, Origin)>,
    // The last serial received in a configure event or None is unconfigured
    configuration_serial: Option<u32>
}

impl LayerSurfaceState {
    fn new(wl_surface: Surface) -> Self {
        Self {
            wl_surface,
            buffer: None,
            configuration_serial: None
        }
    }
    fn attach_buffer(&mut self) {
        match self.buffer.take() {
            None => panic!("Cannot attach buffer to LayerSurface: no buffer available"),
            Some((buffer, Origin { x, y })) => {
                trace!("Attaching buffer ({:p}) @ ({},{})", &buffer, x, y);
                self.wl_surface.attach(Some(&buffer), x, y);
            }
        }
    }
}

struct LayerSurfaceEventHandler {}

impl wlr_layer_surface::EventHandler for LayerSurfaceEventHandler {
    fn configure(&mut self, layer_surface: WlrLayerSurface, serial: u32, width: u32, height: u32) {
        info!("layer_surface::configure {}x{}", width, height);
        layer_surface.ack_configure(serial);

        let mut state = unwrap_state(layer_surface.as_ref()).borrow_mut();
        state.configuration_serial = Some(serial);
        if state.buffer.is_some() {
            state.attach_buffer();
            state.wl_surface.commit();
        }
    }
    fn closed(&mut self, _layer_surface: WlrLayerSurface) {
        unimplemented!();
    }
}

fn unwrap_state(proxy: &Proxy<WlrLayerSurface>) -> &RefCell<LayerSurfaceState> {
    proxy
        .user_data::<Rc<RefCell<LayerSurfaceState>>>()
        .expect("No user data on WlrLayerSurface")
}
