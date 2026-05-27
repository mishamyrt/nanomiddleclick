#include <CoreFoundation/CoreFoundation.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <strings.h>

#include "nanomiddleclick_preferences.h"

static CFTypeRef NMCPCopyPreference(const char *domain, const char *key);
static bool NMCPParseBoolString(const char *value, bool *out_value);
static bool NMCPGetBoolValue(CFTypeRef value, bool default_value);
static int64_t NMCPGetInt64Value(CFTypeRef value, int64_t default_value);
static double NMCPGetDoubleValue(CFTypeRef value, double default_value);
static char *NMCPCopyUTF8String(CFStringRef value);

bool nmcp_get_system_tap_to_click(void) {
    Boolean found = false;
    Boolean value = CFPreferencesGetAppBooleanValue(
        CFSTR("Clicking"),
        CFSTR("com.apple.driver.AppleBluetoothMultitouch.trackpad"),
        &found
    );
    return found ? value : false;
}

bool nmcp_get_bool(const char *domain, const char *key, bool default_value) {
    CFTypeRef value = NMCPCopyPreference(domain, key);
    bool result = NMCPGetBoolValue(value, default_value);
    if (value != NULL) {
        CFRelease(value);
    }
    return result;
}

int64_t nmcp_get_i64(const char *domain, const char *key, int64_t default_value) {
    CFTypeRef value = NMCPCopyPreference(domain, key);
    int64_t result = NMCPGetInt64Value(value, default_value);
    if (value != NULL) {
        CFRelease(value);
    }
    return result;
}

double nmcp_get_f64(const char *domain, const char *key, double default_value) {
    CFTypeRef value = NMCPCopyPreference(domain, key);
    double result = NMCPGetDoubleValue(value, default_value);
    if (value != NULL) {
        CFRelease(value);
    }
    return result;
}

char *nmcp_copy_string(const char *domain, const char *key) {
    CFTypeRef value = NMCPCopyPreference(domain, key);
    char *result = NULL;
    if (value != NULL && CFGetTypeID(value) == CFStringGetTypeID()) {
        result = NMCPCopyUTF8String((CFStringRef)value);
    }
    if (value != NULL) {
        CFRelease(value);
    }
    return result;
}

NMCPStringArray nmcp_copy_string_array(const char *domain, const char *key) {
    NMCPStringArray result = { .values = NULL, .len = 0 };
    CFTypeRef raw_value = NMCPCopyPreference(domain, key);
    if (raw_value == NULL) {
        return result;
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
                CFRelease(raw_value);
                return result;
            }
            CFSetGetValues((CFSetRef)raw_value, set_values);
        }
    } else {
        CFRelease(raw_value);
        return result;
    }

    if (count <= 0) {
        free(set_values);
        CFRelease(raw_value);
        return result;
    }

    result.len = (uintptr_t)count;
    result.values = calloc((size_t)count, sizeof(char *));
    if (result.values == NULL) {
        result.len = 0;
        free(set_values);
        CFRelease(raw_value);
        return result;
    }

    for (CFIndex index = 0; index < count; index += 1) {
        CFTypeRef entry = type == CFArrayGetTypeID()
            ? CFArrayGetValueAtIndex((CFArrayRef)raw_value, index)
            : set_values[index];
        if (entry == NULL || CFGetTypeID(entry) != CFStringGetTypeID()) {
            continue;
        }

        result.values[index] = NMCPCopyUTF8String((CFStringRef)entry);
    }

    free(set_values);
    CFRelease(raw_value);
    return result;
}

void nmcp_free_string(char *value) {
    free(value);
}

void nmcp_free_string_array(NMCPStringArray *array) {
    if (array == NULL || array->values == NULL) {
        return;
    }

    for (uintptr_t index = 0; index < array->len; index += 1) {
        free(array->values[index]);
    }
    free(array->values);
    memset(array, 0, sizeof(*array));
}

static CFTypeRef NMCPCopyPreference(const char *domain, const char *key) {
    if (domain == NULL || key == NULL) {
        return NULL;
    }

    CFStringRef domain_ref = CFStringCreateWithCString(kCFAllocatorDefault, domain, kCFStringEncodingUTF8);
    CFStringRef key_ref = CFStringCreateWithCString(kCFAllocatorDefault, key, kCFStringEncodingUTF8);
    if (domain_ref == NULL || key_ref == NULL) {
        if (domain_ref != NULL) {
            CFRelease(domain_ref);
        }
        if (key_ref != NULL) {
            CFRelease(key_ref);
        }
        return NULL;
    }

    CFPreferencesAppSynchronize(domain_ref);
    CFTypeRef value = CFPreferencesCopyAppValue(key_ref, domain_ref);
    CFRelease(key_ref);
    CFRelease(domain_ref);
    return value;
}

static bool NMCPParseBoolString(const char *value, bool *out_value) {
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

static bool NMCPGetBoolValue(CFTypeRef value, bool default_value) {
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
        char *buffer = NMCPCopyUTF8String((CFStringRef)value);
        if (buffer == NULL) {
            return default_value;
        }

        bool parsed = false;
        bool result = default_value;
        if (NMCPParseBoolString(buffer, &result)) {
            parsed = true;
        }
        free(buffer);
        return parsed ? result : default_value;
    }

    return default_value;
}

static int64_t NMCPGetInt64Value(CFTypeRef value, int64_t default_value) {
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
        char *buffer = NMCPCopyUTF8String((CFStringRef)value);
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

static double NMCPGetDoubleValue(CFTypeRef value, double default_value) {
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
        char *buffer = NMCPCopyUTF8String((CFStringRef)value);
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

static char *NMCPCopyUTF8String(CFStringRef value) {
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
