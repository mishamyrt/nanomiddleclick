#ifndef NANOMIDDLECLICK_SHIM_H
#define NANOMIDDLECLICK_SHIM_H

#include <stdbool.h>
#include <stdint.h>

#include "MultitouchSupport.h"

typedef void (*NMCTouchCallback)(const MTTouch *touches, uintptr_t touchCount, double timestamp, int32_t frame, uint32_t source_kind);
typedef uint32_t (*NMCMouseEventCallback)(uint32_t kind);
typedef void (*NMCSystemEventCallback)(uint32_t kind);
typedef void (*NMCSignalEventCallback)(uint32_t kind);
typedef void (*NMCFrontmostBundleCallback)(const char *bundleID);

typedef struct {
    int64_t fingers;
    bool allow_more_fingers;
    double max_distance_delta;
    int64_t max_time_delta_ms;
    bool tap_to_click;
    uint32_t mouse_click_mode;
    char **ignored_app_bundles;
    uintptr_t ignored_app_bundles_len;
} NMCConfigSnapshot;

typedef uint32_t NMCMouseEventKind;
enum {
    NMCMouseEventKindLeftDown = 1,
    NMCMouseEventKindLeftUp = 2,
    NMCMouseEventKindRightDown = 3,
    NMCMouseEventKindRightUp = 4,
};

typedef uint32_t NMCMouseAction;
enum {
    NMCMouseActionPass = 0,
    NMCMouseActionRewriteDown = 1,
    NMCMouseActionRewriteUp = 2,
};

typedef uint32_t NMCSystemEventKind;
enum {
    NMCSystemEventKindDeviceAdded = 1,
    NMCSystemEventKindWake = 2,
    NMCSystemEventKindDisplayReconfigured = 3,
};

typedef uint32_t NMCSignalKind;
enum {
    NMCSignalKindReload = 1,
};

typedef uint32_t NMCTouchDeviceKind;
enum {
    NMCTouchDeviceKindUnknown = 0,
    NMCTouchDeviceKindMouse = 1,
    NMCTouchDeviceKindTrackpad = 2,
};

void NMCStartWorkspaceMonitor(
    NMCSystemEventCallback system_callback,
    NMCFrontmostBundleCallback frontmost_bundle_callback
);
void NMCSetFrontmostBundleMonitorEnabled(
    NMCFrontmostBundleCallback frontmost_bundle_callback
);
void NMCStopWorkspaceMonitor(void);

#endif
