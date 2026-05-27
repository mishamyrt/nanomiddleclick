#ifndef NANOMIDDLECLICK_APP_MONITOR_H
#define NANOMIDDLECLICK_APP_MONITOR_H

#include <stdint.h>

typedef void (*NMCAppMonitorEventCallback)(uint32_t kind);
typedef void (*NMCFrontmostBundleCallback)(const char *bundleID);

typedef uint32_t NMCAppMonitorEventKind;
enum {
    NMCAppMonitorEventKindWake = 1,
};

void NMCStartWorkspaceMonitor(
    NMCAppMonitorEventCallback event_callback,
    NMCFrontmostBundleCallback frontmost_bundle_callback
);
void NMCSetFrontmostBundleMonitorEnabled(
    NMCFrontmostBundleCallback frontmost_bundle_callback
);
void NMCStopWorkspaceMonitor(void);

#endif
