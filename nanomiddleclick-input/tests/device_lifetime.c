#include <assert.h>
#include <stdbool.h>
#include <stdlib.h>

static bool fail_allocation = false;

static void *test_calloc(size_t count, size_t size) {
    return fail_allocation ? NULL : calloc(count, size);
}

#define calloc test_calloc
#include "../shim/input_runtime.c"
#undef calloc

// Model the system's ownership independently of the list and the input runtime.
static CFMutableDataRef device;
static bool registered = false;
static bool started = false;

CFMutableArrayRef MTDeviceCreateList(void) {
    CFMutableArrayRef list = CFArrayCreateMutable(NULL, 0, &kCFTypeArrayCallBacks);
    CFArrayAppendValue(list, device);
    return list;
}

io_service_t MTDeviceGetService(MTDeviceRef ref) {
    assert(ref == device);
    return IO_OBJECT_NULL;
}

int MTRegisterContactFrameCallback(
    MTDeviceRef ref,
    void (*callback)(MTDeviceRef, MTTouch *, int, double, int)
) {
    assert(ref == device);
    assert(callback == NMCTouchFrameCallback);
    assert(!registered);
    registered = true;
    return 0;
}

int MTUnregisterContactFrameCallback(
    MTDeviceRef ref,
    void (*callback)(MTDeviceRef, MTTouch *, int, double, int)
) {
    assert(ref == device);
    assert(callback == NMCTouchFrameCallback);
    assert(registered);
    registered = false;
    return 0;
}

int MTDeviceStart(MTDeviceRef ref, int unknown) {
    assert(ref == device);
    assert(unknown == 0);
    assert(registered && !started);
    started = true;
    return 0;
}

int MTDeviceStop(MTDeviceRef ref) {
    assert(ref == device);
    assert(!registered && started);
    started = false;
    return 0;
}

void MTDeviceRelease(MTDeviceRef ref) {
    assert(ref == device);
    assert(!registered && !started);
    CFRelease(ref);
}

int main(void) {
    device = CFDataCreateMutable(NULL, 0);
    assert(device != NULL);

    for (int iteration = 0; iteration < 100; iteration += 1) {
        NMCStartTouchDevices();
        assert(registered && started);
        // The runtime must own a reference after its temporary list is released.
        assert(CFGetRetainCount(device) == 2);
        NMCStopTouchDevices();
        assert(!registered && !started);
        assert(g_touch_devices == NULL && g_touch_device_count == 0);
        assert(CFGetRetainCount(device) == 1);
    }

    fail_allocation = true;
    NMCStartTouchDevices();
    assert(!registered && !started);
    assert(g_touch_devices == NULL && g_touch_device_count == 0);
    assert(CFGetRetainCount(device) == 1);
    NMCStopTouchDevices();

    CFRelease(device);
    return 0;
}
