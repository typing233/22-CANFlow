#ifndef CANFLOW_PLUGIN_SDK_H
#define CANFLOW_PLUGIN_SDK_H

/**
 * CANFlow Plugin SDK
 *
 * Stable C ABI for writing custom analysis plugins.
 * Plugins are shared libraries (.so) that implement the canflow_plugin_init() symbol.
 *
 * Lifecycle:
 *   1. Host calls canflow_plugin_init() to get the vtable
 *   2. Host calls vtable->create(config_json) to create state
 *   3. Host calls vtable->ingest() for each CAN frame
 *   4. Host calls vtable->tick() every 100ms
 *   5. Host calls vtable->destroy(state) on unload
 *
 * Thread safety: The host guarantees single-threaded access to each plugin instance.
 * All string pointers in alerts must remain valid until the next call to ingest/tick/reset.
 */

#include <stdint.h>
#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

#define CANFLOW_PLUGIN_ABI_VERSION 1

typedef struct {
    uint64_t timestamp_ns;
    uint32_t id;            /* CAN ID (11 or 29 bit) */
    uint8_t  dlc;           /* Data length code (0-8) */
    uint8_t  data[8];       /* Payload bytes */
    uint8_t  is_extended;   /* 1 = 29-bit extended ID */
    uint8_t  is_error;      /* 1 = error frame */
    uint8_t  is_remote;     /* 1 = remote transmission request */
    uint8_t  _pad;
} canflow_frame_t;

typedef enum {
    CANFLOW_SEVERITY_INFO     = 0,
    CANFLOW_SEVERITY_WARNING  = 1,
    CANFLOW_SEVERITY_CRITICAL = 2,
} canflow_severity_t;

typedef struct {
    canflow_severity_t severity;
    const char* message;        /* Null-terminated, valid until next ingest/tick call */
    const char* details_json;   /* Null-terminated JSON or NULL */
} canflow_alert_t;

typedef struct {
    /* Must be CANFLOW_PLUGIN_ABI_VERSION */
    uint32_t abi_version;

    /* Plugin metadata */
    const char* name;
    const char* version;

    /**
     * Create a new plugin instance.
     * @param config_json  JSON configuration string (may be "{}" if no config)
     * @return Opaque state pointer, passed to all other functions
     */
    void* (*create)(const char* config_json);

    /**
     * Destroy a plugin instance and free all resources.
     */
    void (*destroy)(void* state);

    /**
     * Process one CAN frame.
     * @param state       Plugin state from create()
     * @param frame       The frame to analyze
     * @param out_alerts  Caller-provided buffer for alert output
     * @param max_alerts  Size of out_alerts buffer
     * @return Number of alerts written (0 to max_alerts)
     */
    uint32_t (*ingest)(void* state, const canflow_frame_t* frame,
                       canflow_alert_t* out_alerts, uint32_t max_alerts);

    /**
     * Periodic tick (called every ~100ms).
     * Use for time-based analysis that doesn't depend on individual frames.
     */
    uint32_t (*tick)(void* state,
                     canflow_alert_t* out_alerts, uint32_t max_alerts);

    /**
     * Reset all internal state (e.g., on session restart).
     */
    void (*reset)(void* state);

} canflow_plugin_vtable_t;

/**
 * Every plugin must export this symbol.
 * Called once at load time to get the plugin's vtable.
 */
const canflow_plugin_vtable_t* canflow_plugin_init(void);

#ifdef __cplusplus
}
#endif

#endif /* CANFLOW_PLUGIN_SDK_H */
