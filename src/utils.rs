use sysinfo::{System, SystemExt};

pub(crate) struct Utils {}

impl Utils {
    /// Retrieve the current process PID
    pub(crate) fn get_pid() -> u32 {
        std::process::id()
    }

    pub(crate) fn get_os_version() -> String {
        System::new().long_os_version().unwrap_or_else(|| "Unknown".to_string())
    }
}


#[cfg(windows)]
impl Utils {
    /// Retrieve the system locale or return en_US as default value
    pub(crate) fn get_locale() -> String {
        use std::os::windows::ffi::OsStringExt;

        let mut locale_name = [0u16; winapi::shared::ntdef::LOCALE_NAME_MAX_LENGTH];

        let locale = match unsafe {
            winapi::um::winnls::GetSystemDefaultLocaleName(
                locale_name.as_mut_ptr(),
                locale_name.len() as _,
            )
        } {
            0 => std::ffi::OsString::from("en_US"),
            n => std::ffi::OsString::from_wide(&locale_name[..(n - 1) as usize]),
        };

        locale.into_string().unwrap_or("en_US".to_string())
    }

    pub fn get_model() -> String {
        use std::ffi::CString;

        let mut model_name = [0u8; 1024];
        let mut len = model_name.len() as _;

        let sub_key = CString::new("SYSTEM\\HardwareConfig\\Current").unwrap();
        let value = CString::new("SystemProductName").unwrap();

        match unsafe {
            winapi::um::winreg::RegGetValueA(
                winapi::um::winreg::HKEY_LOCAL_MACHINE,
                sub_key.as_ptr(),
                value.as_ptr(),
                winapi::um::winreg::RRF_RT_REG_SZ,
                std::ptr::null_mut(),
                model_name.as_mut_ptr() as _,
                &mut len,
            )
        } {
            0 if len > 1 => String::from_utf8_lossy(&model_name[..(len - 1) as usize]).to_string(),
            _ => "<Unknown>".to_string(),
        }
    }
}

#[cfg(target_os = "linux")]
impl Utils {
    pub(crate) fn get_locale() -> String {
        "en_US".to_string()
    }

    pub fn get_model() -> String {
        "Computer".to_string()
    }
}

#[cfg(target_os = "macos")]
impl Utils {
    pub(crate) fn get_locale() -> String {
        "en_US".to_string()
    }

    pub fn get_model() -> String {
        let mut model_name = [0u8; 1024];
        let mut len = model_name.len() as _;

        match unsafe {
            libc::sysctlbyname(
                b"hw.model\0".as_ptr() as _,
                model_name.as_mut_ptr() as _,
                &mut len,
                std::ptr::null_mut(),
                0,
            )
        } {
            0 if len > 1 => String::from_utf8_lossy(&model_name[..(len - 1) as usize]).to_string(),
            _ => "<Unknown>".to_string(),
        }
    }
}
