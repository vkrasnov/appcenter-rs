use super::Utils;
use serde::Serialize;

#[cfg(windows)]
const OS_NAME: &str = "Windows";
#[cfg(target_os = "macos")]
const OS_NAME: &str = "macOS";
#[cfg(target_os = "linux")]
const OS_NAME: &str = "Linux";

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Device {
    model: String,
    sdk_name: &'static str,
    sdk_version: &'static str,
    os_name: &'static str,
    os_version: String,
    locale: String,
    app_version: String,
    app_build: String,
}

impl Device {
    pub(crate) fn current_device(app_version: &str, app_build: &Option<String>) -> Self {
        Device {
            model: Utils::get_model(),
            sdk_name: "appcenter.custom",
            sdk_version: "3.2.2",
            os_name: OS_NAME,
            os_version: Utils::get_os_version(),
            locale: Utils::get_locale(),
            app_version: app_version.to_string(),
            app_build: app_build.clone().unwrap_or(String::new()),
        }
    }
}
