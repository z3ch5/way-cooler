#include "seat.h"

#include <stdlib.h>

#include <wlr/types/wlr_seat.h>
#include <wlr/types/wlr_surface.h>

#include "cursor.h"
#include "server.h"

static void wc_seat_request_cursor(struct wl_listener* listener, void* data) {
	struct wc_seat* seat = wl_container_of(listener, seat,
			request_set_cursor);
	struct wc_server* server = seat->server;
	struct wc_cursor* cursor = server->cursor;
	struct wlr_seat_pointer_request_set_cursor_event* event = data;
	struct wlr_seat_client *focused_client =
		server->seat->seat->pointer_state.focused_client;
	if (focused_client == event->seat_client) {
		cursor->image = NULL;
		wlr_cursor_set_surface(cursor->wlr_cursor,
				event->surface, event->hotspot_x, event->hotspot_y);
	}
}

void wc_seat_update_surface_focus(struct wc_seat* seat,
		struct wlr_surface* surface, double sx, double sy, uint32_t time) {
	struct wlr_seat* wlr_seat = seat->seat;
	if (surface == NULL) {
		wlr_seat_pointer_clear_focus(wlr_seat);
		return;
	}
	bool focused_changed = wlr_seat->pointer_state.focused_surface != surface;
	wlr_seat_pointer_notify_enter(wlr_seat, surface, sx, sy);
	if (!focused_changed) {
		wlr_seat_pointer_notify_motion(wlr_seat, time, sx, sy);
	}
}

void wc_seat_set_focus_layer(struct wc_seat* seat,
		struct wlr_layer_surface_v1* layer) {
	// TODO
}

void wc_init_seat(struct wc_server* server) {
	struct wc_seat* seat = calloc(1, sizeof(struct wc_seat));
	seat->server = server;
	seat->seat = wlr_seat_create(server->wl_display, "seat0");

	seat->request_set_cursor.notify = wc_seat_request_cursor;
	wl_signal_add(&seat->seat->events.request_set_cursor,
			&seat->request_set_cursor);

	server->seat = seat;
}
