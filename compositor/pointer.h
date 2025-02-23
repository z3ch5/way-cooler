#ifndef WC_POINTER_H
#define WC_POINTER_H

#include "wlr/types/wlr_input_device.h"

#include "server.h"

struct wc_pointer {
	struct wl_list link;
	struct wc_server* server;

	struct wlr_input_device* device;

	struct wl_listener destroy;
};

void wc_new_pointer(struct wc_server* server, struct wlr_input_device* device);
void wc_init_pointers(struct wc_server* server);

#endif//WC_POINTER_H
