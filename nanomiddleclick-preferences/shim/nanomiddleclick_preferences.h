#ifndef NANOMIDDLECLICK_PREFERENCES_H
#define NANOMIDDLECLICK_PREFERENCES_H

#include <stdbool.h>
#include <stdint.h>

typedef struct {
    char **values;
    uintptr_t len;
} NMCPStringArray;

bool nmcp_get_system_tap_to_click(void);
bool nmcp_get_bool(const char *domain, const char *key, bool default_value);
int64_t nmcp_get_i64(const char *domain, const char *key, int64_t default_value);
double nmcp_get_f64(const char *domain, const char *key, double default_value);
char *nmcp_copy_string(const char *domain, const char *key);
NMCPStringArray nmcp_copy_string_array(const char *domain, const char *key);
void nmcp_free_string(char *value);
void nmcp_free_string_array(NMCPStringArray *array);

#endif
