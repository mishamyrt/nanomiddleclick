mod raw;

use std::ffi::{CStr, CString, NulError, c_char};

pub struct Preferences {
    domain: CString,
}

impl Preferences {
    pub fn new(domain: &str) -> Result<Self, NulError> {
        Ok(Self { domain: CString::new(domain)? })
    }

    pub fn get_bool(
        &self,
        key: &str,
        default_value: bool,
    ) -> Result<bool, NulError> {
        let key = CString::new(key)?;
        Ok(raw::get_bool(&self.domain, &key, default_value))
    }

    pub fn get_i64(&self, key: &str, default_value: i64) -> Result<i64, NulError> {
        let key = CString::new(key)?;
        Ok(raw::get_i64(&self.domain, &key, default_value))
    }

    pub fn get_f64(&self, key: &str, default_value: f64) -> Result<f64, NulError> {
        let key = CString::new(key)?;
        Ok(raw::get_f64(&self.domain, &key, default_value))
    }

    pub fn get_string(&self, key: &str) -> Result<Option<String>, NulError> {
        let key = CString::new(key)?;
        let value = raw::copy_string(&self.domain, &key);
        if value.is_null() {
            return Ok(None);
        }

        let _guard = RawStringGuard(value);
        Ok(Some(unsafe { CStr::from_ptr(value) }.to_string_lossy().into_owned()))
    }

    pub fn get_string_array(&self, key: &str) -> Result<Box<[Box<str>]>, NulError> {
        let key = CString::new(key)?;
        let mut raw_array = raw::copy_string_array(&self.domain, &key);
        let _guard = RawStringArrayGuard(std::ptr::addr_of_mut!(raw_array));

        let mut values = Vec::with_capacity(raw_array.len);
        if raw_array.values.is_null() || raw_array.len == 0 {
            return Ok(values.into_boxed_slice());
        }

        let raw_values =
            unsafe { std::slice::from_raw_parts(raw_array.values, raw_array.len) };
        for value in raw_values {
            if value.is_null() {
                continue;
            }

            values.push(
                unsafe { CStr::from_ptr(*value) }
                    .to_string_lossy()
                    .into_owned()
                    .into_boxed_str(),
            );
        }

        Ok(values.into_boxed_slice())
    }
}

pub fn system_tap_to_click() -> bool {
    raw::system_tap_to_click()
}

struct RawStringGuard(*mut c_char);

impl Drop for RawStringGuard {
    fn drop(&mut self) {
        raw::free_string(self.0);
    }
}

struct RawStringArrayGuard(*mut raw::RawStringArray);

impl Drop for RawStringArrayGuard {
    fn drop(&mut self) {
        raw::free_string_array(self.0);
    }
}
