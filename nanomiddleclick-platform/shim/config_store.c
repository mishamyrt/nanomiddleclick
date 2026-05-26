#include <ApplicationServices/ApplicationServices.h>
#include <CoreFoundation/CoreFoundation.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <strings.h>

#include "nanomiddleclick_shim.h"

static CFStringRef const NMCDefaultsDomain = CFSTR("co.myrt.nanomiddleclick");

static bool NMCGetSystemTapToClick(void);
static CFTypeRef NMCReadPreference(CFStringRef key);
static bool NMCParseBoolString(const char *value, bool *out_value);
static bool NMCGetBoolValue(CFTypeRef value, bool default_value);
static int64_t NMCGetInt64Value(CFTypeRef value, int64_t default_value);
static double NMCGetDoubleValue(CFTypeRef value, double default_value);
static uint32_t NMCParseMouseClickMode(CFTypeRef raw_value);
static char *NMCCopyUTF8String(CFStringRef value);
static void NMCLoadIgnoredBundles(CFTypeRef raw_value, NMCConfigSnapshot *out_snapshot);

bool nmc_is_accessibility_trusted(bool prompt) {
    const void *keys[] = { kAXTrustedCheckOptionPrompt };
    const void *values[] = { prompt ? kCFBooleanTrue : kCFBooleanFalse };
    CFDictionaryRef options = CFDictionaryCreate(
        kCFAllocatorDefault,
        keys,
        values,
        1,
        &kCFTypeDictionaryKeyCallBacks,
        &kCFTypeDictionaryValueCallBacks
    );
    if (options == NULL) {
        return AXIsProcessTrustedWithOptions(NULL);
    }

    const bool trusted = AXIsProcessTrustedWithOptions(options);
    CFRelease(options);
    return trusted;
}

bool nmc_get_system_tap_to_click(void) {
    return NMCGetSystemTapToClick();
}

bool nmc_load_config(NMCConfigSnapshot *out_snapshot) {
    if (out_snapshot == NULL) {
        return false;
    }

    memset(out_snapshot, 0, sizeof(*out_snapshot));
    CFPreferencesAppSynchronize(NMCDefaultsDomain);

    CFTypeRef fingers = NMCReadPreference(CFSTR("fingers"));
    CFTypeRef allow_more_fingers = NMCReadPreference(CFSTR("allowMoreFingers"));
    CFTypeRef max_distance_delta = NMCReadPreference(CFSTR("maxDistanceDelta"));
    CFTypeRef max_time_delta = NMCReadPreference(CFSTR("maxTimeDelta"));
    CFTypeRef tap_to_click = NMCReadPreference(CFSTR("tapToClick"));
    CFTypeRef mouse_click_mode = NMCReadPreference(CFSTR("mouseClickMode"));
    CFTypeRef ignored = NMCReadPreference(CFSTR("ignoredAppBundles"));

    out_snapshot->fingers = NMCGetInt64Value(fingers, 3);
    out_snapshot->allow_more_fingers = NMCGetBoolValue(allow_more_fingers, false);
    out_snapshot->max_distance_delta = NMCGetDoubleValue(max_distance_delta, 0.05);
    out_snapshot->max_time_delta_ms = NMCGetInt64Value(max_time_delta, 300);
    out_snapshot->tap_to_click = tap_to_click != NULL
        ? NMCGetBoolValue(tap_to_click, NMCGetSystemTapToClick())
        : NMCGetSystemTapToClick();
    out_snapshot->mouse_click_mode = NMCParseMouseClickMode(mouse_click_mode);
    NMCLoadIgnoredBundles(ignored, out_snapshot);

    if (fingers != NULL) {
        CFRelease(fingers);
    }
    if (allow_more_fingers != NULL) {
        CFRelease(allow_more_fingers);
    }
    if (max_distance_delta != NULL) {
        CFRelease(max_distance_delta);
    }
    if (max_time_delta != NULL) {
        CFRelease(max_time_delta);
    }
    if (tap_to_click != NULL) {
        CFRelease(tap_to_click);
    }
    if (mouse_click_mode != NULL) {
        CFRelease(mouse_click_mode);
    }
    if (ignored != NULL) {
        CFRelease(ignored);
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

static bool NMCGetSystemTapToClick(void) {
    Boolean found = false;
    Boolean value = CFPreferencesGetAppBooleanValue(
        CFSTR("Clicking"),
        CFSTR("com.apple.driver.AppleBluetoothMultitouch.trackpad"),
        &found
    );
    return found ? value : false;
}

static CFTypeRef NMCReadPreference(CFStringRef key) {
    return CFPreferencesCopyAppValue(key, NMCDefaultsDomain);
}

static bool NMCParseBoolString(const char *value, bool *out_value) {
    char *end = NULL;
    long long numeric = strtoll(value, &end, 10);
    if (end != value && *end == '\0') {
        *out_value = numeric != 0;
        return true;
    }

    if (strcasecmp(value, "true") == 0 || strcasecmp(value, "yes") == 0) {
        *out_value = true;
        return true;
    }
    if (strcasecmp(value, "false") == 0 || strcasecmp(value, "no") == 0) {
        *out_value = false;
        return true;
    }

    return false;
}

static bool NMCGetBoolValue(CFTypeRef value, bool default_value) {
    if (value == NULL) {
        return default_value;
    }

    CFTypeID type = CFGetTypeID(value);
    if (type == CFBooleanGetTypeID()) {
        return CFBooleanGetValue((CFBooleanRef)value);
    }

    if (type == CFNumberGetTypeID()) {
        int64_t numeric = 0;
        if (CFNumberGetValue((CFNumberRef)value, kCFNumberSInt64Type, &numeric)) {
            return numeric != 0;
        }
        return default_value;
    }

    if (type == CFStringGetTypeID()) {
        char *buffer = NMCCopyUTF8String((CFStringRef)value);
        if (buffer == NULL) {
            return default_value;
        }

        bool parsed = false;
        bool result = default_value;
        if (NMCParseBoolString(buffer, &result)) {
            parsed = true;
        }
        free(buffer);
        return parsed ? result : default_value;
    }

    return default_value;
}

static int64_t NMCGetInt64Value(CFTypeRef value, int64_t default_value) {
    if (value == NULL) {
        return default_value;
    }

    CFTypeID type = CFGetTypeID(value);
    if (type == CFNumberGetTypeID()) {
        int64_t numeric = 0;
        if (CFNumberGetValue((CFNumberRef)value, kCFNumberSInt64Type, &numeric)) {
            return numeric;
        }
        return default_value;
    }

    if (type == CFStringGetTypeID()) {
        char *buffer = NMCCopyUTF8String((CFStringRef)value);
        if (buffer == NULL) {
            return default_value;
        }

        char *end = NULL;
        long long numeric = strtoll(buffer, &end, 10);
        const bool parsed = end != buffer && *end == '\0';
        free(buffer);
        if (parsed) {
            return (int64_t)numeric;
        }
    }

    return default_value;
}

static double NMCGetDoubleValue(CFTypeRef value, double default_value) {
    if (value == NULL) {
        return default_value;
    }

    CFTypeID type = CFGetTypeID(value);
    if (type == CFNumberGetTypeID()) {
        double numeric = 0.0;
        if (CFNumberGetValue((CFNumberRef)value, kCFNumberDoubleType, &numeric)) {
            return numeric;
        }
        return default_value;
    }

    if (type == CFStringGetTypeID()) {
        char *buffer = NMCCopyUTF8String((CFStringRef)value);
        if (buffer == NULL) {
            return default_value;
        }

        char *end = NULL;
        double numeric = strtod(buffer, &end);
        const bool parsed = end != buffer && *end == '\0';
        free(buffer);
        if (parsed) {
            return numeric;
        }
    }

    return default_value;
}

static uint32_t NMCParseMouseClickMode(CFTypeRef raw_value) {
    if (raw_value == NULL) {
        return 1;
    }

    CFTypeID type = CFGetTypeID(raw_value);
    if (type == CFNumberGetTypeID()) {
        int64_t numeric = 0;
        if (CFNumberGetValue((CFNumberRef)raw_value, kCFNumberSInt64Type, &numeric)) {
            switch (numeric) {
                case 0:
                case 1:
                case 2:
                    return (uint32_t)numeric;
                default:
                    return 1;
            }
        }
        return 1;
    }

    if (type != CFStringGetTypeID()) {
        return 1;
    }

    CFStringRef value = (CFStringRef)raw_value;
    if (CFStringCompare(value, CFSTR("center"), kCFCompareCaseInsensitive) == kCFCompareEqualTo) {
        return 1;
    }
    if (CFStringCompare(value, CFSTR("disabled"), kCFCompareCaseInsensitive) == kCFCompareEqualTo) {
        return 2;
    }
    if (CFStringCompare(value, CFSTR("threefinger"), kCFCompareCaseInsensitive) == kCFCompareEqualTo) {
        return 0;
    }

    return 1;
}

static char *NMCCopyUTF8String(CFStringRef value) {
    if (value == NULL) {
        return NULL;
    }

    CFIndex length = CFStringGetLength(value);
    CFIndex capacity = CFStringGetMaximumSizeForEncoding(length, kCFStringEncodingUTF8) + 1;
    char *buffer = calloc((size_t)capacity, sizeof(char));
    if (buffer == NULL) {
        return NULL;
    }

    if (!CFStringGetCString(value, buffer, capacity, kCFStringEncodingUTF8)) {
        free(buffer);
        return NULL;
    }

    return buffer;
}

static void NMCLoadIgnoredBundles(CFTypeRef raw_value, NMCConfigSnapshot *out_snapshot) {
    if (raw_value == NULL) {
        return;
    }

    const void **set_values = NULL;
    CFTypeID type = CFGetTypeID(raw_value);
    CFIndex count = 0;

    if (type == CFArrayGetTypeID()) {
        count = CFArrayGetCount((CFArrayRef)raw_value);
    } else if (type == CFSetGetTypeID()) {
        count = CFSetGetCount((CFSetRef)raw_value);
        if (count > 0) {
            set_values = calloc((size_t)count, sizeof(*set_values));
            if (set_values == NULL) {
                return;
            }
            CFSetGetValues((CFSetRef)raw_value, set_values);
        }
    } else {
        return;
    }

    if (count <= 0) {
        free(set_values);
        return;
    }

    out_snapshot->ignored_app_bundles_len = (uintptr_t)count;
    out_snapshot->ignored_app_bundles = calloc((size_t)count, sizeof(char *));
    if (out_snapshot->ignored_app_bundles == NULL) {
        out_snapshot->ignored_app_bundles_len = 0;
        free(set_values);
        return;
    }

    for (CFIndex index = 0; index < count; index += 1) {
        CFTypeRef entry = type == CFArrayGetTypeID()
            ? CFArrayGetValueAtIndex((CFArrayRef)raw_value, index)
            : set_values[index];
        if (entry == NULL || CFGetTypeID(entry) != CFStringGetTypeID()) {
            continue;
        }

        out_snapshot->ignored_app_bundles[index] = NMCCopyUTF8String((CFStringRef)entry);
    }

    free(set_values);
}
