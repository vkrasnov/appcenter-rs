mod device;
mod utils;

use backtrace::Backtrace;
use device::Device;
use serde::Serialize;
use std::panic::{self, PanicInfo};
use std::sync::{Arc, Mutex};
pub(crate) use utils::Utils;

///
/// Install the custom panic hook that will attempt to upload panic stacktraces to
/// appcenter using the provided app secret. `CARGO_PKG_VERSION` will be used as the application version.
/// After the report is sent, the original panic hook is executed.
///
#[macro_export]
macro_rules! start {
    ($app_secret:expr) => {
        app_center::AppCenter::start($app_secret, env!("CARGO_PKG_VERSION"))
    };
}

#[allow(dead_code)]
pub struct AppCenter {
    inner: Arc<AppCenterInner>,
}

impl AppCenter {
    ///
    /// Install an optonal callback to be executed just before the report is sent
    /// usually this is the place to add any atachements to the report using
    /// `add_binary_attachement` or `add_text_attachement`
    ///
    pub fn set_report_callback<T>(&self, callback: T)
    where
        T: Fn(&mut AppCenterLogs) + Send + Sync + 'static,
    {
        *self.inner.on_report.lock().unwrap() = Some(Box::new(callback));
    }

    ///
    /// Associate the report with a specific user ID
    ///
    pub fn set_user_id<S: Into<String>>(&self, id: Option<S>) {
        *self.inner.user_id.lock().unwrap() = id.map(|s| s.into());
    }

    ///
    /// Install the custom panic hook that will attempt to upload panic stacktraces to
    /// appcenter using the provided app secret and application version.
    /// After the report is sent, the original panic hook is executed.
    ///
    pub fn start<S: Into<String>>(app_secret: S, app_version: &'static str) -> Self {
        let inner = Arc::new(AppCenterInner {
            app_secret: app_secret.into(),
            app_version,
            app_build: None,
            app_launch_timestamp: chrono::Utc::now(),
            user_id: Mutex::new(None),
            on_report: Mutex::new(None),
        });

        inner.set_panic_hook();

        AppCenter { inner }
    }
}

// The implementation is pretty straigtforward and follows the documentation in https://docs.microsoft.com/en-us/appcenter/diagnostics/upload-crashes
struct AppCenterInner {
    app_secret: String,
    app_version: &'static str,
    app_build: Option<String>,
    app_launch_timestamp: chrono::DateTime<chrono::Utc>,
    user_id: Mutex<Option<String>>,
    on_report: Mutex<Option<Box<dyn Fn(&mut AppCenterLogs) + Send + Sync>>>,
}

#[derive(Serialize)]
pub struct AppCenterLogs<'a> {
    logs: Vec<AppCenterLog<'a>>,
}

#[derive(Serialize)]
#[serde(tag = "type")]
#[serde(rename_all = "camelCase")]
enum AppCenterLog<'a> {
    #[serde(rename_all = "camelCase")]
    ManagedError {
        id: uuid::Uuid,
        user_id: Option<String>,
        app_launch_timestamp: chrono::DateTime<chrono::Utc>,
        timestamp: chrono::DateTime<chrono::Utc>,
        fatal: bool,
        process_id: u32,
        process_name: String,
        device: Device,
        exception: AppCenterException,
    },
    #[serde(rename_all = "camelCase")]
    ErrorAttachment {
        id: uuid::Uuid,
        error_id: uuid::Uuid,
        device: Device,
        content_type: &'static str,
        #[serde(serialize_with = "as_base64")]
        data: Vec<u8>,
        #[serde(skip_serializing_if = "Option::is_none")]
        file_name: Option<&'a str>,
    },
}

fn as_base64<S>(val: &Vec<u8>, s: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    s.serialize_str(&base64::encode(val))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AppCenterException {
    r#type: &'static str,
    message: String,
    frames: Vec<ExceptionFrame>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ExceptionFrame {
    #[serde(skip_serializing_if = "Option::is_none")]
    method_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    line_number: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    file_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    address: Option<String>,
}

impl ExceptionFrame {
    fn collect_backtrace() -> Vec<ExceptionFrame> {
        let mut frames = Vec::new();

        // First step is to collect the backtrace
        let current_backtrace = Backtrace::new();

        // We skip the frames until we hit the one that means something
        for frame in current_backtrace.frames().into_iter() {
            for symbol in frame.symbols() {
                frames.push(ExceptionFrame {
                    method_name: symbol.name().map(|n| format!("{}", n)),

                    line_number: symbol.lineno(),

                    file_name: symbol
                        .filename()
                        .and_then(|n| n.to_str())
                        .map(|s| s.to_string()),

                    address: None,
                });
            }
        }

        frames
    }
}

impl AppCenterException {
    fn new(panic_info: &PanicInfo) -> Self {
        let mut message = String::new();
        if let Some(payload) = panic_info.payload().downcast_ref::<&str>() {
            message.push_str(payload);
        }

        if let Some(location) = panic_info.location() {
            message.push_str(&format!(" at {}:{}", location.file(), location.line()));
        }

        AppCenterException {
            r#type: "panic",
            message,
            frames: ExceptionFrame::collect_backtrace(),
        }
    }
}

impl<'a> AppCenterLogs<'a> {
    fn add_attachement_inner(
        &mut self,
        data: Vec<u8>,
        file_name: Option<&'a str>,
        content_type: &'static str,
    ) {
        // First attachement is always the ManagedError kind
        let (device, error_id) = match &self.logs[0] {
            AppCenterLog::ManagedError { device, id, .. } => (device.clone(), id.clone()),
            _ => unreachable!(),
        };

        self.logs.push(AppCenterLog::ErrorAttachment {
            id: uuid::Uuid::new_v4(),
            device: device.clone(),
            error_id,
            content_type,
            data,
            file_name,
        });
    }

    pub fn add_binary_attachement(&mut self, data: Vec<u8>, file_name: Option<&'a str>) {
        self.add_attachement_inner(data, file_name, "application/octet_stream");
    }

    pub fn add_text_attachement(&'a mut self, data: &str, file_name: Option<&'a str>) {
        self.add_attachement_inner(data.as_bytes().to_vec(), file_name, "text/plain");
    }
}

impl AppCenterInner {
    fn new_payload(&self, panic_info: &PanicInfo) -> AppCenterLogs {
        let user_id = { (*self.user_id.lock().unwrap()).clone() };

        AppCenterLogs {
            logs: vec![AppCenterLog::ManagedError {
                id: uuid::Uuid::new_v4(),
                user_id,
                app_launch_timestamp: self.app_launch_timestamp,
                timestamp: chrono::Utc::now(),
                fatal: true,
                process_id: Utils::get_pid(),
                process_name: "".to_string(),
                device: Device::current_device(self.app_version, &self.app_build),
                exception: AppCenterException::new(panic_info),
            }],
        }
    }

    fn set_panic_hook(self: &Arc<Self>) {
        let app_center = Arc::clone(self);

        let old_hook = panic::take_hook();

        panic::set_hook(Box::new(move |panic_info| {
            let mut payload = app_center.new_payload(panic_info);

            let report_callback = { app_center.on_report.lock().unwrap().take() };

            if let Some(report_callback) = report_callback {
                report_callback(&mut payload)
            }

            if let Ok(client) = reqwest::blocking::Client::builder()
                .connect_timeout(std::time::Duration::from_secs(4))
                .build()
            {
                let send_payload = client
                    .post("https://in.appcenter.ms/logs?Api-Version=1.0.0")
                    .header("Content-Type", "application/json")
                    .header("app-secret", &app_center.app_secret)
                    .header("install-id", "00000000-0000-0000-0000-000000000001")
                    .body(serde_json::to_vec(&payload).unwrap())
                    .send();

                match send_payload {
                    Ok(resp) => log::info!("Crash report sent: {:?}", resp.text()),
                    // TODO: We failed to send the crash report, save it to disk to be sent later
                    Err(err) => log::error!("Failed to send crash report {:?}", err),
                }
            }

            // Execute the original panic handler
            old_hook(panic_info)
        }));
    }
}
