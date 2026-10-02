#include <assert.h>
#include <ApplicationServices/ApplicationServices.h>
#include <stdbool.h>
#include <stdlib.h>

static bool fail_allocation = false;
static CGPoint cursor = {300, 400};
static size_t cursor_move_count;
static bool fail_cursor_move;

static CGError test_warp_cursor(CGPoint location) {
    if (fail_cursor_move) {
        return kCGErrorFailure;
    }
    cursor = location;
    cursor_move_count += 1;
    return kCGErrorSuccess;
}

static CGEventRef test_create_event(CGEventSourceRef source) {
    CGEventRef event = CGEventCreate(source);
    assert(event != NULL);
    CGEventSetLocation(event, cursor);
    CGEventSetFlags(event, kCGEventFlagMaskShift);
    return event;
}

typedef struct {
    CGEventType type;
    CGPoint location;
    int64_t button;
    int64_t delta_x;
    int64_t delta_y;
    CGEventFlags flags;
} PostedEvent;

static PostedEvent posted[16];
static size_t posted_count;

static void test_post_event(CGEventTapLocation tap, CGEventRef event) {
    assert(tap == kCGHIDEventTap);
    assert(posted_count < sizeof(posted) / sizeof(posted[0]));
    if (CGEventGetType(event) == kCGEventOtherMouseDragged) {
        assert(CGEventGetLocation(event).x == cursor.x && CGEventGetLocation(event).y == cursor.y);
    }
    posted[posted_count++] = (PostedEvent){
        .type = CGEventGetType(event),
        .location = CGEventGetLocation(event),
        .button = CGEventGetIntegerValueField(event, kCGMouseEventButtonNumber),
        .delta_x = CGEventGetIntegerValueField(event, kCGMouseEventDeltaX),
        .delta_y = CGEventGetIntegerValueField(event, kCGMouseEventDeltaY),
        .flags = CGEventGetFlags(event),
    };
}

static void *test_calloc(size_t count, size_t size) {
    return fail_allocation ? NULL : calloc(count, size);
}

#define calloc test_calloc
#define CGEventCreate test_create_event
#define CGEventPost test_post_event
#define CGWarpMouseCursorPosition test_warp_cursor
#include "../shim/input_runtime.c"
#undef calloc
#undef CGEventCreate
#undef CGEventPost
#undef CGWarpMouseCursorPosition

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

static uint32_t mouse_action;

static uint32_t test_mouse_callback(uint32_t kind) {
    assert(kind == NMCMouseEventKindMoved);
    return mouse_action;
}

static void test_middle_drag(void) {
    g_mouse_event_callback = test_mouse_callback;
    const CGEventType types[] = {kCGEventLeftMouseDragged, kCGEventRightMouseDragged, kCGEventMouseMoved};
    for (size_t index = 0; index < sizeof(types) / sizeof(types[0]); index += 1) {
        CGEventRef event = CGEventCreateMouseEvent(NULL, types[index], CGPointMake(120, 230), kCGMouseButtonLeft);
        assert(event != NULL);
        CGEventSetIntegerValueField(event, kCGMouseEventDeltaX, 7);
        CGEventSetIntegerValueField(event, kCGMouseEventDeltaY, -4);
        CGEventSetFlags(event, kCGEventFlagMaskCommand);
        mouse_action = NMCMouseActionPass;
        assert(NMCMouseTapCallback(NULL, types[index], event, NULL) == event);
        assert(CGEventGetType(event) == types[index]);
        mouse_action = NMCMouseActionRewriteDrag;
        assert(NMCMouseTapCallback(NULL, types[index], event, NULL) == event);
        assert(CGEventGetType(event) == kCGEventOtherMouseDragged);
        assert(CGEventGetIntegerValueField(event, kCGMouseEventButtonNumber) == kCGMouseButtonCenter);
        assert(CGEventGetLocation(event).x == 120 && CGEventGetLocation(event).y == 230);
        assert(CGEventGetIntegerValueField(event, kCGMouseEventDeltaX) == 7);
        assert(CGEventGetIntegerValueField(event, kCGMouseEventDeltaY) == -4);
        assert(CGEventGetFlags(event) == kCGEventFlagMaskCommand);
        mouse_action = NMCMouseActionSuppress;
        assert(NMCMouseTapCallback(NULL, types[index], event, NULL) == NULL);
        CFRelease(event);
    }
    g_mouse_event_callback = NULL;

    nmc_post_middle_mouse_event(NMCMouseActionRewriteDrag, 30, -20);
    assert(posted_count == 0); // No drag before a button press.
    nmc_post_middle_mouse_event(NMCMouseActionRewriteDown, 0, 0);
    assert(g_middle_mouse_down && posted_count == 1 && cursor_move_count == 0);
    fail_cursor_move = true;
    nmc_post_middle_mouse_event(NMCMouseActionRewriteDrag, 30, -20);
    assert(posted_count == 1 && cursor.x == 300 && cursor.y == 400);
    fail_cursor_move = false;
    nmc_post_middle_mouse_event(NMCMouseActionRewriteDrag, 30, -20);
    assert(g_middle_mouse_down && cursor.x == 330 && cursor.y == 380);
    nmc_post_middle_mouse_event(NMCMouseActionRewriteDrag, 0.25, -0.5);
    nmc_post_middle_mouse_event(NMCMouseActionRewriteDrag, 0.25, -0.5);
    nmc_post_middle_mouse_event(NMCMouseActionRewriteDrag, 0.25, -0.5);
    nmc_post_middle_mouse_event(NMCMouseActionRewriteDrag, NAN, 0);
    nmc_post_middle_mouse_event(NMCMouseActionRewriteUp, 0, 0);
    nmc_post_middle_mouse_event(NMCMouseActionRewriteUp, 0, 0);
    assert(posted_count == 6);
    assert(posted[0].type == kCGEventOtherMouseDown);
    assert(posted[0].location.x == 300 && posted[0].location.y == 400);
    assert(posted[1].type == kCGEventOtherMouseDragged);
    assert(posted[1].location.x == 330 && posted[1].location.y == 380);
    assert(posted[1].delta_x == 30 && posted[1].delta_y == -20);
    assert(posted[2].type == kCGEventOtherMouseDragged);
    assert(posted[2].location.x == 330.25 && posted[2].location.y == 379.5);
    assert(posted[2].delta_x + posted[3].delta_x + posted[4].delta_x == 1);
    assert(posted[2].delta_y + posted[3].delta_y + posted[4].delta_y == -1);
    assert(posted[5].type == kCGEventOtherMouseUp);
    assert(posted[5].location.x == 330.75 && posted[5].location.y == 378.5);
    for (size_t index = 0; index < posted_count; index += 1) {
        assert(posted[index].button == kCGMouseButtonCenter);
        assert(posted[index].flags == kCGEventFlagMaskShift);
    }
    assert(!g_middle_mouse_down);
    assert(cursor_move_count == 4 && cursor.x == 330.75 && cursor.y == 378.5);

    nmc_post_middle_mouse_event(NMCMouseActionRewriteDown, 0, 0);
    nmc_stop();
    assert(posted_count == 8 && posted[7].type == kCGEventOtherMouseUp);
    assert(!g_middle_mouse_down); // Shutdown must release a held button.
    nmc_post_middle_mouse_event(NMCMouseActionRewriteDown, 0, 0);
    nmc_post_middle_mouse_event(NMCMouseActionRewriteUp, 0, 0);
    assert(posted_count == 10);
    assert(posted[8].type == kCGEventOtherMouseDown && posted[9].type == kCGEventOtherMouseUp);
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

    test_middle_drag();
    CFRelease(device);
    return 0;
}
