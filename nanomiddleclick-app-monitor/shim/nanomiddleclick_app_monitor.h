#ifndef NANOMIDDLECLICK_APP_MONITOR_H
#define NANOMIDDLECLICK_APP_MONITOR_H

typedef void (*NMCWakeCallback)(void);
typedef void (*NMCFrontmostBundleCallback)(const char *bundleID);

void NMCStartWorkspaceMonitor(
    NMCWakeCallback wake_callback,
    NMCFrontmostBundleCallback frontmost_bundle_callback
);
void NMCSetFrontmostBundleMonitorEnabled(
    NMCFrontmostBundleCallback frontmost_bundle_callback
);
void NMCStopWorkspaceMonitor(void);

#endif
