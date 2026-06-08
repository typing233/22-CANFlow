/**
 * Example CANFlow Plugin: Counter Analyzer
 *
 * Detects monotonic counter fields in CAN frame payloads.
 * Alerts when a counter value is skipped (possible frame loss or injection).
 *
 * Build: gcc -shared -fPIC -o counter_analyzer.so example_analyzer.c
 */

#include "../plugin_sdk.h"
#include <stdlib.h>
#include <stdio.h>
#include <string.h>

#define MAX_TRACKED_IDS 256

typedef struct {
    uint32_t id;
    uint8_t last_counter;
    uint8_t counter_byte_idx;
    int initialized;
    int skip_count;
} id_tracker_t;

typedef struct {
    id_tracker_t trackers[MAX_TRACKED_IDS];
    int num_trackers;
    char alert_msg[256];
    char alert_details[512];
} plugin_state_t;

static void* plugin_create(const char* config_json) {
    plugin_state_t* state = (plugin_state_t*)calloc(1, sizeof(plugin_state_t));
    return state;
}

static void plugin_destroy(void* state) {
    free(state);
}

static id_tracker_t* find_or_create_tracker(plugin_state_t* state, uint32_t id) {
    for (int i = 0; i < state->num_trackers; i++) {
        if (state->trackers[i].id == id) {
            return &state->trackers[i];
        }
    }
    if (state->num_trackers >= MAX_TRACKED_IDS) {
        return NULL;
    }
    id_tracker_t* t = &state->trackers[state->num_trackers++];
    t->id = id;
    t->initialized = 0;
    t->counter_byte_idx = 0;  /* Assume counter is first byte by default */
    t->skip_count = 0;
    return t;
}

static uint32_t plugin_ingest(void* vstate, const canflow_frame_t* frame,
                              canflow_alert_t* out_alerts, uint32_t max_alerts) {
    plugin_state_t* state = (plugin_state_t*)vstate;

    if (frame->dlc == 0 || max_alerts == 0) return 0;

    id_tracker_t* tracker = find_or_create_tracker(state, frame->id);
    if (!tracker) return 0;

    uint8_t current = frame->data[tracker->counter_byte_idx];

    if (!tracker->initialized) {
        tracker->last_counter = current;
        tracker->initialized = 1;
        return 0;
    }

    uint8_t expected = (tracker->last_counter + 1) & 0xFF;
    tracker->last_counter = current;

    if (current != expected && current != tracker->last_counter) {
        tracker->skip_count++;

        snprintf(state->alert_msg, sizeof(state->alert_msg),
                 "counter skip on 0x%03X: expected 0x%02X got 0x%02X (skips: %d)",
                 frame->id, expected, current, tracker->skip_count);

        snprintf(state->alert_details, sizeof(state->alert_details),
                 "{\"frame_id\": %u, \"expected\": %u, \"actual\": %u, \"total_skips\": %d}",
                 frame->id, expected, current, tracker->skip_count);

        out_alerts[0].severity = (tracker->skip_count > 10) ?
            CANFLOW_SEVERITY_WARNING : CANFLOW_SEVERITY_INFO;
        out_alerts[0].message = state->alert_msg;
        out_alerts[0].details_json = state->alert_details;
        return 1;
    }

    return 0;
}

static uint32_t plugin_tick(void* state, canflow_alert_t* out_alerts, uint32_t max_alerts) {
    return 0;
}

static void plugin_reset(void* vstate) {
    plugin_state_t* state = (plugin_state_t*)vstate;
    state->num_trackers = 0;
}

static const canflow_plugin_vtable_t vtable = {
    .abi_version = CANFLOW_PLUGIN_ABI_VERSION,
    .name = "counter_analyzer",
    .version = "0.1.0",
    .create = plugin_create,
    .destroy = plugin_destroy,
    .ingest = plugin_ingest,
    .tick = plugin_tick,
    .reset = plugin_reset,
};

const canflow_plugin_vtable_t* canflow_plugin_init(void) {
    return &vtable;
}
