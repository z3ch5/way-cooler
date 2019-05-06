//! Wrapper around a wl_output

use std::{cell::RefCell, fmt, rc::Rc};

use wayland_client::{
    protocol::wl_output::{self, WlOutput},
    GlobalImplementor, NewProxy, Proxy
};

/// The minimum version of the wl_output global to bind to.
pub const WL_OUTPUT_VERSION: u32 = 2;

/// Wrapper around WlOutput.
#[derive(Clone, Eq, PartialEq)]
pub struct Output {
    output: WlOutput
}

pub trait OutputEventHandler {
    fn output_changed(&self, output: Output);
}

impl OutputEventHandler for Fn(Output) {
    fn output_changed(&self, output: Output) {
        (*self)(output);
    }
}

// Provides new WlOutputs with an implementation.
pub struct WlOutputManager {
    handler: Rc<dyn OutputEventHandler>
}

impl WlOutputManager {
    pub fn new(handler: Rc<dyn OutputEventHandler>) -> Self {
        WlOutputManager { handler }
    }
}

// Handle incoming events for WlOutput.
struct WlOutputEventHandler {
    handler: Rc<dyn OutputEventHandler>
}

/// The cached state for the WlOutput.
#[derive(Debug, Default, Clone, Eq, PartialEq)]
struct OutputState {
    name: String,
    // TODO(ried): use area::Size
    resolution: (u32, u32)
}

impl Output {
    pub fn resolution(&self) -> (u32, u32) {
        unwrap_state(self.as_ref()).borrow().resolution
    }

    pub fn name(&self) -> String {
        unwrap_state(self.as_ref()).borrow().name.clone()
    }
}

impl GlobalImplementor<WlOutput> for WlOutputManager {
    fn new_global(&mut self, new_proxy: NewProxy<WlOutput>) -> WlOutput {
        new_proxy.implement(
            WlOutputEventHandler {
                handler: self.handler.clone()
            },
            Rc::new(RefCell::new(OutputState::default()))
        )
    }
}

impl wl_output::EventHandler for WlOutputEventHandler {
    #[allow(unused)]
    fn geometry(
        &mut self,
        object: WlOutput,
        x: i32,
        y: i32,
        physical_width: i32,
        physical_height: i32,
        subpixel: wl_output::Subpixel,
        make: String,
        model: String,
        transform: wl_output::Transform
    ) {
        unwrap_state(object.as_ref()).borrow_mut().name = format!("{} ({})", make, model);
    }

    #[allow(unused)]
    fn mode(&mut self, object: WlOutput, flags: wl_output::Mode, width: i32, height: i32, refresh: i32) {
        unwrap_state(object.as_ref()).borrow_mut().resolution = (width as u32, height as u32);
    }

    #[allow(unused)]
    fn done(&mut self, object: WlOutput) {
        self.handler.output_changed(Output { output: object });
    }

    #[allow(unused)]
    fn scale(&mut self, object: WlOutput, factor: i32) {
        // TODO
        if factor != 1 {
            unimplemented!("output scaling");
        }
    }
}

impl fmt::Debug for Output {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self.name())
    }
}

impl AsRef<WlOutput> for Output {
    fn as_ref(&self) -> &WlOutput {
        &self.output
    }
}

impl AsRef<Proxy<WlOutput>> for Output {
    fn as_ref(&self) -> &Proxy<WlOutput> {
        &self.output.as_ref()
    }
}

fn unwrap_state(proxy: &Proxy<WlOutput>) -> &RefCell<OutputState> {
    proxy
        .user_data::<Rc<RefCell<OutputState>>>()
        .expect("User data has not been set yet")
}
