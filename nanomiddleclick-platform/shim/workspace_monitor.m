#import <AppKit/AppKit.h>

#import "nanomiddleclick_shim.h"

static NMCSystemEventCallback g_system_event_callback = NULL;
static NMCFrontmostBundleCallback g_frontmost_bundle_callback = NULL;
static id g_wake_observer = nil;
static id g_activation_observer = nil;

static void NMCStartWakeObserver(void);
static void NMCStopWakeObserver(void);
static void NMCStartActivationObserver(void);
static void NMCStopActivationObserver(void);
static void NMCNotifyFrontmostBundle(void);

void NMCStartWorkspaceMonitor(
    NMCSystemEventCallback system_callback,
    NMCFrontmostBundleCallback frontmost_bundle_callback
) {
    g_system_event_callback = system_callback;

    NMCStartWakeObserver();
    NMCSetFrontmostBundleMonitorEnabled(frontmost_bundle_callback);
}

void NMCSetFrontmostBundleMonitorEnabled(
    NMCFrontmostBundleCallback frontmost_bundle_callback
) {
    g_frontmost_bundle_callback = frontmost_bundle_callback;

    if (frontmost_bundle_callback == NULL) {
        NMCStopActivationObserver();
    } else {
        NMCStartActivationObserver();
    }
}

void NMCStopWorkspaceMonitor(void) {
    NMCStopWakeObserver();
    NMCStopActivationObserver();
    g_system_event_callback = NULL;
    g_frontmost_bundle_callback = NULL;
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

static void NMCNotifyFrontmostBundle(void) {
    if (g_frontmost_bundle_callback == NULL) {
        return;
    }

    NSRunningApplication *application = NSWorkspace.sharedWorkspace.frontmostApplication;
    NSString *bundle_id = application.bundleIdentifier;
    const char *utf8_bundle_id = bundle_id.length > 0 ? bundle_id.UTF8String : NULL;
    g_frontmost_bundle_callback(utf8_bundle_id);
}
