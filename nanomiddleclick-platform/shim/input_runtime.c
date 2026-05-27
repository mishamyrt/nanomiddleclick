#include <ApplicationServices/ApplicationServices.h>
#include <CoreFoundation/CoreFoundation.h>
#include <IOKit/IOKitLib.h>
#include <dispatch/dispatch.h>
#include <signal.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>

#include "nanomiddleclick_shim.h"

typedef struct {
    MTDeviceRef device;
    NMCTouchDeviceKind kind;
} NMCTouchDeviceEntry;

static NMCTouchCallback g_touch_callback = NULL;
static NMCMouseEventCallback g_mouse_event_callback = NULL;
static NMCSystemEventCallback g_system_event_callback = NULL;
static NMCSignalEventCallback g_signal_event_callback = NULL;

static NMCTouchDeviceEntry *g_touch_devices = NULL;
static size_t g_touch_device_count = 0;
static CFMachPortRef g_event_tap = NULL;
static CFRunLoopSourceRef g_event_tap_source = NULL;
static CFRunLoopRef g_run_loop = NULL;

static IONotificationPortRef g_device_notification_port = NULL;
static io_iterator_t g_device_iterator = IO_OBJECT_NULL;
static dispatch_source_t g_reload_signal_source = NULL;
static dispatch_source_t g_term_signal_source = NULL;
static dispatch_source_t g_int_signal_source = NULL;
static bool g_display_callback_registered = false;

static void NMCDrainIterator(io_iterator_t iterator);
static void NMCReleaseTouchDeviceList(CFArrayRef list);
static void NMCStopTouchDevices(void);
static void NMCStartTouchDevices(void);
static void NMCStopEventTap(void);
static bool NMCStartEventTap(void);
static void NMCStartDeviceMonitor(void);
static void NMCStopDeviceMonitor(void);
static void NMCStartDisplayObserver(void);
static void NMCStopDisplayObserver(void);
static void NMCStartSignalMonitor(void);
static void NMCStopSignalMonitor(void);
static bool NMCDeviceHasMousePreferences(MTDeviceRef device);
static NMCTouchDeviceKind NMCClassifyTouchDevice(MTDeviceRef device);
static void NMCHandleReloadSignal(void *context);
static void NMCHandleStopSignal(void *context);

static CGEventRef NMCMouseTapCallback(CGEventTapProxy proxy, CGEventType type, CGEventRef event, void *userInfo) {
    (void)proxy;
    (void)userInfo;

    if (type == kCGEventTapDisabledByTimeout || type == kCGEventTapDisabledByUserInput) {
        if (g_event_tap != NULL) {
            CGEventTapEnable(g_event_tap, true);
        }
        return event;
    }

    if (g_mouse_event_callback == NULL) {
        return event;
    }

    uint32_t kind = 0;
    switch (type) {
        case kCGEventLeftMouseDown:
            kind = NMCMouseEventKindLeftDown;
            break;
        case kCGEventLeftMouseUp:
            kind = NMCMouseEventKindLeftUp;
            break;
        case kCGEventRightMouseDown:
            kind = NMCMouseEventKindRightDown;
            break;
        case kCGEventRightMouseUp:
            kind = NMCMouseEventKindRightUp;
            break;
        default:
            return event;
    }

    uint32_t action = g_mouse_event_callback(kind);
    if (action == NMCMouseActionRewriteDown) {
        CGEventSetType(event, kCGEventOtherMouseDown);
        CGEventSetIntegerValueField(event, kCGMouseEventButtonNumber, (int64_t)kCGMouseButtonCenter);
    } else if (action == NMCMouseActionRewriteUp) {
        CGEventSetType(event, kCGEventOtherMouseUp);
        CGEventSetIntegerValueField(event, kCGMouseEventButtonNumber, (int64_t)kCGMouseButtonCenter);
    }

    return event;
}

static void NMCTouchFrameCallback(MTDeviceRef device, MTTouch touches[], int numTouches, double timestamp, int frame) {
    if (g_touch_callback == NULL) {
        return;
    }

    const MTTouch *pointer = numTouches > 0 ? touches : NULL;
    const uint32_t source_kind = NMCClassifyTouchDevice(device);
    g_touch_callback(pointer, (uintptr_t)(numTouches > 0 ? numTouches : 0), timestamp, frame, source_kind);
}

static bool NMCDeviceHasMousePreferences(MTDeviceRef device) {
    const io_service_t service = MTDeviceGetService(device);
    if (service == IO_OBJECT_NULL) {
        return false;
    }

    CFTypeRef preferences = IORegistryEntryCreateCFProperty(
        service,
        CFSTR("MultitouchPreferences"),
        kCFAllocatorDefault,
        0
    );
    if (preferences == NULL) {
        return false;
    }

    bool has_mouse_preferences = false;
    if (CFGetTypeID(preferences) == CFDictionaryGetTypeID()) {
        has_mouse_preferences = CFDictionaryContainsKey(
            (CFDictionaryRef)preferences,
            CFSTR("MouseButtonMode")
        );
    }

    CFRelease(preferences);
    return has_mouse_preferences;
}

static NMCTouchDeviceKind NMCClassifyTouchDevice(MTDeviceRef device) {
    for (size_t index = 0; index < g_touch_device_count; index += 1) {
        if (g_touch_devices[index].device == device) {
            return g_touch_devices[index].kind;
        }
    }

    const io_service_t service = MTDeviceGetService(device);
    if (service == IO_OBJECT_NULL) {
        return NMCTouchDeviceKindUnknown;
    }

    return NMCDeviceHasMousePreferences(device)
        ? NMCTouchDeviceKindMouse
        : NMCTouchDeviceKindTrackpad;
}

static void NMCMultitouchDeviceAdded(void *refcon, io_iterator_t iterator) {
    (void)refcon;
    NMCDrainIterator(iterator);
    if (g_system_event_callback != NULL) {
        g_system_event_callback(NMCSystemEventKindDeviceAdded);
    }
}

static void NMCDisplayReconfigurationCallback(CGDirectDisplayID display, CGDisplayChangeSummaryFlags flags, void *userInfo) {
    (void)display;
    (void)userInfo;

    if (g_system_event_callback == NULL) {
        return;
    }

    const bool changed =
        (flags & kCGDisplaySetModeFlag) ||
        (flags & kCGDisplayAddFlag) ||
        (flags & kCGDisplayRemoveFlag) ||
        (flags & kCGDisplayDisabledFlag);

    if (changed) {
        g_system_event_callback(NMCSystemEventKindDisplayReconfigured);
    }
}

bool nmc_restart_listeners(void);

bool nmc_start(
    NMCTouchCallback touch_callback,
    NMCMouseEventCallback mouse_callback,
    NMCSystemEventCallback system_callback,
    NMCSignalEventCallback signal_callback,
    NMCFrontmostBundleCallback frontmost_bundle_callback,
    bool monitor_frontmost_bundle
) {
    g_touch_callback = touch_callback;
    g_mouse_event_callback = mouse_callback;
    g_system_event_callback = system_callback;
    g_signal_event_callback = signal_callback;
    g_run_loop = CFRunLoopGetCurrent();

    NMCStartDeviceMonitor();
    NMCStartWorkspaceMonitor(
        system_callback,
        monitor_frontmost_bundle ? frontmost_bundle_callback : NULL
    );
    NMCStartDisplayObserver();
    NMCStartSignalMonitor();

    return nmc_restart_listeners();
}

void nmc_set_frontmost_bundle_monitor_enabled(
    NMCFrontmostBundleCallback frontmost_bundle_callback,
    bool enabled
) {
    NMCSetFrontmostBundleMonitorEnabled(enabled ? frontmost_bundle_callback : NULL);
}

bool nmc_restart_listeners(void) {
    NMCStopTouchDevices();
    NMCStopEventTap();
    NMCStartTouchDevices();
    return NMCStartEventTap();
}

void nmc_stop(void) {
    NMCStopTouchDevices();
    NMCStopEventTap();
    NMCStopDeviceMonitor();
    NMCStopWorkspaceMonitor();
    NMCStopDisplayObserver();
    NMCStopSignalMonitor();
}

void nmc_run_loop_run(void) {
    g_run_loop = CFRunLoopGetCurrent();
    CFRunLoopRun();
}

void nmc_post_middle_mouse_click(void) {
    CGEventRef source_event = CGEventCreate(NULL);
    CGPoint location = CGPointZero;

    if (source_event != NULL) {
        location = CGEventGetLocation(source_event);
        CFRelease(source_event);
    }

    CGEventRef down = CGEventCreateMouseEvent(NULL, kCGEventOtherMouseDown, location, kCGMouseButtonCenter);
    CGEventRef up = CGEventCreateMouseEvent(NULL, kCGEventOtherMouseUp, location, kCGMouseButtonCenter);

    if (down != NULL) {
        CGEventPost(kCGHIDEventTap, down);
        CFRelease(down);
    }
    if (up != NULL) {
        CGEventPost(kCGHIDEventTap, up);
        CFRelease(up);
    }
}

static void NMCDrainIterator(io_iterator_t iterator) {
    io_object_t object = IO_OBJECT_NULL;
    while ((object = IOIteratorNext(iterator)) != IO_OBJECT_NULL) {
        IOObjectRelease(object);
    }
}

static void NMCReleaseTouchDeviceList(CFArrayRef list) {
    if (list == NULL) {
        return;
    }

    const CFIndex count = CFArrayGetCount(list);
    for (CFIndex index = 0; index < count; index += 1) {
        MTDeviceRef device = (MTDeviceRef)CFArrayGetValueAtIndex(list, index);
        if (device != NULL) {
            MTDeviceRelease(device);
        }
    }
}

static void NMCStopTouchDevices(void) {
    for (size_t index = 0; index < g_touch_device_count; index += 1) {
        MTDeviceRef ref = g_touch_devices[index].device;
        if (ref == NULL) {
            continue;
        }

        MTUnregisterContactFrameCallback(ref, NMCTouchFrameCallback);
        MTDeviceStop(ref);
        MTDeviceRelease(ref);
    }

    free(g_touch_devices);
    g_touch_devices = NULL;
    g_touch_device_count = 0;
}

static void NMCStartTouchDevices(void) {
    CFArrayRef list = MTDeviceCreateList();
    if (list == NULL) {
        return;
    }

    const CFIndex count = CFArrayGetCount(list);
    if (count <= 0) {
        CFRelease(list);
        return;
    }

    g_touch_devices = calloc((size_t)count, sizeof(*g_touch_devices));
    if (g_touch_devices == NULL) {
        NMCReleaseTouchDeviceList(list);
        CFRelease(list);
        return;
    }

    for (CFIndex index = 0; index < count; index += 1) {
        MTDeviceRef ref = (MTDeviceRef)CFArrayGetValueAtIndex(list, index);
        const NMCTouchDeviceKind kind = NMCClassifyTouchDevice(ref);
        g_touch_devices[g_touch_device_count++] = (NMCTouchDeviceEntry){
            .device = ref,
            .kind = kind,
        };
        MTRegisterContactFrameCallback(ref, NMCTouchFrameCallback);
        MTDeviceStart(ref, 0);
    }

    CFRelease(list);
}

static void NMCStopEventTap(void) {
    if (g_event_tap != NULL) {
        CGEventTapEnable(g_event_tap, false);
    }

    if (g_event_tap_source != NULL && g_run_loop != NULL) {
        CFRunLoopRemoveSource(g_run_loop, g_event_tap_source, kCFRunLoopCommonModes);
        CFRelease(g_event_tap_source);
        g_event_tap_source = NULL;
    }

    if (g_event_tap != NULL) {
        CFMachPortInvalidate(g_event_tap);
        CFRelease(g_event_tap);
        g_event_tap = NULL;
    }
}

static bool NMCStartEventTap(void) {
    CGEventMask mask =
        CGEventMaskBit(kCGEventLeftMouseDown) |
        CGEventMaskBit(kCGEventLeftMouseUp) |
        CGEventMaskBit(kCGEventRightMouseDown) |
        CGEventMaskBit(kCGEventRightMouseUp);

    g_event_tap = CGEventTapCreate(
        kCGHIDEventTap,
        kCGHeadInsertEventTap,
        kCGEventTapOptionDefault,
        mask,
        NMCMouseTapCallback,
        NULL
    );

    if (g_event_tap == NULL) {
        return false;
    }

    g_event_tap_source = CFMachPortCreateRunLoopSource(kCFAllocatorDefault, g_event_tap, 0);
    if (g_event_tap_source == NULL) {
        CFMachPortInvalidate(g_event_tap);
        CFRelease(g_event_tap);
        g_event_tap = NULL;
        return false;
    }

    if (g_run_loop == NULL) {
        g_run_loop = CFRunLoopGetCurrent();
    }

    CFRunLoopAddSource(g_run_loop, g_event_tap_source, kCFRunLoopCommonModes);
    CGEventTapEnable(g_event_tap, true);
    return true;
}

static void NMCStartDeviceMonitor(void) {
    if (g_device_notification_port != NULL) {
        return;
    }

    g_device_notification_port = IONotificationPortCreate(kIOMainPortDefault);
    if (g_device_notification_port == NULL) {
        return;
    }

    CFRunLoopSourceRef source = IONotificationPortGetRunLoopSource(g_device_notification_port);
    if (source != NULL) {
        CFRunLoopAddSource(g_run_loop, source, kCFRunLoopDefaultMode);
    }

    kern_return_t status = IOServiceAddMatchingNotification(
        g_device_notification_port,
        kIOFirstMatchNotification,
        IOServiceMatching("AppleMultitouchDevice"),
        NMCMultitouchDeviceAdded,
        NULL,
        &g_device_iterator
    );

    if (status != KERN_SUCCESS) {
        NMCStopDeviceMonitor();
        return;
    }

    NMCDrainIterator(g_device_iterator);
}

static void NMCStopDeviceMonitor(void) {
    if (g_device_iterator != IO_OBJECT_NULL) {
        IOObjectRelease(g_device_iterator);
        g_device_iterator = IO_OBJECT_NULL;
    }

    if (g_device_notification_port != NULL) {
        CFRunLoopSourceRef source = IONotificationPortGetRunLoopSource(g_device_notification_port);
        if (source != NULL && g_run_loop != NULL) {
            CFRunLoopRemoveSource(g_run_loop, source, kCFRunLoopDefaultMode);
        }
        IONotificationPortDestroy(g_device_notification_port);
        g_device_notification_port = NULL;
    }
}

static void NMCStartDisplayObserver(void) {
    if (g_display_callback_registered) {
        return;
    }

    CGDisplayRegisterReconfigurationCallback(NMCDisplayReconfigurationCallback, NULL);
    g_display_callback_registered = true;
}

static void NMCStopDisplayObserver(void) {
    if (!g_display_callback_registered) {
        return;
    }

    CGDisplayRemoveReconfigurationCallback(NMCDisplayReconfigurationCallback, NULL);
    g_display_callback_registered = false;
}

static void NMCStartSignalMonitor(void) {
    if (g_reload_signal_source != NULL) {
        return;
    }

    signal(SIGHUP, SIG_IGN);
    signal(SIGTERM, SIG_IGN);
    signal(SIGINT, SIG_IGN);

    g_reload_signal_source = dispatch_source_create(DISPATCH_SOURCE_TYPE_SIGNAL, SIGHUP, 0, dispatch_get_main_queue());
    if (g_reload_signal_source != NULL) {
        dispatch_source_set_event_handler_f(g_reload_signal_source, NMCHandleReloadSignal);
        dispatch_resume(g_reload_signal_source);
    }

    g_term_signal_source = dispatch_source_create(DISPATCH_SOURCE_TYPE_SIGNAL, SIGTERM, 0, dispatch_get_main_queue());
    if (g_term_signal_source != NULL) {
        dispatch_source_set_event_handler_f(g_term_signal_source, NMCHandleStopSignal);
        dispatch_resume(g_term_signal_source);
    }

    g_int_signal_source = dispatch_source_create(DISPATCH_SOURCE_TYPE_SIGNAL, SIGINT, 0, dispatch_get_main_queue());
    if (g_int_signal_source != NULL) {
        dispatch_source_set_event_handler_f(g_int_signal_source, NMCHandleStopSignal);
        dispatch_resume(g_int_signal_source);
    }
}

static void NMCStopSignalMonitor(void) {
    if (g_reload_signal_source != NULL) {
        dispatch_source_cancel(g_reload_signal_source);
        dispatch_release(g_reload_signal_source);
        g_reload_signal_source = NULL;
    }
    if (g_term_signal_source != NULL) {
        dispatch_source_cancel(g_term_signal_source);
        dispatch_release(g_term_signal_source);
        g_term_signal_source = NULL;
    }
    if (g_int_signal_source != NULL) {
        dispatch_source_cancel(g_int_signal_source);
        dispatch_release(g_int_signal_source);
        g_int_signal_source = NULL;
    }
}

static void NMCHandleReloadSignal(void *context) {
    (void)context;

    if (g_signal_event_callback != NULL) {
        g_signal_event_callback(NMCSignalKindReload);
    }
}

static void NMCHandleStopSignal(void *context) {
    (void)context;

    if (g_run_loop != NULL) {
        CFRunLoopStop(g_run_loop);
    }
}
