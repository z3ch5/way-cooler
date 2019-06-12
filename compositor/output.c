#define _POSIX_C_SOURCE 200809L

#include "output.h"

#include <stdlib.h>
#include <time.h>
#include <assert.h>

#include <wayland-server.h>
#include <wlr/types/wlr_matrix.h>
#include <wlr/types/wlr_output.h>
#include <wlr/types/wlr_output_layout.h>
#include <wlr/render/wlr_renderer.h>

#include "layer_shell.h"
#include "view.h"
#include "server.h"

/* Used to move all of the data necessary to render a surface from the top-level
 * frame handler to the per-surface render function. */
struct wc_view_render_data {
	struct wlr_output* output;
	struct wlr_renderer* renderer;
	struct wc_view* view;
	struct timespec* when;
};

/* Used to move all of the data necessary to render a surface from the layers */
struct wc_layer_render_data {
	struct wlr_renderer* renderer;
	struct wc_layer* layer;
	struct timespec* when;
};

static void wc_render_surface(struct wlr_surface* surface,
		struct wlr_output* output, struct wlr_renderer* renderer,
		struct timespec* when, int sx, int sy, double ox, double oy) {
	struct wlr_texture* texture = wlr_surface_get_texture(surface);
	if (texture == NULL) {
		return;
	}
	struct wlr_box box = {
		.x = ox * output->scale,
		.y = oy * output->scale,
		.width = surface->current.width * output->scale,
		.height = surface->current.height * output->scale,
	};
	float matrix[9];
	enum wl_output_transform transform =
		wlr_output_transform_invert(surface->current.transform);
	wlr_matrix_project_box(matrix, &box, transform, 0,
			output->transform_matrix);

	wlr_render_texture_with_matrix(renderer, texture, matrix, 1);

	wlr_surface_send_frame_done(surface, when);
}

static void wc_render_view(struct wlr_surface* surface,
		int sx, int sy, void* data) {
	struct wc_view_render_data* rdata = data;
	struct wc_view* view = rdata->view;
	struct wlr_output* output = rdata->output;

	double ox = 0, oy = 0;
	wlr_output_layout_output_coords(
			view->server->output_layout, output, &ox, &oy);
	ox += view->x + sx, oy += view->y + sy;

	wc_render_surface(surface, output, rdata->renderer,
			rdata->when, sx, sy, ox, oy);
}

static void wc_render_layer(struct wlr_surface* surface,
		int sx, int sy, void* data) {
	struct wc_layer_render_data* rdata = data;
	struct wc_layer* layer = rdata->layer;
	struct wc_server* server = layer->server;
	struct wlr_output* output = layer->layer_surface->output;

	double ox = 0, oy = 0;
	wlr_output_layout_output_coords(
			server->output_layout, output, &ox, &oy);
	ox += layer->geo.x + sx, oy += layer->geo.y + sy;

	wc_render_surface(surface, output, rdata->renderer,
			rdata->when, sx, sy, ox, oy);
}

static void wc_render_layers(struct timespec* now,
		struct wlr_renderer* renderer, struct wc_output* output,
		struct wl_list* layers) {
	struct wc_layer* layer;
	wl_list_for_each_reverse(layer, layers, link) {
		if (!layer->mapped) {
			continue;
		}
		struct wc_layer_render_data rdata = {
			.layer = layer,
			.renderer = renderer,
			.when = now
		};

		wlr_layer_surface_v1_for_each_surface(layer->layer_surface,
				wc_render_layer, &rdata);
	}
}

static void wc_output_frame(struct wl_listener* listener, void* data) {
	struct wc_output* output = wl_container_of(listener, output, frame);
	struct wc_server* server = output->server;
	struct wlr_output* wlr_output = output->output;
	struct wlr_renderer* renderer = wlr_backend_get_renderer(wlr_output->backend);
	assert(renderer);

	struct timespec now;
	clock_gettime(CLOCK_MONOTONIC, &now);
	//TODO wlr_output_attach_render(wlr_output, NULL);
	if (!wlr_output_attach_render(wlr_output, NULL)) { //otherwise failed to compile in the new version of wlroots)
		return;
	}

	int width, height;
	wlr_output_effective_resolution(wlr_output, &width, &height);
	wlr_renderer_begin(renderer, width, height);

	// TODO Remove this once a background renders
	float color[4] = { 0.25f, 0.25f, 0.25f, 1 };
	wlr_renderer_clear(renderer, color);

	struct wl_list* backgrounds =
		&output->layers[ZWLR_LAYER_SHELL_V1_LAYER_BACKGROUND];
	struct wl_list* bottom =
		&output->layers[ZWLR_LAYER_SHELL_V1_LAYER_BOTTOM];
	struct wl_list* top =
		&output->layers[ZWLR_LAYER_SHELL_V1_LAYER_TOP];
	struct wl_list* overlay =
		&output->layers[ZWLR_LAYER_SHELL_V1_LAYER_OVERLAY];

	wc_render_layers(&now, renderer, output, backgrounds);
	wc_render_layers(&now, renderer, output, bottom);

	// Render traditional shell surfaces between bottom and top layers.
	struct wc_view* view;
	wl_list_for_each_reverse(view, &server->views, link) {
		if (!view->mapped) {
			continue;
		}
		struct wc_view_render_data rdata = {
			.output = output->output,
			.view = view,
			.renderer = renderer,
			.when = &now
		};

		wlr_xdg_surface_for_each_surface(view->xdg_surface,
				wc_render_view, &rdata);
	}

	wc_render_layers(&now, renderer, output, top);
	wc_render_layers(&now, renderer, output, overlay);

	wlr_output_render_software_cursors(wlr_output, NULL);

	//TODO use wlr_output_commit(wlr_output);
	wlr_renderer_end(renderer);
	wlr_output_commit(wlr_output); //otherwise failed to compile with the new wlroots
}

static void wc_output_destroy(struct wl_listener* listener, void* data) {
	struct wc_output* output = wl_container_of(listener, output, destroy);
	struct wc_server* server = output->server;
	wl_list_remove(&output->link);

	wl_list_remove(&output->frame.link);
	wl_list_remove(&output->destroy.link);

	if (server->active_output == output) {
		server->active_output = NULL;
		if (!wl_list_empty(&server->outputs)) {
			server->active_output = wl_container_of(
					server->outputs.prev, server->active_output, link);
		}
	}

	free(output);
}

static void wc_new_output(struct wl_listener* listener, void* data) {
	struct wc_server* server = wl_container_of(listener, server, new_output);
	struct wlr_output* output = data;

	if (!wl_list_empty(&output->modes)) {
		struct wlr_output_mode* mode =
			wl_container_of(output->modes.prev, mode, link);
		wlr_output_set_mode(output, mode);
	}

	struct wc_output* wc_output = calloc(1, sizeof(struct wc_output));
	wc_output->output = output;
	wc_output->server = server;
	output->data = wc_output;

	size_t len = sizeof(wc_output->layers) / sizeof(wc_output->layers[0]);
	for (size_t i = 0; i < len; i++) {
		wl_list_init(&wc_output->layers[i]);
	}

	wc_output->frame.notify = wc_output_frame;
	wl_signal_add(&output->events.frame, &wc_output->frame);
	wc_output->destroy.notify = wc_output_destroy;
	wl_signal_add(&output->events.destroy, &wc_output->destroy);

	wl_list_insert(&server->outputs, &wc_output->link);

	if (server->active_output == NULL) {
		server->active_output = wc_output;
	}

	wlr_output_layout_add_auto(server->output_layout, output);
	wlr_output_create_global(output);

	wc_layer_shell_arrange_layers(wc_output);
}


struct wc_output* wc_get_active_output(struct wc_server* server) {
	if (wl_list_empty(&server->outputs)) {
		return NULL;
	}
	struct wc_output* output = server->active_output;
	if (output == NULL) {
		output = wl_container_of(server->outputs.prev, output, link);
	}
	return output;
}

void wc_init_output(struct wc_server* server) {
	server->output_layout = wlr_output_layout_create();
	wl_list_init(&server->outputs);
	server->new_output.notify = wc_new_output;
	wl_signal_add(&server->backend->events.new_output, &server->new_output);
}
