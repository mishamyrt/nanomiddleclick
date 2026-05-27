#ifndef NANOMIDDLECLICK_INPUT_H
#define NANOMIDDLECLICK_INPUT_H

#include <stdbool.h>
#include <stdint.h>

#include "MultitouchSupport.h"

typedef void (*NMCTouchCallback)(const MTTouch *touches, uintptr_t touchCount, double timestamp, int32_t frame, uint32_t source_kind);
typedef uint32_t (*NMCMouseEventCallback)(uint32_t kind);
typedef void (*NMCSystemEventCallback)(uint32_t kind);
typedef void (*NMCSignalEventCallback)(uint32_t kind);

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
    NMCSystemEventKindDisplayReconfigured = 2,
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

#endif
