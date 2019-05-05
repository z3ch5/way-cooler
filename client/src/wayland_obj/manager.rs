use std::{cell::RefCell, rc::Rc};

use wayland_client::{
    protocol::{
        wl_compositor::WlCompositor, wl_output::WlOutput, wl_registry::WlRegistry, wl_seat::WlSeat,
        wl_shm::WlShm
    },
    Display, EventQueue, GlobalEvent, GlobalImplementor, Interface, Proxy
};

use crate::area::Size;

use super::{
    layer_shell::{self, LayerShellManager, LayerSurface, WlrLayerShell, WLR_LAYER_SHELL_VERSION},
    output::{OutputEventHandler, WlOutputManager, WL_OUTPUT_VERSION},
    wl_compositor::{WlCompositorManager, WL_COMPOSITOR_VERSION},
    wl_seat::{WlSeatManager, WL_SEAT_VERSION},
    wl_shm::{Buffer, WlShmManager, WL_SHM_VERSION}
};

thread_local! {
    static WAYLAND: RefCell<Option<WaylandManager>> = RefCell::new(None);
}

struct WaylandManager {
    compositor_manager: WlCompositorManager,
    output_manager: WlOutputManager,
    seat_manager: WlSeatManager,
    shell_manager: LayerShellManager,
    shm_manager: WlShmManager
}

// pub fn create_surface() -> Result<WlSurface, ()> {
//     WAYLAND.with(|wayland| {
//         let wayland = wayland.borrow();
//         let wayland = wayland.as_ref().expect("Wayland not initialized");

//         wayland.compositor_manager.create_surface()
//     })
// }

pub fn create_buffer(size: Size) -> Result<Buffer, ()> {
    WAYLAND.with(|wayland| {
        let wayland = wayland.borrow();
        let wayland = wayland.as_ref().expect("Wayland not initialized");

        wayland.shm_manager.create_buffer(size)
    })
}

pub fn create_layer_surface() -> Result<LayerSurface, ()> {
    WAYLAND.with(|wayland| {
        use layer_shell::Layer;
        let wayland = wayland.borrow();
        let wayland = wayland.as_ref().expect("Wayland not initialized");

        let wl_surface = wayland.compositor_manager.create_surface()?;

        wayland
            .shell_manager
            .create_layer_surface(wl_surface, None, Layer::Top)
    })
}

pub fn global_callback(event: GlobalEvent, registry: WlRegistry) {
    WAYLAND.with(|wayland| {
        let wayland = &mut wayland.borrow_mut();
        let wayland = wayland.as_mut().expect("Wayland not initialized");
        match event {
            GlobalEvent::New {
                id,
                interface,
                version
            } => wayland.new_global(registry, id, &interface, version),
            _ => unimplemented!("GlobalEvent")
        }
    })
}

pub fn init_wayland() -> (Display, EventQueue) {
    let (display, event_queue) = Display::connect_to_env().unwrap_or_else(|err| {
        use std::{env, process::exit};
        use wayland_client::ConnectError::*;
        match err {
            NoWaylandLib => error!("Could not find Wayland library, is it installed and in PATH?"),
            NoCompositorListening => {
                error!("Could not connect to Wayland server. Is it running?");
                error!(
                    "WAYLAND_DISPLAY={}",
                    env::var("WAYLAND_DISPLAY").unwrap_or_default()
                );
            },
            InvalidName => error!("Invalid socket name provided in WAYLAND_SOCKET"),
            XdgRuntimeDirNotSet => error!("XDG_RUNTIME_DIR must be set"),
            InvalidFd => error!("Invalid socket provided in WAYLAND_SOCKET")
        }
        exit(1);
    });

    WAYLAND.with(|wayland| {
        let wayland = &mut wayland.borrow_mut();
        let way_man = WaylandManager::new(crate::lua::WaylandHandler {});
        (*wayland).replace(way_man);
    });

    (display, event_queue)
}

impl WaylandManager {
    pub fn new<T>(handler: T) -> Self
    where
        T: OutputEventHandler + 'static
    {
        let handler = Rc::new(handler);
        WaylandManager {
            compositor_manager: WlCompositorManager::default(),
            output_manager: WlOutputManager::new(handler),
            seat_manager: WlSeatManager::new(),
            shell_manager: LayerShellManager::new(),
            shm_manager: WlShmManager::new()
        }
    }

    fn new_global(&mut self, registry: WlRegistry, id: u32, interface: &str, version: u32) {
        match interface {
            WlCompositor::NAME => Self::handle::<WlCompositor>(
                registry,
                WL_COMPOSITOR_VERSION,
                version,
                id,
                &mut self.compositor_manager
            ),
            WlOutput::NAME => {
                Self::handle::<WlOutput>(registry, WL_OUTPUT_VERSION, version, id, &mut self.output_manager)
            },
            WlSeat::NAME => {
                Self::handle::<WlSeat>(registry, WL_SEAT_VERSION, version, id, &mut self.seat_manager)
            },
            WlShm::NAME => {
                Self::handle::<WlShm>(registry, WL_SHM_VERSION, version, id, &mut self.shm_manager)
            },
            WlrLayerShell::NAME => Self::handle::<WlrLayerShell>(
                registry,
                WLR_LAYER_SHELL_VERSION,
                version,
                id,
                &mut self.shell_manager
            ),
            _ => info!("unhandled global: {}", interface)
        }
    }

    fn handle<I: Interface + From<Proxy<I>>>(
        registry: WlRegistry,
        min_version: u32,
        version: u32,
        id: u32,
        implementor: &mut GlobalImplementor<I>
    ) {
        if version < min_version {
            implementor.error(version);
        } else {
            registry
                .bind::<I, _>(version, id, |newp| implementor.new_global(newp))
                .expect("wl_registry died unexpectedly");
        }
    }
}
