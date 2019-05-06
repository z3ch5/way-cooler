use std::{cell::RefCell, rc::Rc};

use wayland_client::{protocol::wl_seat as protocol, GlobalImplementor, NewProxy, Proxy};

use protocol::{Capability, WlSeat};

/// The minimum version of the wl_seat global to bind to.
pub(super) const WL_SEAT_VERSION: u32 = 1;

#[derive(Debug)]
pub(super) struct SeatData {
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

#[derive(Default)]
pub(super) struct WlSeatManager {
    seat: Option<WlSeat>
}

impl GlobalImplementor<WlSeat> for WlSeatManager {
    fn new_global(&mut self, new_seat: NewProxy<WlSeat>) -> WlSeat {
        let seat = new_seat.implement(WlSeatEventHandler, Rc::new(RefCell::new(SeatData::new())));

        if self.seat.is_some() {
            warn!("There is already a seat registered. Ignoring additional seat.");
        } else {
            self.seat = Some(seat.clone());
        }

        seat
    }
}

struct WlSeatEventHandler;

impl protocol::EventHandler for WlSeatEventHandler {
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
