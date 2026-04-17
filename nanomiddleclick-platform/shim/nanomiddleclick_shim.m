#import <AppKit/AppKit.h>
#import <ApplicationServices/ApplicationServices.h>
#import <CoreFoundation/CoreFoundation.h>
#import <Foundation/Foundation.h>
#import <IOKit/IOKitLib.h>
#import <dispatch/dispatch.h>
#import <signal.h>
#import <stdbool.h>
#import <stdint.h>
#import <stdlib.h>
#import <string.h>

#import "MultitouchSupport.h"

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

typedef NS_ENUM(uint32_t, NMCMouseEventKind) {
    NMCMouseEventKindLeftDown = 1,
    NMCMouseEventKindLeftUp = 2,
    NMCMouseEventKindRightDown = 3,
    NMCMouseEventKindRightUp = 4,
};

typedef NS_ENUM(uint32_t, NMCMouseAction) {
    NMCMouseActionPass = 0,
    NMCMouseActionRewriteDown = 1,
    NMCMouseActionRewriteUp = 2,
};

typedef NS_ENUM(uint32_t, NMCSystemEventKind) {
    NMCSystemEventKindDeviceAdded = 1,
    NMCSystemEventKindWake = 2,
    NMCSystemEventKindDisplayReconfigured = 3,
};

typedef NS_ENUM(uint32_t, NMCSignalKind) {
    NMCSignalKindReload = 1,
};

typedef NS_ENUM(uint32_t, NMCTouchDeviceKind) {
    NMCTouchDeviceKindUnknown = 0,
    NMCTouchDeviceKindMouse = 1,
    NMCTouchDeviceKindTrackpad = 2,
};

static NSString *const NMCDefaultsDomain = @"co.myrt.nanomiddleclick";

static NMCTouchCallback g_touch_callback = NULL;
static NMCMouseEventCallback g_mouse_event_callback = NULL;
static NMCSystemEventCallback g_system_event_callback = NULL;
static NMCSignalEventCallback g_signal_event_callback = NULL;
static NMCFrontmostBundleCallback g_frontmost_bundle_callback = NULL;

static NSArray *g_devices = nil;
static NSMutableDictionary<NSValue *, NSNumber *> *g_touch_device_kinds = nil;
static CFMachPortRef g_event_tap = NULL;
static CFRunLoopSourceRef g_event_tap_source = NULL;
static CFRunLoopRef g_run_loop = NULL;

static IONotificationPortRef g_device_notification_port = NULL;
static io_iterator_t g_device_iterator = IO_OBJECT_NULL;
static id g_wake_observer = nil;
static id g_activation_observer = nil;
static dispatch_source_t g_reload_signal_source = nil;
static dispatch_source_t g_term_signal_source = nil;
static dispatch_source_t g_int_signal_source = nil;
static bool g_display_callback_registered = false;

static bool NMCGetSystemTapToClick(void);
static void NMCDrainIterator(io_iterator_t iterator);
static void NMCStopTouchDevices(void);
static void NMCStartTouchDevices(void);
static void NMCStopEventTap(void);
static bool NMCStartEventTap(void);
static void NMCStartDeviceMonitor(void);
static void NMCStopDeviceMonitor(void);
static void NMCStartWakeObserver(void);
static void NMCStopWakeObserver(void);
static void NMCStartActivationObserver(void);
static void NMCStopActivationObserver(void);
static void NMCStartDisplayObserver(void);
static void NMCStopDisplayObserver(void);
static void NMCStartSignalMonitor(void);
static void NMCStopSignalMonitor(void);
static void NMCNotifyFrontmostBundle(void);
static NSDictionary *NMCReadDefaultsDomain(void);
static uint32_t NMCParseMouseClickMode(id raw_value);
static bool NMCDeviceHasMousePreferences(MTDeviceRef device);
static NMCTouchDeviceKind NMCClassifyTouchDevice(MTDeviceRef device);

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
    NSNumber *cached_kind = [g_touch_device_kinds objectForKey:[NSValue valueWithPointer:device]];
    if (cached_kind != nil) {
        return (NMCTouchDeviceKind)cached_kind.unsignedIntValue;
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

bool nmc_is_accessibility_trusted(bool prompt) {
    CFDictionaryRef options = (__bridge CFDictionaryRef)@{
        (__bridge NSString *)kAXTrustedCheckOptionPrompt: @(prompt),
    };
    return AXIsProcessTrustedWithOptions(options);
}

bool nmc_get_system_tap_to_click(void) {
    return NMCGetSystemTapToClick();
}

bool nmc_load_config(NMCConfigSnapshot *out_snapshot) {
    if (out_snapshot == NULL) {
        return false;
    }

    memset(out_snapshot, 0, sizeof(*out_snapshot));

    NSDictionary *domain = NMCReadDefaultsDomain();

    NSNumber *fingers = [domain objectForKey:@"fingers"];
    NSNumber *allow_more_fingers = [domain objectForKey:@"allowMoreFingers"];
    NSNumber *max_distance_delta = [domain objectForKey:@"maxDistanceDelta"];
    NSNumber *max_time_delta = [domain objectForKey:@"maxTimeDelta"];
    id tap_to_click = [domain objectForKey:@"tapToClick"];
    id mouse_click_mode = [domain objectForKey:@"mouseClickMode"];
    id ignored = [domain objectForKey:@"ignoredAppBundles"];

    out_snapshot->fingers = fingers != nil ? fingers.longLongValue : 3;
    out_snapshot->allow_more_fingers = allow_more_fingers != nil ? allow_more_fingers.boolValue : false;
    out_snapshot->max_distance_delta = max_distance_delta != nil ? max_distance_delta.doubleValue : 0.05;
    out_snapshot->max_time_delta_ms = max_time_delta != nil ? max_time_delta.longLongValue : 300;
    out_snapshot->tap_to_click = tap_to_click != nil ? [tap_to_click boolValue] : NMCGetSystemTapToClick();
    out_snapshot->mouse_click_mode = NMCParseMouseClickMode(mouse_click_mode);

    NSArray<NSString *> *bundle_ids = nil;
    if ([ignored isKindOfClass:[NSArray class]]) {
        bundle_ids = ignored;
    } else if ([ignored isKindOfClass:[NSSet class]]) {
        bundle_ids = [(NSSet *)ignored allObjects];
    }

    if (bundle_ids.count == 0) {
        return true;
    }

    out_snapshot->ignored_app_bundles_len = (uintptr_t)bundle_ids.count;
    out_snapshot->ignored_app_bundles = calloc(bundle_ids.count, sizeof(char *));
    if (out_snapshot->ignored_app_bundles == NULL) {
        out_snapshot->ignored_app_bundles_len = 0;
        return false;
    }

    for (NSUInteger index = 0; index < bundle_ids.count; index += 1) {
        NSString *bundle_id = bundle_ids[index];
        if (![bundle_id isKindOfClass:[NSString class]]) {
            continue;
        }

        out_snapshot->ignored_app_bundles[index] = strdup(bundle_id.UTF8String);
    }

    return true;
}

void nmc_free_config(NMCConfigSnapshot *snapshot) {
    if (snapshot == NULL) {
        return;
    }

    if (snapshot->ignored_app_bundles != NULL) {
        for (uintptr_t index = 0; index < snapshot->ignored_app_bundles_len; index += 1) {
            free(snapshot->ignored_app_bundles[index]);
        }
        free(snapshot->ignored_app_bundles);
    }

    memset(snapshot, 0, sizeof(*snapshot));
}

bool nmc_restart_listeners(void);

bool nmc_start(
    NMCTouchCallback touch_callback,
    NMCMouseEventCallback mouse_callback,
    NMCSystemEventCallback system_callback,
    NMCSignalEventCallback signal_callback,
    NMCFrontmostBundleCallback frontmost_bundle_callback
) {
    g_touch_callback = touch_callback;
    g_mouse_event_callback = mouse_callback;
    g_system_event_callback = system_callback;
    g_signal_event_callback = signal_callback;
    g_frontmost_bundle_callback = frontmost_bundle_callback;
    g_run_loop = CFRunLoopGetCurrent();

    NMCStartDeviceMonitor();
    NMCStartWakeObserver();
    NMCStartActivationObserver();
    NMCStartDisplayObserver();
    NMCStartSignalMonitor();

    return nmc_restart_listeners();
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
    NMCStopWakeObserver();
    NMCStopActivationObserver();
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

static bool NMCGetSystemTapToClick(void) {
    Boolean found = false;
    Boolean value = CFPreferencesGetAppBooleanValue(
        CFSTR("Clicking"),
        CFSTR("com.apple.driver.AppleBluetoothMultitouch.trackpad"),
        &found
    );
    return found ? value : false;
}

static NSDictionary *NMCReadDefaultsDomain(void) {
    CFPreferencesAppSynchronize((__bridge CFStringRef)NMCDefaultsDomain);

    NSDictionary *domain = [[NSUserDefaults standardUserDefaults] persistentDomainForName:NMCDefaultsDomain];
    if ([domain isKindOfClass:[NSDictionary class]]) {
        return domain;
    }

    return @{};
}

static uint32_t NMCParseMouseClickMode(id raw_value) {
    if ([raw_value isKindOfClass:[NSNumber class]]) {
        return ((NSNumber *)raw_value).unsignedIntValue;
    }

    if (![raw_value isKindOfClass:[NSString class]]) {
        return 0;
    }

    NSString *value = [(NSString *)raw_value lowercaseString];
    if ([value isEqualToString:@"center"]) {
        return 1;
    }
    if ([value isEqualToString:@"disabled"]) {
        return 2;
    }
    if ([value isEqualToString:@"threefinger"]) {
        return 0;
    }

    return 0;
}

static void NMCDrainIterator(io_iterator_t iterator) {
    io_object_t object = IO_OBJECT_NULL;
    while ((object = IOIteratorNext(iterator)) != IO_OBJECT_NULL) {
        IOObjectRelease(object);
    }
}

static void NMCStopTouchDevices(void) {
    for (id device in g_devices) {
        MTDeviceRef ref = (__bridge MTDeviceRef)device;
        MTUnregisterContactFrameCallback(ref, NMCTouchFrameCallback);
        MTDeviceStop(ref);
        MTDeviceRelease(ref);
    }

    g_devices = nil;
    g_touch_device_kinds = nil;
}

static void NMCStartTouchDevices(void) {
    CFArrayRef list = MTDeviceCreateList();
    if (list == NULL) {
        g_devices = @[];
        g_touch_device_kinds = [NSMutableDictionary new];
        return;
    }

    g_devices = CFBridgingRelease(list);
    g_touch_device_kinds = [NSMutableDictionary new];
    for (id device in g_devices) {
        MTDeviceRef ref = (__bridge MTDeviceRef)device;
        const NMCTouchDeviceKind kind = NMCClassifyTouchDevice(ref);
        [g_touch_device_kinds setObject:@(kind) forKey:[NSValue valueWithPointer:ref]];
        MTRegisterContactFrameCallback(ref, NMCTouchFrameCallback);
        MTDeviceStart(ref, 0);
    }
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

static void NMCStartWakeObserver(void) {
    if (g_wake_observer != nil) {
        return;
    }

    g_wake_observer = [NSWorkspace.sharedWorkspace.notificationCenter
        addObserverForName:NSWorkspaceDidWakeNotification
        object:nil
        queue:nil
        usingBlock:^(__unused NSNotification *note) {
            if (g_system_event_callback != NULL) {
                g_system_event_callback(NMCSystemEventKindWake);
            }
        }];
}

static void NMCStopWakeObserver(void) {
    if (g_wake_observer == nil) {
        return;
    }

    [NSWorkspace.sharedWorkspace.notificationCenter removeObserver:g_wake_observer];
    g_wake_observer = nil;
}

static void NMCStartActivationObserver(void) {
    if (g_activation_observer != nil) {
        return;
    }

    g_activation_observer = [NSWorkspace.sharedWorkspace.notificationCenter
        addObserverForName:NSWorkspaceDidActivateApplicationNotification
        object:nil
        queue:nil
        usingBlock:^(__unused NSNotification *note) {
            NMCNotifyFrontmostBundle();
        }];

    NMCNotifyFrontmostBundle();
}

static void NMCStopActivationObserver(void) {
    if (g_activation_observer == nil) {
        return;
    }

    [NSWorkspace.sharedWorkspace.notificationCenter removeObserver:g_activation_observer];
    g_activation_observer = nil;
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
    if (g_reload_signal_source != nil) {
        return;
    }

    signal(SIGHUP, SIG_IGN);
    signal(SIGTERM, SIG_IGN);
    signal(SIGINT, SIG_IGN);

    g_reload_signal_source = dispatch_source_create(DISPATCH_SOURCE_TYPE_SIGNAL, SIGHUP, 0, dispatch_get_main_queue());
    dispatch_source_set_event_handler(g_reload_signal_source, ^{
        if (g_signal_event_callback != NULL) {
            g_signal_event_callback(NMCSignalKindReload);
        }
    });
    dispatch_resume(g_reload_signal_source);

    g_term_signal_source = dispatch_source_create(DISPATCH_SOURCE_TYPE_SIGNAL, SIGTERM, 0, dispatch_get_main_queue());
    dispatch_source_set_event_handler(g_term_signal_source, ^{
        if (g_run_loop != NULL) {
            CFRunLoopStop(g_run_loop);
        }
    });
    dispatch_resume(g_term_signal_source);

    g_int_signal_source = dispatch_source_create(DISPATCH_SOURCE_TYPE_SIGNAL, SIGINT, 0, dispatch_get_main_queue());
    dispatch_source_set_event_handler(g_int_signal_source, ^{
        if (g_run_loop != NULL) {
            CFRunLoopStop(g_run_loop);
        }
    });
    dispatch_resume(g_int_signal_source);
}

static void NMCStopSignalMonitor(void) {
    if (g_reload_signal_source != nil) {
        dispatch_source_cancel(g_reload_signal_source);
        g_reload_signal_source = nil;
    }
    if (g_term_signal_source != nil) {
        dispatch_source_cancel(g_term_signal_source);
        g_term_signal_source = nil;
    }
    if (g_int_signal_source != nil) {
        dispatch_source_cancel(g_int_signal_source);
        g_int_signal_source = nil;
    }
}

static void NMCNotifyFrontmostBundle(void) {
    if (g_frontmost_bundle_callback == NULL) {
        return;
    }

    NSRunningApplication *application = NSWorkspace.sharedWorkspace.frontmostApplication;
    NSString *bundle_id = application.bundleIdentifier;
    const char *utf8_bundle_id = bundle_id.length > 0 ? bundle_id.UTF8String : NULL;
    g_frontmost_bundle_callback(utf8_bundle_id);
}
