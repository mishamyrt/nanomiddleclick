use std::ffi::{CStr, c_char};

#[repr(C)]
pub(crate) struct RawStringArray {
    pub values: *mut *mut c_char,
    pub len: usize,
}

#[link(name = "nanomiddleclick_preferences_shim", kind = "static")]
unsafe extern "C" {
    fn nmcp_get_system_tap_to_click() -> bool;
    fn nmcp_get_bool(
        domain: *const c_char,
        key: *const c_char,
        default_value: bool,
    ) -> bool;
    fn nmcp_get_i64(
        domain: *const c_char,
        key: *const c_char,
        default_value: i64,
    ) -> i64;
    fn nmcp_get_f64(
        domain: *const c_char,
        key: *const c_char,
        default_value: f64,
    ) -> f64;
    fn nmcp_copy_string(domain: *const c_char, key: *const c_char) -> *mut c_char;
    fn nmcp_copy_string_array(
        domain: *const c_char,
        key: *const c_char,
    ) -> RawStringArray;
    fn nmcp_free_string(value: *mut c_char);
    fn nmcp_free_string_array(array: *mut RawStringArray);
}

pub(crate) fn system_tap_to_click() -> bool {
    unsafe { nmcp_get_system_tap_to_click() }
}

pub(crate) fn get_bool(domain: &CStr, key: &CStr, default_value: bool) -> bool {
    unsafe { nmcp_get_bool(domain.as_ptr(), key.as_ptr(), default_value) }
}

pub(crate) fn get_i64(domain: &CStr, key: &CStr, default_value: i64) -> i64 {
    unsafe { nmcp_get_i64(domain.as_ptr(), key.as_ptr(), default_value) }
}

pub(crate) fn get_f64(domain: &CStr, key: &CStr, default_value: f64) -> f64 {
    unsafe { nmcp_get_f64(domain.as_ptr(), key.as_ptr(), default_value) }
}

pub(crate) fn copy_string(domain: &CStr, key: &CStr) -> *mut c_char {
    unsafe { nmcp_copy_string(domain.as_ptr(), key.as_ptr()) }
}

pub(crate) fn copy_string_array(domain: &CStr, key: &CStr) -> RawStringArray {
    unsafe { nmcp_copy_string_array(domain.as_ptr(), key.as_ptr()) }
}

pub(crate) fn free_string(value: *mut c_char) {
    unsafe { nmcp_free_string(value) }
}

pub(crate) fn free_string_array(array: *mut RawStringArray) {
    unsafe { nmcp_free_string_array(array) }
}
