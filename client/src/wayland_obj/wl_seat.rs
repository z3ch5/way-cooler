use std::{cell::RefCell, rc::Rc};
use wayland_client::{protocol::wl_seat as protocol, GlobalImplementor, NewProxy, Proxy};

use protocol::{Capability, WlSeat};

/// The minimum version of the wl_seat global to bind to.
pub(crate) const WL_SEAT_VERSION: u32 = 1;

#[derive(Debug)]
pub(crate) struct SeatData {
    capabilities: Capability,
    name: String
}

impl SeatData {
    pub fn new() -> Self {
        SeatData {
            capabilities: Capability::empty(),
            name: String::default()
        }
    }
}

pub(crate) struct WlSeatManager {
    seat: Option<WlSeat>
}

impl WlSeatManager {
    pub fn new() -> Self {
        WlSeatManager { seat: None }
    }
}

impl GlobalImplementor<WlSeat> for WlSeatManager {
    fn new_global(&mut self, new_seat: NewProxy<WlSeat>) -> WlSeat {
        let seat = new_seat.implement(SeatHandler {}, Rc::new(RefCell::new(SeatData::new())));
        if self.seat.replace(seat.clone()).is_some() {
            panic!("Seat already registered. Multiple seats are not supported");
        }

        seat
    }
}

struct SeatHandler {}

impl SeatHandler {}

#[allow(unused_variables)]
impl protocol::EventHandler for SeatHandler {
    fn capabilities(&mut self, object: WlSeat, capabilities: Capability) {
        seat_data(object.as_ref()).borrow_mut().capabilities = capabilities;
    }

    fn name(&mut self, object: WlSeat, name: String) {
        seat_data(object.as_ref()).borrow_mut().name = name;
    }
}

fn seat_data(seat: &Proxy<WlSeat>) -> &RefCell<SeatData> {
    seat.user_data::<Rc<RefCell<SeatData>>>()
        .expect("No data associated with seat")
}
