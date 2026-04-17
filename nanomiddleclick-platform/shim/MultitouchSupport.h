#ifndef __MULTITOUCH_SUPPORT_H__
#define __MULTITOUCH_SUPPORT_H__

#include <CoreFoundation/CoreFoundation.h>
#include <IOKit/IOKitLib.h>

typedef struct {
    int frame;
    double timestamp;
    int identifier;
    int state;
    int fingerID;
    int handID;
    struct {
        float x;
        float y;
    } normalizedPosition;
    struct {
        float x;
        float y;
    } normalizedVelocity;
    float total;
    float pressure;
    float angle;
    float majorAxis;
    float minorAxis;
    struct {
        float x;
        float y;
    } absolutePosition;
    struct {
        float x;
        float y;
    } absoluteVelocity;
    int unknown1;
    int unknown2;
    float density;
} MTTouch;

typedef void *MTDeviceRef;

extern CFMutableArrayRef MTDeviceCreateList(void);
extern int MTDeviceStart(MTDeviceRef device, int unknown);
extern int MTDeviceStop(MTDeviceRef device);
extern io_service_t MTDeviceGetService(MTDeviceRef device);
extern int MTRegisterContactFrameCallback(
    MTDeviceRef device,
    void (*callback)(MTDeviceRef, MTTouch *, int, double, int)
);
extern int MTUnregisterContactFrameCallback(
    MTDeviceRef device,
    void (*callback)(MTDeviceRef, MTTouch *, int, double, int)
);
extern void MTDeviceRelease(MTDeviceRef device);

#endif
