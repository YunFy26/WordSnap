use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::{mpsc, Mutex, OnceLock},
    thread,
    time::{Duration, Instant},
};

use anyhow::{anyhow, Context, Result};
#[cfg(not(target_os = "macos"))]
use arboard::Clipboard;
use chrono::{DateTime, Local, Utc};
use enigo::{
    Direction::{Click, Press, Release},
    Enigo, Key, Keyboard, Mouse, Settings as EnigoSettings,
};
#[cfg(target_os = "macos")]
use objc2::rc::Retained as MacRetained;
#[cfg(target_os = "macos")]
use objc2::runtime::{AnyObject as MacAnyObject, ProtocolObject as MacProtocolObject};
#[cfg(target_os = "macos")]
use objc2_app_kit::{
    NSEvent as MacNSEvent, NSEventMask, NSPasteboard, NSPasteboardItem, NSPasteboardType,
    NSPasteboardTypeString, NSWindowCollectionBehavior, NSWindowStyleMask,
};
#[cfg(target_os = "macos")]
use objc2_foundation::{NSArray, NSData};
use reqwest::Client;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::image::Image;
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{
    AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, PhysicalPosition, State, WindowEvent,
};
#[cfg(target_os = "macos")]
use tauri_nspanel::{
    tauri_panel, CollectionBehavior, ManagerExt as _, PanelLevel, StyleMask, WebviewWindowExt as _,
};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

static APP_LOG: OnceLock<Mutex<AppLogger>> = OnceLock::new();

struct AppLogger {
    file: File,
}

#[derive(Clone, Copy)]
enum LogLevel {
    Debug,
    Info,
    Error,
}

impl LogLevel {
    fn as_str(self) -> &'static str {
        match self {
            Self::Debug => "DEBUG",
            Self::Info => "INFO",
            Self::Error => "ERROR",
        }
    }
}

impl AppLogger {
    fn new(directory: PathBuf) -> Result<Self> {
        let started_at = Local::now().format("%Y-%m-%d_%H-%M-%S%.3f").to_string();
        Self::new_with_identity(directory, &started_at, std::process::id())
    }

    fn new_with_identity(directory: PathBuf, started_at: &str, process_id: u32) -> Result<Self> {
        let path = directory.join(session_log_filename(started_at, process_id));
        let file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&path)
            .context("failed to create application session log")?;
        restrict_file_permissions(&path);
        Ok(Self { file })
    }

    fn write(&mut self, level: LogLevel, event: &str, details: &str) -> Result<()> {
        let timestamp = Local::now().to_rfc3339();
        writeln!(
            self.file,
            "{timestamp} {} {event} {}",
            level.as_str(),
            sanitize_log_field(details)
        )?;
        self.file.flush()?;
        Ok(())
    }
}

fn session_log_filename(started_at: &str, process_id: u32) -> String {
    format!("{started_at}_pid-{process_id}.log")
}

fn project_logs_dir() -> Result<PathBuf> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(|root| root.join("logs"))
        .context("failed to resolve project root directory")
}

fn init_logging() -> Result<()> {
    let log_dir = project_logs_dir()?;
    fs::create_dir_all(&log_dir).context("failed to create project logs directory")?;
    APP_LOG
        .set(Mutex::new(AppLogger::new(log_dir)?))
        .map_err(|_| anyhow!("application logging was initialized more than once"))?;
    log_event(LogLevel::Info, "app.logging_initialized", "file=session");
    Ok(())
}

fn restrict_file_permissions(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if fs::set_permissions(path, fs::Permissions::from_mode(0o600)).is_err() {
            if APP_LOG.get().is_some() {
                log_event(
                    LogLevel::Error,
                    "storage.file_permission_update_failed",
                    "result=failed",
                );
            } else {
                eprintln!("failed to restrict permissions for a WordSnap local file");
            }
        }
    }
    #[cfg(not(unix))]
    let _ = path;
}

fn log_event(level: LogLevel, event: &str, details: &str) {
    let Some(logger) = APP_LOG.get() else {
        eprintln!("{} {} {}", level.as_str(), event, details);
        return;
    };
    let Ok(mut file) = logger.lock() else {
        eprintln!("ERROR app.logging_lock_failed");
        return;
    };
    if file.write(level, event, details).is_err() {
        eprintln!("ERROR app.logging_write_failed");
    }
}

fn sanitize_log_field(value: &str) -> String {
    value
        .chars()
        .take(240)
        .map(|character| {
            if character.is_ascii_control() {
                ' '
            } else {
                character
            }
        })
        .collect()
}

fn log_ignored_error<T, E>(result: std::result::Result<T, E>, event: &str) -> Option<T> {
    match result {
        Ok(value) => Some(value),
        Err(_) => {
            log_event(LogLevel::Error, event, "result=failed");
            None
        }
    }
}

#[cfg(target_os = "macos")]
tauri_panel! {
    panel!(WordSnapFloatPanel {
        config: {
            can_become_key_window: true,
            can_become_main_window: false,
            is_floating_panel: true,
            becomes_key_only_if_needed: true,
            hides_on_deactivate: false,
            works_when_modal: true
        }
    })
}

#[cfg(target_os = "macos")]
thread_local! {
    // AppKit 持有事件监听器，直至监听器被显式移除。将返回的令牌保存在主线程中，
    // 可以使监听器的生命周期与应用生命周期一致。
    static MAC_FLOAT_CLICK_MONITORS: std::cell::RefCell<Vec<MacRetained<MacAnyObject>>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

struct AppState {
    conn: Mutex<Connection>,
    settings: Mutex<StoredSettings>,
    float: Mutex<FloatPayload>,
    /// 翻译浮窗的逻辑坐标锚点。触发翻译时只记录一次，使浮窗在加载、显示结果和重试
    /// 期间保持原位，不随鼠标的后续移动改变位置。
    anchor: Mutex<(i32, i32)>,
    client: Client,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
struct StoredSettings {
    base_url: String,
    model: String,
    hotkey: String,
    target_lang: String,
    // API Key 保存在应用自身的 settings.json 中，文件权限限制为仅所有者可读，
    // 不使用系统密钥库，因此保存密钥不会触发 macOS 权限提示。
    api_key: String,
}

impl Default for StoredSettings {
    fn default() -> Self {
        Self {
            base_url: "https://api.openai.com/v1".to_string(),
            model: "gpt-4o-mini".to_string(),
            hotkey: "Alt+T".to_string(),
            target_lang: "简体中文".to_string(),
            api_key: String::new(),
        }
    }
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SettingsPayload {
    base_url: String,
    model: String,
    hotkey: String,
    target_lang: String,
    api_key_set: bool,
    api_key_preview: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SaveSettingsRequest {
    base_url: String,
    model: String,
    hotkey: String,
    target_lang: String,
    api_key: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FloatPayload {
    state: String,
    original: String,
    translation: Option<String>,
    is_word: bool,
    count: Option<i64>,
    error: Option<String>,
}

impl Default for FloatPayload {
    fn default() -> Self {
        Self {
            state: "idle".to_string(),
            original: String::new(),
            translation: None,
            is_word: false,
            count: None,
            error: None,
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WordRecord {
    word: String,
    translation: String,
    count: i64,
    first_seen_at: String,
    last_seen_at: String,
    recent: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WordListPayload {
    total: i64,
    words: Vec<WordRecord>,
}

#[derive(Serialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    temperature: f32,
}

impl AppState {
    fn new(app: &AppHandle) -> Result<Self> {
        let app_dir = app_data_dir(app)?;
        fs::create_dir_all(&app_dir).context("failed to create app data directory")?;

        let settings = load_settings(&app_dir)?;
        log_event(
            LogLevel::Info,
            "settings.loaded",
            if settings.api_key.trim().is_empty() {
                "api_key_set=false"
            } else {
                "api_key_set=true"
            },
        );
        let conn = Connection::open(app_dir.join("wordsnap.sqlite3"))
            .context("failed to open WordSnap SQLite database")?;
        init_db(&conn)?;

        let client = match Client::builder()
            .connect_timeout(Duration::from_secs(8))
            .timeout(Duration::from_secs(45))
            .build()
        {
            Ok(client) => client,
            Err(_) => {
                log_event(
                    LogLevel::Error,
                    "network.client_configuration_failed",
                    "fallback=default_client",
                );
                Client::new()
            }
        };
        log_event(LogLevel::Info, "database.initialized", "result=success");

        Ok(Self {
            conn: Mutex::new(conn),
            settings: Mutex::new(settings),
            float: Mutex::new(FloatPayload::default()),
            anchor: Mutex::new((0, 0)),
            // Base URL 指向被阻止或错误的端点时，主机通常无法连接。连接超时用于尽快
            // 返回错误，避免界面长时间停留在加载状态。
            client,
        })
    }
}

#[tauri::command]
fn get_settings(state: State<'_, AppState>) -> Result<SettingsPayload, String> {
    log_event(LogLevel::Debug, "settings.read_started", "source=command");
    let settings = state
        .settings
        .lock()
        .map_err(|error| {
            log_event(LogLevel::Error, "settings.read_failed", "reason=state_lock");
            lock_err(error)
        })?
        .clone();
    Ok(settings_payload(settings))
}

#[tauri::command]
fn save_settings(
    app: AppHandle,
    state: State<'_, AppState>,
    request: SaveSettingsRequest,
) -> Result<SettingsPayload, String> {
    // 用户未输入新密钥时，保留先前保存的 API Key。
    let current = state
        .settings
        .lock()
        .map_err(|error| {
            log_event(LogLevel::Error, "settings.save_failed", "reason=state_lock");
            lock_err(error)
        })?
        .clone();
    let existing_key = current.api_key.clone();
    let mut next = StoredSettings {
        base_url: normalize_base_url(&request.base_url),
        model: request.model.trim().to_string(),
        hotkey: request.hotkey.trim().to_string(),
        target_lang: request.target_lang.trim().to_string(),
        api_key: existing_key,
    };

    if next.model.is_empty() {
        next.model = StoredSettings::default().model;
    }
    if next.hotkey.is_empty() {
        next.hotkey = StoredSettings::default().hotkey;
    }
    if next.target_lang.is_empty() {
        next.target_lang = StoredSettings::default().target_lang;
    }

    if let Some(api_key) = request.api_key {
        let trimmed = api_key.trim();
        if !trimmed.is_empty() {
            next.api_key = trimmed.to_string();
        }
    }

    let change_summary = format!(
        "base_url_changed={} model_changed={} target_lang_changed={} api_key_replaced={} transport={}",
        current.base_url != next.base_url,
        current.model != next.model,
        current.target_lang != next.target_lang,
        current.api_key != next.api_key,
        if next.base_url.starts_with("https://") {
            "https"
        } else {
            "http_or_other"
        }
    );
    let app_dir = app_data_dir(&app).map_err(|error| {
        log_event(
            LogLevel::Error,
            "settings.save_failed",
            "reason=app_data_path",
        );
        to_string(error)
    })?;
    save_settings_file(&app_dir, &next).map_err(|error| {
        log_event(LogLevel::Error, "settings.save_failed", "reason=file_write");
        to_string(error)
    })?;
    *state.settings.lock().map_err(|error| {
        log_event(
            LogLevel::Error,
            "settings.save_failed",
            "reason=state_lock_update",
        );
        lock_err(error)
    })? = next.clone();
    log_event(LogLevel::Info, "settings.saved", &change_summary);
    Ok(settings_payload(next))
}

#[tauri::command]
fn list_words(state: State<'_, AppState>) -> Result<WordListPayload, String> {
    log_event(LogLevel::Debug, "words.list_started", "source=command");
    read_words(&state)
        .inspect(|payload| {
            log_event(
                LogLevel::Debug,
                "words.list_completed",
                &format!("total={}", payload.total),
            );
        })
        .map_err(|error| {
            log_event(LogLevel::Error, "words.list_failed", "result=failed");
            to_string(error)
        })
}

#[tauri::command]
fn remove_word(
    app: AppHandle,
    state: State<'_, AppState>,
    word: String,
) -> Result<WordListPayload, String> {
    let normalized = word.trim().to_lowercase();
    if normalized.is_empty() {
        log_event(
            LogLevel::Error,
            "words.remove_rejected",
            "reason=empty_input",
        );
        return Err("单词不能为空。".to_string());
    }

    delete_word(&state, &normalized).map_err(|error| {
        log_event(LogLevel::Error, "words.remove_failed", "result=failed");
        to_string(error)
    })?;
    let payload = read_words(&state).map_err(|error| {
        log_event(LogLevel::Error, "words.list_failed", "after=remove");
        to_string(error)
    })?;
    log_ignored_error(app.emit("words-updated", ()), "words.update_event_failed");
    log_event(LogLevel::Info, "words.removed", "result=success");
    Ok(payload)
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum FrontendLogLevel {
    Debug,
    Info,
    Error,
}

#[tauri::command]
fn report_frontend_event(level: FrontendLogLevel, event: String) {
    let allowed = [
        "frontend.initialized",
        "frontend.command_completed",
        "frontend.command_failed",
        "frontend.event_listener_failed",
        "frontend.render_failed",
        "frontend.unhandled_error",
        "frontend.unhandled_rejection",
    ];
    if allowed.contains(&event.as_str()) {
        let level = match level {
            FrontendLogLevel::Debug => LogLevel::Debug,
            FrontendLogLevel::Info => LogLevel::Info,
            FrontendLogLevel::Error => LogLevel::Error,
        };
        log_event(level, &event, "source=webview");
    } else {
        log_event(
            LogLevel::Error,
            "frontend.log_event_rejected",
            "reason=unknown_event",
        );
    }
}

#[tauri::command]
fn current_float(state: State<'_, AppState>) -> Result<FloatPayload, String> {
    state
        .float
        .lock()
        .map_err(|error| {
            log_event(
                LogLevel::Error,
                "window.float_state_read_failed",
                "reason=state_lock",
            );
            lock_err(error)
        })
        .map(|payload| payload.clone())
}

#[tauri::command]
fn show_words(app: AppHandle) -> Result<(), String> {
    hide_menu_window(&app);
    show_window(&app, "words")
        .inspect(|_| {
            log_event(LogLevel::Info, "window.words_shown", "result=success");
        })
        .inspect_err(|_| {
            log_event(LogLevel::Error, "window.words_show_failed", "result=failed");
        })
}

#[tauri::command]
fn show_settings(app: AppHandle) -> Result<(), String> {
    hide_menu_window(&app);
    show_window(&app, "settings").inspect_err(|_| {
        log_event(
            LogLevel::Error,
            "window.settings_show_failed",
            "result=failed",
        );
    })?;
    // 设置 WebView 只创建一次并重复使用。每次打开时通知其重新加载字段，避免显示
    // 上一次关闭窗口时遗留的状态。
    if let Some(window) = app.get_webview_window("settings") {
        log_ignored_error(
            window.emit("settings-refresh", ()),
            "settings.refresh_event_failed",
        );
    }
    log_event(LogLevel::Info, "window.settings_shown", "result=success");
    Ok(())
}

#[tauri::command]
fn hide_menu(app: AppHandle) {
    log_event(
        LogLevel::Debug,
        "window.menu_hide_requested",
        "source=command",
    );
    hide_menu_window(&app);
}

#[tauri::command]
fn hide_settings(app: AppHandle) {
    log_event(
        LogLevel::Debug,
        "window.settings_hide_requested",
        "source=command",
    );
    if let Some(window) = app.get_webview_window("settings") {
        log_ignored_error(window.hide(), "window.settings_hide_failed");
    }
}

#[tauri::command]
fn hide_float(app: AppHandle) {
    log_event(
        LogLevel::Debug,
        "window.float_hide_requested",
        "source=command",
    );
    if let Some(window) = app.get_webview_window("float") {
        log_ignored_error(window.hide(), "window.float_hide_failed");
    }
}

#[tauri::command]
fn resize_float(app: AppHandle, width: u32, height: u32) -> Result<(), String> {
    log_event(
        LogLevel::Debug,
        "window.float_resize_requested",
        &format!("width={width} height={height}"),
    );
    let window = app
        .get_webview_window("float")
        .ok_or_else(|| "float window not found".to_string())?;
    let width = width.clamp(240, 400) as f64;
    let height = height.clamp(70, 420) as f64;
    window
        .set_size(LogicalSize::new(width, height))
        .map_err(to_string)?;

    // 卡片实际高度可能超过后端初始估计值。根据实际尺寸重新限制原点，避免卡片超出
    // 屏幕底部。
    let (anchor_x, anchor_y) = app
        .try_state::<AppState>()
        .and_then(|state| state.anchor.lock().ok().map(|anchor| *anchor))
        .unwrap_or((620, 260));
    let (x, y) = float_origin(&window, anchor_x as f64, anchor_y as f64, width, height);
    log_ignored_error(
        window.set_position(LogicalPosition::new(x, y)),
        "window.float_position_failed",
    );
    Ok(())
}

#[tauri::command]
fn quit_app(app: AppHandle) {
    log_event(LogLevel::Info, "app.quit_requested", "source=command");
    app.exit(0);
}

#[tauri::command]
async fn translate_text(
    app: AppHandle,
    state: State<'_, AppState>,
    text: String,
) -> Result<FloatPayload, String> {
    log_event(
        LogLevel::Info,
        "translation.manual_requested",
        &format!("text_chars={}", text.chars().count()),
    );
    let anchor = cursor_position_on_main(&app);
    if let Ok(mut stored) = state.anchor.lock() {
        *stored = anchor;
    }
    translate_selection(app, state.inner(), text)
        .await
        .map_err(to_string)
}

#[tauri::command]
async fn retry_translation(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<FloatPayload, String> {
    let original = state
        .float
        .lock()
        .map_err(lock_err)?
        .original
        .trim()
        .to_string();

    if original.is_empty() {
        log_event(
            LogLevel::Error,
            "translation.retry_rejected",
            "reason=no_text",
        );
        return Err("没有可重试的文本。".to_string());
    }

    log_event(
        LogLevel::Info,
        "translation.retry_requested",
        "result=started",
    );
    translate_selection(app, state.inner(), original)
        .await
        .map_err(to_string)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default();
    #[cfg(target_os = "macos")]
    let builder = builder.plugin(tauri_nspanel::init());

    let run_result = builder
        .setup(|app| {
            init_logging()?;
            log_event(
                LogLevel::Info,
                "app.starting",
                &format!(
                    "version={} platform={}",
                    env!("CARGO_PKG_VERSION"),
                    std::env::consts::OS
                ),
            );
            #[cfg(target_os = "macos")]
            {
                app.handle()
                    .set_activation_policy(tauri::ActivationPolicy::Accessory)?;
                app.handle().set_dock_visibility(false)?;
                log_event(LogLevel::Info, "app.macos_accessory_mode", "result=success");
            }

            let state = AppState::new(app.handle()).inspect_err(|_| {
                log_event(
                    LogLevel::Error,
                    "app.state_initialization_failed",
                    "result=failed",
                );
            })?;
            let should_show_settings = match state.settings.lock() {
                Ok(settings) => settings.api_key.trim().is_empty(),
                Err(_) => {
                    log_event(
                        LogLevel::Error,
                        "settings.initial_state_read_failed",
                        "fallback=open_settings",
                    );
                    true
                }
            };

            app.manage(state);
            #[cfg(target_os = "macos")]
            {
                configure_macos_float_window(app.handle()).inspect_err(|_| {
                    log_event(
                        LogLevel::Error,
                        "window.macos_panel_setup_failed",
                        "result=failed",
                    );
                })?;
                setup_macos_float_click_monitor(app.handle()).inspect_err(|_| {
                    log_event(
                        LogLevel::Error,
                        "input.macos_click_monitor_setup_failed",
                        "result=failed",
                    );
                })?;
            }
            setup_window_events(app);
            setup_tray(app).inspect_err(|_| {
                log_event(LogLevel::Error, "tray.setup_failed", "result=failed");
            })?;
            setup_global_shortcut(app).inspect_err(|_| {
                log_event(LogLevel::Error, "hotkey.setup_failed", "result=failed");
            })?;

            if should_show_settings {
                log_event(
                    LogLevel::Info,
                    "settings.api_key_missing",
                    "action=open_settings",
                );
                show_settings(app.handle().clone()).map_err(anyhow::Error::msg)?;
            }

            log_event(LogLevel::Info, "app.ready", "result=success");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            current_float,
            get_settings,
            hide_float,
            hide_menu,
            hide_settings,
            list_words,
            remove_word,
            quit_app,
            report_frontend_event,
            resize_float,
            retry_translation,
            save_settings,
            show_settings,
            show_words,
            translate_text
        ])
        .run(tauri::generate_context!());

    match &run_result {
        Ok(()) => log_event(LogLevel::Info, "app.stopped", "result=success"),
        Err(_) => log_event(LogLevel::Error, "app.stopped", "result=runtime_error"),
    }
    run_result.expect("error while running tauri application");
}

fn setup_global_shortcut(app: &mut tauri::App) -> Result<()> {
    let shortcut = Shortcut::new(Some(Modifiers::ALT), Code::KeyT);
    let registered_shortcut = shortcut;

    app.handle().plugin(
        tauri_plugin_global_shortcut::Builder::new()
            .with_handler(move |app, shortcut, event| {
                if shortcut == &registered_shortcut && event.state() == ShortcutState::Pressed {
                    log_event(
                        LogLevel::Info,
                        "hotkey.pressed",
                        "action=translate_selection",
                    );
                    let app = app.clone();
                    tauri::async_runtime::spawn(async move {
                        if let Err(error) = process_hotkey(app.clone()).await {
                            log_event(
                                LogLevel::Error,
                                "hotkey.processing_failed",
                                &format!("category={}", hotkey_error_category(&error)),
                            );
                            show_error_float(&app, "无法读取选区", friendly_hotkey_error(&error));
                        }
                    });
                }
            })
            .build(),
    )?;

    app.global_shortcut().register(shortcut)?;
    log_event(LogLevel::Info, "hotkey.registered", "shortcut=Alt+T");
    Ok(())
}

#[cfg(target_os = "macos")]
fn configure_macos_float_window(app: &AppHandle) -> Result<()> {
    let window = app
        .get_webview_window("float")
        .context("float window not found")?;
    let panel = window
        .to_panel::<WordSnapFloatPanel>()
        .context("failed to convert the macOS float window to an NSPanel")?;

    // WordSnap 保持非活动状态时，普通 NSWindow 无法可靠显示在其他应用的原生全屏
    // Space 上方。采用 NSPanel 和非激活样式，符合 Spotlight 类工具的 AppKit 实现方式。
    panel.set_level(PanelLevel::Floating.value());
    panel.set_style_mask(StyleMask::empty().nonactivating_panel().into());
    panel.set_floating_panel(true);
    panel.set_hides_on_deactivate(false);
    panel.set_becomes_key_only_if_needed(true);
    panel.set_works_when_modal(true);

    panel.set_collection_behavior(
        CollectionBehavior::new()
            .can_join_all_spaces()
            .full_screen_auxiliary()
            .into(),
    );

    // 如果 AppKit 拒绝共享原生全屏 Space 所需的任一属性，应立即返回错误，避免未来的
    // 依赖或 macOS 变更使窗口在无提示的情况下恢复为不可见状态。
    let native_panel = panel.as_panel();
    if !native_panel
        .styleMask()
        .contains(NSWindowStyleMask::NonactivatingPanel)
    {
        anyhow::bail!("macOS translation panel is activating");
    }
    let required_behavior = NSWindowCollectionBehavior::CanJoinAllSpaces
        | NSWindowCollectionBehavior::FullScreenAuxiliary;
    if !native_panel
        .collectionBehavior()
        .contains(required_behavior)
    {
        anyhow::bail!("macOS translation panel cannot join full-screen Spaces");
    }
    if !panel.is_floating_panel() || panel.hides_on_deactivate() {
        anyhow::bail!("macOS translation panel floating behavior was not applied");
    }

    Ok(())
}

#[cfg(target_os = "macos")]
fn show_float_without_activation(window: &tauri::WebviewWindow) {
    let app = window.app_handle().clone();
    if let Err(error) = window.run_on_main_thread(move || match app.get_webview_panel("float") {
        Ok(panel) => panel.show(),
        Err(_) => log_event(
            LogLevel::Error,
            "window.macos_panel_access_failed",
            "result=failed",
        ),
    }) {
        let _ = error;
        log_event(
            LogLevel::Error,
            "window.float_show_schedule_failed",
            "result=failed",
        );
    }
}

#[cfg(target_os = "macos")]
fn setup_macos_float_click_monitor(app: &AppHandle) -> Result<()> {
    let mouse_down_mask =
        NSEventMask::LeftMouseDown | NSEventMask::RightMouseDown | NSEventMask::OtherMouseDown;

    // 全局监听器接收发送至其他应用的点击事件，适用于非激活翻译面板的主要使用场景。
    let app_for_global = app.clone();
    let global_handler = block2::RcBlock::new(move |_event: std::ptr::NonNull<MacNSEvent>| {
        schedule_hide_completed_float(&app_for_global);
    });
    let global_monitor =
        MacNSEvent::addGlobalMonitorForEventsMatchingMask_handler(mouse_down_mask, &global_handler)
            .context("failed to install the macOS global click monitor")?;

    // 全局监听器不会接收当前应用的事件，因此使用本地监听器处理 WordSnap 托盘、菜单和
    // 设置窗口中的点击，同时保留翻译卡片内部的点击行为。
    let app_for_local = app.clone();
    let local_handler = block2::RcBlock::new(
        move |event: std::ptr::NonNull<MacNSEvent>| -> *mut MacNSEvent {
            let event_ref = unsafe { event.as_ref() };
            let is_float_click = app_for_local
                .get_webview_panel("float")
                .ok()
                .is_some_and(|panel| panel.as_panel().windowNumber() == event_ref.windowNumber());
            if !is_float_click {
                hide_completed_float(&app_for_local);
            }
            event.as_ptr()
        },
    );
    let local_monitor = unsafe {
        MacNSEvent::addLocalMonitorForEventsMatchingMask_handler(mouse_down_mask, &local_handler)
    }
    .context("failed to install the macOS local click monitor")?;

    MAC_FLOAT_CLICK_MONITORS.with(|monitors| {
        monitors
            .borrow_mut()
            .extend([global_monitor, local_monitor]);
    });
    Ok(())
}

#[cfg(target_os = "macos")]
fn schedule_hide_completed_float(app: &AppHandle) {
    let app_for_main = app.clone();
    if app
        .run_on_main_thread(move || hide_completed_float(&app_for_main))
        .is_err()
    {
        log_event(
            LogLevel::Error,
            "window.float_hide_schedule_failed",
            "result=failed",
        );
    }
}

fn float_is_dismissible(state: &str) -> bool {
    matches!(state, "word" | "sentence" | "error")
}

fn hide_completed_float(app: &AppHandle) {
    let should_hide = app
        .try_state::<AppState>()
        .and_then(|state| {
            state
                .float
                .lock()
                .ok()
                .map(|payload| float_is_dismissible(&payload.state))
        })
        .unwrap_or(false);
    if !should_hide {
        return;
    }

    #[cfg(target_os = "macos")]
    if let Ok(panel) = app.get_webview_panel("float") {
        panel.hide();
    }
    #[cfg(not(target_os = "macos"))]
    if let Some(window) = app.get_webview_window("float") {
        log_ignored_error(window.hide(), "window.float_hide_failed");
    }
}

fn setup_tray(app: &mut tauri::App) -> Result<()> {
    #[cfg(target_os = "macos")]
    let tooltip_text = "WordSnap · ⌥T 翻译";
    #[cfg(not(target_os = "macos"))]
    let tooltip_text = "WordSnap · Alt+T 翻译";

    TrayIconBuilder::with_id("wordsnap")
        .tooltip(tooltip_text)
        .icon(tray_template_icon()?)
        .icon_as_template(true)
        .show_menu_on_left_click(false)
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                position,
                ..
            } = event
            {
                log_event(LogLevel::Info, "tray.clicked", "button=left");
                toggle_menu_window(tray.app_handle(), position.x as i32, position.y as i32);
            }
        })
        .build(app)?;

    log_event(LogLevel::Info, "tray.initialized", "result=success");
    Ok(())
}

fn tray_template_icon() -> Result<Image<'static>> {
    Image::from_bytes(include_bytes!("../icons/tray-template.png"))
        .map_err(|error| anyhow!("failed to load WordSnap menu bar icon: {error}"))
}

fn setup_window_events(app: &mut tauri::App) {
    log_event(
        LogLevel::Debug,
        "window.event_handlers_setup_started",
        "result=started",
    );
    for label in ["words", "settings"] {
        if let Some(window) = app.get_webview_window(label) {
            let window_for_event = window.clone();
            window.on_window_event(move |event| {
                if let WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    log_ignored_error(window_for_event.hide(), "window.close_hide_failed");
                    log_event(LogLevel::Info, "window.close_intercepted", "action=hide");
                }
            });
        }
    }

    for label in ["float", "menu"] {
        if let Some(window) = app.get_webview_window(label) {
            let window_for_event = window.clone();
            let app_for_event = app.handle().clone();
            let label_for_event = label;
            window.on_window_event(move |event| {
                if let WindowEvent::Focused(false) = event {
                    if label_for_event == "float" {
                        hide_completed_float(&app_for_event);
                    } else {
                        log_ignored_error(window_for_event.hide(), "window.focus_loss_hide_failed");
                    }
                }
            });
        }
    }
    log_event(
        LogLevel::Debug,
        "window.event_handlers_setup_completed",
        "result=success",
    );
}

async fn process_hotkey(app: AppHandle) -> Result<()> {
    let started = Instant::now();
    log_event(
        LogLevel::Debug,
        "selection.capture_started",
        "source=hotkey",
    );
    // 在其他操作可能移动鼠标前记录当前选区位置，并在后续重绘中复用该锚点，避免浮窗
    // 位置发生跳动。
    let anchor = cursor_position_on_main(&app);
    if let Some(state) = app.try_state::<AppState>() {
        if let Ok(mut stored) = state.anchor.lock() {
            *stored = anchor;
        }
    }

    let selected = capture_selected_text_on_main(&app)
        .inspect_err(|_| {
            log_event(LogLevel::Error, "selection.capture_failed", "result=failed");
        })
        .context("failed to capture selected text")?;
    if selected.trim().is_empty() {
        log_event(
            LogLevel::Info,
            "selection.capture_empty",
            &format!("elapsed_ms={}", started.elapsed().as_millis()),
        );
        #[cfg(target_os = "macos")]
        let error_msg =
            "请先选中可复制的文本，再按 ⌥T。若已选中，请在系统设置中允许 WordSnap 使用辅助功能。";
        #[cfg(not(target_os = "macos"))]
        let error_msg = "请先选中可复制的文本，再按 Alt+T。";

        show_error_float(&app, "未读取到文本", error_msg);
        return Ok(());
    }

    log_event(
        LogLevel::Info,
        "selection.captured",
        &format!(
            "text_chars={} elapsed_ms={}",
            selected.trim().chars().count(),
            started.elapsed().as_millis()
        ),
    );
    let state = app.state::<AppState>();
    translate_selection(app.clone(), state.inner(), selected).await?;
    Ok(())
}

async fn translate_selection(
    app: AppHandle,
    state: &AppState,
    selected: String,
) -> Result<FloatPayload> {
    let started = Instant::now();
    let original = selected.trim().to_string();
    let is_word = is_single_english_word(&original);
    log_event(
        LogLevel::Info,
        "translation.started",
        &format!("text_chars={} is_word={is_word}", original.chars().count()),
    );
    let loading = FloatPayload {
        state: "loading".to_string(),
        original: original.clone(),
        translation: None,
        is_word,
        count: None,
        error: None,
    };
    set_float_payload(&app, loading, if is_word { 250 } else { 320 }, 96);

    let settings = state
        .settings
        .lock()
        .map_err(|_| {
            log_event(
                LogLevel::Error,
                "translation.settings_read_failed",
                "reason=state_lock",
            );
            anyhow!("settings lock poisoned")
        })?
        .clone();
    let api_key = settings.api_key.trim().to_string();
    if api_key.is_empty() {
        log_event(
            LogLevel::Info,
            "translation.rejected",
            "reason=api_key_missing",
        );
        let payload = FloatPayload {
            state: "error".to_string(),
            original,
            translation: None,
            is_word,
            count: None,
            error: Some("请先点击菜单栏图标，在「设置…」里填写 API Key。".to_string()),
        };
        set_float_payload(&app, payload.clone(), 300, 150);
        return Ok(payload);
    }

    let translated =
        match call_translation_api(&state.client, &settings, &api_key, &original, is_word).await {
            Ok(text) => text,
            Err(error) => {
                log_event(
                    LogLevel::Error,
                    "translation.failed",
                    &format!(
                        "category={} elapsed_ms={}",
                        translation_error_category(&error),
                        started.elapsed().as_millis()
                    ),
                );
                let payload = FloatPayload {
                    state: "error".to_string(),
                    original,
                    translation: None,
                    is_word,
                    count: None,
                    error: Some(error.to_string()),
                };
                set_float_payload(&app, payload.clone(), 300, 150);
                return Ok(payload);
            }
        };

    let count = if is_word {
        Some(
            upsert_word(state, &original.to_lowercase(), &translated).inspect_err(|_| {
                log_event(LogLevel::Error, "words.upsert_failed", "result=failed");
            })?,
        )
    } else {
        None
    };

    if is_word {
        log_ignored_error(app.emit("words-updated", ()), "words.update_event_failed");
        log_event(LogLevel::Info, "words.upserted", "result=success");
    }

    let payload = FloatPayload {
        state: if is_word { "word" } else { "sentence" }.to_string(),
        original,
        translation: Some(translated),
        is_word,
        count,
        error: None,
    };

    let width = if is_word { 300 } else { 340 };
    set_float_payload(&app, payload.clone(), width, 150);
    log_event(
        LogLevel::Info,
        "translation.completed",
        &format!(
            "is_word={is_word} output_chars={} elapsed_ms={}",
            payload
                .translation
                .as_deref()
                .map(str::chars)
                .map(Iterator::count)
                .unwrap_or(0),
            started.elapsed().as_millis()
        ),
    );
    Ok(payload)
}

fn show_error_float(app: &AppHandle, original: impl Into<String>, error: impl Into<String>) {
    log_event(
        LogLevel::Debug,
        "window.error_float_requested",
        "result=started",
    );
    let payload = FloatPayload {
        state: "error".to_string(),
        original: original.into(),
        translation: None,
        is_word: false,
        count: None,
        error: Some(error.into()),
    };
    set_float_payload(app, payload, 300, 150);
}

fn friendly_hotkey_error(error: &anyhow::Error) -> &'static str {
    let message = error.to_string();
    if message.contains("capture selected text") || message.contains("input simulator") {
        #[cfg(target_os = "macos")]
        return "无法读取当前选中文本。请确认文本可以复制，并在系统设置中允许 WordSnap 使用辅助功能。";
        #[cfg(not(target_os = "macos"))]
        return "无法读取当前选中文本。请确认文本可以复制。";
    } else {
        "翻译失败，请检查网络或 API 设置。"
    }
}

fn hotkey_error_category(error: &anyhow::Error) -> &'static str {
    let message = error.to_string();
    if message.contains("timed out") {
        "selection_timeout"
    } else if message.contains("capture selected text") {
        "selection_capture"
    } else if message.contains("input simulator") {
        "input_simulator"
    } else {
        "translation_or_internal"
    }
}

fn translation_error_category(error: &anyhow::Error) -> &'static str {
    let message = error.to_string();
    if message.contains("连接超时") {
        "timeout"
    } else if message.contains("无法连接服务器") {
        "connection"
    } else if message.contains("API Key") {
        "authentication"
    } else if message.contains("API 返回错误") {
        "http_status"
    } else if message.contains("JSON") || message.contains("缺少翻译内容") {
        "invalid_response"
    } else if message.contains("空的翻译结果") {
        "empty_response"
    } else {
        "network_or_internal"
    }
}

async fn call_translation_api(
    client: &Client,
    settings: &StoredSettings,
    api_key: &str,
    text: &str,
    is_word: bool,
) -> Result<String> {
    let started = Instant::now();
    let target_lang = if settings.target_lang.trim().is_empty() {
        StoredSettings::default().target_lang
    } else {
        settings.target_lang.trim().to_string()
    };

    let prompt = if is_word {
        format!(
            "请把下面的文本翻译成{}。\n如果当前文本已经是简体中文，请翻译为英文。\n只返回{}中最常见的释义或译法,多个释义用分号分隔。\n不要例句,不要词性,不要解释,不要 Markdown。\n\n文本:\n{}",
            target_lang, target_lang, text
        )
    } else {
        format!(
            "请把下面的文本翻译成{}。\n如果当前文本已经是简体中文，请翻译为英文。\n只返回{}译文,不要解释,不要例句,不要 Markdown。\n\n文本:\n{}",
            target_lang, target_lang, text
        )
    };

    let request = ChatRequest {
        model: settings.model.clone(),
        temperature: 0.0,
        messages: vec![ChatMessage {
            role: "user".to_string(),
            content: prompt,
        }],
    };

    log_event(
        LogLevel::Info,
        "network.translation_request_started",
        &format!(
            "transport={} is_word={is_word}",
            if settings.base_url.starts_with("https://") {
                "https"
            } else {
                "http_or_other"
            }
        ),
    );
    let response = match client
        .post(chat_completions_url(&settings.base_url))
        .bearer_auth(api_key)
        .json(&request)
        .send()
        .await
    {
        Ok(response) => response,
        Err(error) => {
            let reason = if error.is_timeout() {
                "连接超时"
            } else if error.is_connect() {
                "无法连接服务器"
            } else {
                "网络请求失败"
            };
            log_event(
                LogLevel::Error,
                "network.translation_request_failed",
                &format!(
                    "category={} elapsed_ms={}",
                    if error.is_timeout() {
                        "timeout"
                    } else if error.is_connect() {
                        "connection"
                    } else {
                        "request"
                    },
                    started.elapsed().as_millis()
                ),
            );
            return Err(anyhow!(
                "{reason}：请检查网络，或确认「模型地址」是否正确可达。"
            ));
        }
    };

    let status = response.status();
    if !status.is_success() {
        log_event(
            LogLevel::Error,
            "network.translation_http_error",
            &format!(
                "status={} elapsed_ms={}",
                status.as_u16(),
                started.elapsed().as_millis()
            ),
        );
        if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
            return Err(anyhow!("API Key 无效或无权限，请在「设置…」中检查。"));
        }
        let detail = response.text().await.unwrap_or_default();
        return Err(anyhow!(
            "API 返回错误 {}：{}",
            status.as_u16(),
            extract_api_error(&detail)
        ));
    }

    let raw = response.text().await.context("无法读取 API 响应内容。")?;
    let body: Value = serde_json::from_str(&raw)
        .map_err(|_| anyhow!("API 响应不是有效的 JSON：{}", truncate(&raw, 120)))?;
    let content = body
        .get("choices")
        .and_then(|choices| choices.get(0))
        .and_then(|choice| choice.get("message"))
        .and_then(|message| message.get("content"))
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("API 响应缺少翻译内容：{}", truncate(&raw, 120)))?;

    let trimmed = content.trim().replace('；', ";");
    if trimmed.is_empty() {
        log_event(
            LogLevel::Error,
            "network.translation_response_empty",
            &format!("elapsed_ms={}", started.elapsed().as_millis()),
        );
        Err(anyhow!("API 返回了空的翻译结果。"))
    } else {
        log_event(
            LogLevel::Info,
            "network.translation_request_completed",
            &format!(
                "status={} response_chars={} elapsed_ms={}",
                status.as_u16(),
                trimmed.chars().count(),
                started.elapsed().as_millis()
            ),
        );
        Ok(trimmed)
    }
}

/// 从 OpenAI 格式的错误正文 `{"error":{"message":"…"}}` 中提取可读消息。解析失败时，
/// 返回截断后的原始正文，使界面能够显示服务端原因，而不是通用的翻译失败提示。
fn extract_api_error(body: &str) -> String {
    serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|value| {
            value
                .get("error")
                .and_then(|error| error.get("message"))
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .filter(|message| !message.trim().is_empty())
        .unwrap_or_else(|| truncate(body, 140))
}

fn truncate(text: &str, max_chars: usize) -> String {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return "无响应内容".to_string();
    }
    let mut result: String = trimmed.chars().take(max_chars).collect();
    if trimmed.chars().count() > max_chars {
        result.push('…');
    }
    result
}

#[cfg(target_os = "macos")]
struct MacPasteboardItemSnapshot {
    representations: Vec<MacPasteboardRepresentation>,
}

#[cfg(target_os = "macos")]
struct MacPasteboardRepresentation {
    data_type: MacRetained<NSPasteboardType>,
    data: Vec<u8>,
}

#[cfg(target_os = "macos")]
struct MacClipboardRestoreGuard {
    pasteboard: MacRetained<NSPasteboard>,
    snapshot: Vec<MacPasteboardItemSnapshot>,
    restore_if_change_count: isize,
}

#[cfg(target_os = "macos")]
impl MacClipboardRestoreGuard {
    fn new() -> Self {
        let pasteboard = NSPasteboard::generalPasteboard();
        let restore_if_change_count = pasteboard.changeCount();
        let snapshot = snapshot_macos_pasteboard(&pasteboard);
        log_event(
            LogLevel::Debug,
            "clipboard.snapshot_created",
            &format!("items={}", snapshot.len()),
        );

        Self {
            pasteboard,
            snapshot,
            restore_if_change_count,
        }
    }

    fn clear_for_capture(&mut self) {
        self.restore_if_change_count = self.pasteboard.clearContents();
        log_event(
            LogLevel::Debug,
            "clipboard.cleared_for_capture",
            "result=success",
        );
    }

    fn mark_current_as_owned(&mut self) {
        self.restore_if_change_count = self.pasteboard.changeCount();
    }

    fn restore(&mut self) {
        // 如果用户在 WordSnap 翻译期间复制了新内容，不得使用先前快照覆盖新剪贴板内容。
        if self.pasteboard.changeCount() != self.restore_if_change_count {
            log_event(
                LogLevel::Info,
                "clipboard.restore_skipped",
                "reason=clipboard_changed_by_user",
            );
            return;
        }

        self.pasteboard.clearContents();
        if self.snapshot.is_empty() {
            log_event(LogLevel::Debug, "clipboard.restore_completed", "items=0");
            return;
        }

        let mut items = Vec::new();
        for snapshot_item in &self.snapshot {
            let pasteboard_item = NSPasteboardItem::new();
            let mut wrote_any = false;

            for representation in &snapshot_item.representations {
                let data = NSData::with_bytes(&representation.data);
                wrote_any |= pasteboard_item.setData_forType(&data, &representation.data_type);
            }

            if wrote_any {
                items.push(MacProtocolObject::from_retained(pasteboard_item));
            }
        }

        if !items.is_empty() {
            let objects = NSArray::from_retained_slice(&items);
            if self.pasteboard.writeObjects(&objects) {
                log_event(
                    LogLevel::Debug,
                    "clipboard.restore_completed",
                    &format!("items={}", items.len()),
                );
            } else {
                log_event(
                    LogLevel::Error,
                    "clipboard.restore_failed",
                    "platform=macos",
                );
            }
        }
    }
}

#[cfg(target_os = "macos")]
impl Drop for MacClipboardRestoreGuard {
    fn drop(&mut self) {
        self.restore();
    }
}

#[cfg(target_os = "macos")]
fn snapshot_macos_pasteboard(pasteboard: &NSPasteboard) -> Vec<MacPasteboardItemSnapshot> {
    let Some(items) = pasteboard.pasteboardItems() else {
        return Vec::new();
    };

    items
        .to_vec()
        .into_iter()
        .filter_map(|item| {
            let representations = item
                .types()
                .to_vec()
                .into_iter()
                .filter_map(|data_type| {
                    item.dataForType(&data_type)
                        .map(|data| MacPasteboardRepresentation {
                            data_type,
                            data: data.to_vec(),
                        })
                })
                .collect::<Vec<_>>();

            (!representations.is_empty()).then_some(MacPasteboardItemSnapshot { representations })
        })
        .collect()
}

#[cfg(target_os = "macos")]
fn capture_selected_text() -> Result<String> {
    log_event(
        LogLevel::Debug,
        "clipboard.capture_started",
        "platform=macos",
    );
    let mut restore_guard = MacClipboardRestoreGuard::new();
    restore_guard.clear_for_capture();

    simulate_copy_selection()?;

    let mut selected = String::new();
    for _ in 0..50 {
        thread::sleep(Duration::from_millis(12));
        if let Some(text) = restore_guard
            .pasteboard
            .stringForType(unsafe { NSPasteboardTypeString })
        {
            let text = text.to_string();
            if !text.is_empty() {
                selected = text;
                restore_guard.mark_current_as_owned();
                break;
            }
        }
    }

    Ok(selected)
}

#[cfg(not(target_os = "macos"))]
fn capture_selected_text() -> Result<String> {
    log_event(
        LogLevel::Debug,
        "clipboard.capture_started",
        "platform=non_macos",
    );
    let mut clipboard = Clipboard::new().context("failed to open clipboard")?;
    let previous = clipboard.get_text().ok();

    // 模拟复制前清空剪贴板。复制操作未写入内容时，读取结果应为空，而不应保留先前文本，
    // 否则应用会错误翻译旧内容。Windows 曾因此只翻译最近一次手动复制的文本。
    if clipboard.set_text(String::new()).is_err() {
        log_event(
            LogLevel::Error,
            "clipboard.clear_failed",
            "platform=non_macos",
        );
    }

    simulate_copy_selection()?;

    // 不同应用写入剪贴板的延迟不同，因此轮询等待复制结果，不使用单次固定延迟。等待时间
    // 过短会产生旧结果或空结果。轮询上限约为 600 毫秒，低于主线程读取的 3 秒超时。
    let mut selected = String::new();
    for _ in 0..50 {
        thread::sleep(Duration::from_millis(12));
        if let Ok(text) = clipboard.get_text() {
            if !text.is_empty() {
                selected = text;
                break;
            }
        }
    }

    if let Some(previous_text) = previous {
        if clipboard.set_text(previous_text).is_err() {
            log_event(
                LogLevel::Error,
                "clipboard.restore_failed",
                "platform=non_macos",
            );
        } else {
            log_event(
                LogLevel::Debug,
                "clipboard.restore_completed",
                "format=text",
            );
        }
    }

    Ok(selected)
}

fn simulate_copy_selection() -> Result<()> {
    log_event(
        LogLevel::Debug,
        "input.copy_simulation_started",
        "result=started",
    );
    let mut enigo = Enigo::new(&EnigoSettings::default())
        .map_err(|error| anyhow!("failed to initialize input simulator: {error:?}"))?;

    // 全局快捷键在按键按下时触发，因此执行到此处时，Alt+T 或 Option+T 中的 Alt 键仍处于
    // 物理按下状态。在 Windows 和 Linux 上，模拟输入与真实键盘共享修饰键状态，继续按住
    // Alt 会使复制组合键变为 Ctrl+Alt+C，导致双击选择的文本无法读取。执行复制前应释放
    // Alt。macOS 模拟事件携带独立的修饰键标志，不受此问题影响，因此不修改其按键状态。
    #[cfg(not(target_os = "macos"))]
    {
        if enigo.key(Key::Alt, Release).is_err() {
            log_event(
                LogLevel::Error,
                "input.alt_release_failed",
                "platform=non_macos",
            );
        }
        thread::sleep(Duration::from_millis(20));
    }

    #[cfg(target_os = "macos")]
    let modifier = Key::Meta;
    #[cfg(not(target_os = "macos"))]
    let modifier = Key::Control;

    #[cfg(target_os = "macos")]
    let key_c = Key::Unicode('c');
    #[cfg(target_os = "windows")]
    let key_c = Key::Other(0x43); // Windows 上使用 Unicode('c') 会绕过 Ctrl 修饰键，因此使用 VK_C。
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let key_c = Key::Unicode('c');

    enigo.key(modifier, Press).map_err(enigo_err)?;
    enigo.key(key_c, Click).map_err(enigo_err)?;
    enigo.key(modifier, Release).map_err(enigo_err)?;

    log_event(
        LogLevel::Debug,
        "input.copy_simulation_completed",
        "result=success",
    );
    Ok(())
}

fn capture_selected_text_on_main(app: &AppHandle) -> Result<String> {
    let (tx, rx) = mpsc::channel();

    app.run_on_main_thread(move || {
        let result = capture_selected_text();
        if tx.send(result).is_err() {
            log_event(
                LogLevel::Error,
                "selection.result_channel_send_failed",
                "result=failed",
            );
        }
    })
    .context("failed to schedule selection capture on main thread")?;

    rx.recv_timeout(Duration::from_secs(3))
        .context("timed out while capturing selected text on main thread")?
}

fn cursor_position() -> (i32, i32) {
    let Ok(input) = Enigo::new(&EnigoSettings::default()) else {
        log_event(
            LogLevel::Error,
            "cursor.input_initialization_failed",
            "fallback=default",
        );
        return (620, 260);
    };
    input.location().unwrap_or_else(|_| {
        log_event(
            LogLevel::Error,
            "cursor.position_read_failed",
            "fallback=default",
        );
        (620, 260)
    })
}

/// 在主线程中读取鼠标位置。macOS 输入 API 必须由主线程调用；在派生任务中调用
/// `cursor_position()` 可能直接返回备用值，使浮窗显示在错误位置。返回值已经通过
/// `logical_cursor_position` 转换为逻辑坐标。
fn cursor_position_on_main(app: &AppHandle) -> (i32, i32) {
    let (tx, rx) = mpsc::channel();
    let app_for_main = app.clone();
    if app
        .run_on_main_thread(move || {
            if tx.send(logical_cursor_position(&app_for_main)).is_err() {
                log_event(
                    LogLevel::Error,
                    "cursor.result_channel_send_failed",
                    "result=failed",
                );
            }
        })
        .is_ok()
    {
        if let Ok(pos) = rx.recv_timeout(Duration::from_secs(1)) {
            return pos;
        }
        log_event(
            LogLevel::Error,
            "cursor.main_thread_read_timeout",
            "fallback=direct_read",
        );
    } else {
        log_event(
            LogLevel::Error,
            "cursor.main_thread_schedule_failed",
            "fallback=direct_read",
        );
    }
    logical_cursor_position(app)
}

/// enigo 在 Windows 和 Linux 上以物理像素报告鼠标位置，在 macOS 上则以逻辑点报告。
/// 浮窗使用 `LogicalPosition` 定位，因此，在启用缩放的 Windows 或 Linux 显示器上直接
/// 使用物理坐标，会使浮窗按缩放系数偏向右下方。此函数将坐标统一转换为逻辑点，以保证
/// 所有平台上的锚点位置正确。
fn logical_cursor_position(app: &AppHandle) -> (i32, i32) {
    let (raw_x, raw_y) = cursor_position();
    #[cfg(target_os = "macos")]
    {
        let _ = app;
        (raw_x, raw_y)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let scale = app
            .get_webview_window("float")
            .and_then(|window| {
                monitor_scale_for_physical_point(&window, raw_x as f64, raw_y as f64)
            })
            .unwrap_or_else(|| {
                log_event(
                    LogLevel::Error,
                    "cursor.monitor_scale_missing",
                    "fallback=1",
                );
                1.0
            });
        (
            (raw_x as f64 / scale).round() as i32,
            (raw_y as f64 / scale).round() as i32,
        )
    }
}

/// 返回物理边界包含指定物理坐标的显示器缩放系数。无法匹配时，使用当前显示器或主显示器。
#[cfg(not(target_os = "macos"))]
fn monitor_scale_for_physical_point(window: &tauri::WebviewWindow, x: f64, y: f64) -> Option<f64> {
    if let Ok(monitors) = window.available_monitors() {
        for monitor in &monitors {
            let pos = monitor.position();
            let size = monitor.size();
            let left = pos.x as f64;
            let top = pos.y as f64;
            if x >= left && x < left + size.width as f64 && y >= top && y < top + size.height as f64
            {
                return Some(monitor.scale_factor());
            }
        }
    }
    window
        .current_monitor()
        .ok()
        .flatten()
        .or_else(|| window.primary_monitor().ok().flatten())
        .map(|monitor| monitor.scale_factor())
}

fn set_float_payload(app: &AppHandle, payload: FloatPayload, width: u32, height: u32) {
    log_event(
        LogLevel::Debug,
        "window.float_payload_update_started",
        &format!("state={} width={width} height={height}", payload.state),
    );
    if let Some(state) = app.try_state::<AppState>() {
        if let Ok(mut current) = state.float.lock() {
            *current = payload.clone();
        } else {
            log_event(
                LogLevel::Error,
                "window.float_state_update_failed",
                "reason=state_lock",
            );
        }
    } else {
        log_event(
            LogLevel::Error,
            "window.float_state_update_failed",
            "reason=state_missing",
        );
    }

    if let Some(window) = app.get_webview_window("float") {
        let (cursor_x, cursor_y) = app
            .try_state::<AppState>()
            .and_then(|state| state.anchor.lock().ok().map(|anchor| *anchor))
            .unwrap_or((620, 260));
        let (x, y) = float_origin(
            &window,
            cursor_x as f64,
            cursor_y as f64,
            width as f64,
            height as f64,
        );
        log_ignored_error(
            window.set_size(LogicalSize::new(width as f64, height as f64)),
            "window.float_size_failed",
        );
        log_ignored_error(
            window.set_position(LogicalPosition::new(x, y)),
            "window.float_position_failed",
        );
        log_ignored_error(
            window.emit("float-updated", payload),
            "window.float_update_event_failed",
        );
        #[cfg(target_os = "macos")]
        show_float_without_activation(&window);
        #[cfg(not(target_os = "macos"))]
        {
            if window.show().is_err() {
                log_event(LogLevel::Error, "window.float_show_failed", "result=failed");
            }
            if window.set_focus().is_err() {
                log_event(
                    LogLevel::Error,
                    "window.float_focus_failed",
                    "result=failed",
                );
            }
        }
    } else {
        log_event(
            LogLevel::Error,
            "window.float_update_failed",
            "reason=window_missing",
        );
    }
}

/// 将浮窗放置在选区或鼠标的下方并略微向左偏移，同时保证整个卡片位于目标显示器内。
/// 所有计算均使用逻辑点，以保证 Retina 显示器上的定位正确。
fn float_origin(
    window: &tauri::WebviewWindow,
    cursor_x: f64,
    cursor_y: f64,
    width: f64,
    height: f64,
) -> (f64, f64) {
    // 使浮窗位于鼠标左下方，避免覆盖用户刚选中的文本。
    let mut x = cursor_x - 24.0;
    let mut y = cursor_y + 20.0;

    // 根据选区所在的显示器限制浮窗位置，不使用浮窗先前所在的显示器，以保证辅助显示器上的
    // 选区仍在该显示器上显示浮窗。
    if let Some((left, top, right, bottom)) = monitor_bounds_for_point(window, cursor_x, cursor_y) {
        let left = left + 8.0;
        let top = top + 8.0;
        let right = right - 8.0;
        let bottom = bottom - 8.0;
        x = x.clamp(left, (right - width).max(left));
        y = y.clamp(top, (bottom - height).max(top));
    }

    (x, y)
}

/// 返回包含指定逻辑坐标的显示器逻辑边界 `(left, top, right, bottom)`。无法匹配时，使用
/// 当前显示器或主显示器。显示器几何信息采用物理坐标，因此需要除以各显示器的缩放系数，
/// 以匹配鼠标位置使用的逻辑坐标空间。
fn monitor_bounds_for_point(
    window: &tauri::WebviewWindow,
    x: f64,
    y: f64,
) -> Option<(f64, f64, f64, f64)> {
    let bounds = |monitor: &tauri::window::Monitor| {
        let scale = monitor.scale_factor();
        let pos = monitor.position();
        let size = monitor.size();
        let left = pos.x as f64 / scale;
        let top = pos.y as f64 / scale;
        (
            left,
            top,
            left + size.width as f64 / scale,
            top + size.height as f64 / scale,
        )
    };

    if let Ok(monitors) = window.available_monitors() {
        for monitor in &monitors {
            let (left, top, right, bottom) = bounds(monitor);
            if x >= left && x < right && y >= top && y < bottom {
                return Some((left, top, right, bottom));
            }
        }
    }

    let fallback = window
        .current_monitor()
        .ok()
        .flatten()
        .or_else(|| window.primary_monitor().ok().flatten())?;
    Some(bounds(&fallback))
}

fn toggle_menu_window(app: &AppHandle, tray_x: i32, tray_y: i32) {
    if let Some(window) = app.get_webview_window("menu") {
        let is_visible = match window.is_visible() {
            Ok(value) => value,
            Err(_) => {
                log_event(
                    LogLevel::Error,
                    "window.menu_visibility_read_failed",
                    "result=failed",
                );
                false
            }
        };
        if is_visible {
            log_ignored_error(window.hide(), "window.menu_hide_failed");
            log_event(LogLevel::Debug, "window.menu_hidden", "source=tray");
            return;
        }

        let scale_factor = window.scale_factor().unwrap_or_else(|_| {
            log_event(
                LogLevel::Error,
                "window.menu_scale_read_failed",
                "fallback=1",
            );
            1.0
        });

        // 查找包含托盘图标点击位置的显示器。
        let monitor = if let Ok(monitors) = window.available_monitors() {
            monitors.into_iter().find(|m| {
                let pos = m.position();
                let size = m.size();
                tray_x >= pos.x
                    && tray_x < pos.x + size.width as i32
                    && tray_y >= pos.y
                    && tray_y < pos.y + size.height as i32
            })
        } else {
            None
        }
        .or_else(|| window.current_monitor().ok().flatten())
        .or_else(|| window.primary_monitor().ok().flatten());

        if let Some(monitor) = monitor {
            let work_area = monitor.work_area();
            let window_size = window.outer_size().unwrap_or_else(|_| {
                log_event(
                    LogLevel::Error,
                    "window.menu_size_read_failed",
                    "fallback=default",
                );
                Default::default()
            });
            let w = window_size.width as i32;
            let h = window_size.height as i32;

            // 以托盘图标点击位置为基准，使窗口水平居中。
            let mut x = tray_x - w / 2;

            // 根据点击位置判断任务栏位于屏幕顶部还是底部。
            let monitor_center_y = work_area.position.y + (work_area.size.height as i32) / 2;
            let gap = (8.0 * scale_factor) as i32;
            let mut y = if tray_y > monitor_center_y {
                // 任务栏位于底部时，在托盘图标上方显示窗口。
                tray_y - h - gap
            } else {
                // 任务栏位于顶部时，在托盘图标下方显示窗口。
                tray_y + gap
            };

            // 限制两个坐标，保证整个窗口位于显示器工作区内。
            x = x.clamp(
                work_area.position.x,
                work_area.position.x + work_area.size.width as i32 - w,
            );
            y = y.clamp(
                work_area.position.y,
                work_area.position.y + work_area.size.height as i32 - h,
            );

            log_ignored_error(
                window.set_position(PhysicalPosition::new(x, y)),
                "window.menu_position_failed",
            );
        } else {
            // 无法取得显示器信息时，使用原有固定偏移量。
            let x = tray_x.saturating_sub(196);
            let y = tray_y.saturating_add(24);
            log_event(
                LogLevel::Info,
                "window.menu_monitor_fallback",
                "result=used",
            );
            log_ignored_error(
                window.set_position(PhysicalPosition::new(x, y)),
                "window.menu_position_failed",
            );
        }

        log_ignored_error(window.show(), "window.menu_show_failed");
        log_ignored_error(window.set_focus(), "window.menu_focus_failed");
        log_event(LogLevel::Debug, "window.menu_shown", "source=tray");
    } else {
        log_event(
            LogLevel::Error,
            "window.menu_toggle_failed",
            "reason=window_missing",
        );
    }
}

fn hide_menu_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("menu") {
        log_ignored_error(window.hide(), "window.menu_hide_failed");
    }
}

fn show_window(app: &AppHandle, label: &str) -> Result<(), String> {
    let window = app
        .get_webview_window(label)
        .ok_or_else(|| format!("window not found: {label}"))?;
    window.unminimize().map_err(to_string)?;
    window.show().map_err(to_string)?;
    window.set_focus().map_err(to_string)?;
    Ok(())
}

fn read_words(state: &AppState) -> Result<WordListPayload> {
    log_event(
        LogLevel::Debug,
        "database.words_query_started",
        "result=started",
    );
    let conn = state
        .conn
        .lock()
        .map_err(|_| anyhow!("database lock poisoned"))?;
    let total: i64 = conn.query_row("SELECT COUNT(*) FROM words", [], |row| row.get(0))?;

    let mut stmt = conn.prepare(
        "SELECT word, translation, count, first_seen_at, last_seen_at
         FROM words
         ORDER BY last_seen_at DESC",
    )?;
    let rows = stmt.query_map([], |row| {
        let last_seen_at: String = row.get(4)?;
        Ok(WordRecord {
            word: row.get(0)?,
            translation: row.get(1)?,
            count: row.get(2)?,
            first_seen_at: row.get(3)?,
            recent: format_recent(&last_seen_at),
            last_seen_at,
        })
    })?;

    let words = rows.collect::<rusqlite::Result<Vec<_>>>()?;
    log_event(
        LogLevel::Debug,
        "database.words_query_completed",
        &format!("total={total}"),
    );
    Ok(WordListPayload { total, words })
}

fn upsert_word(state: &AppState, word: &str, translation: &str) -> Result<i64> {
    log_event(
        LogLevel::Debug,
        "database.word_upsert_started",
        "result=started",
    );
    let now = Utc::now().to_rfc3339();
    let conn = state
        .conn
        .lock()
        .map_err(|_| anyhow!("database lock poisoned"))?;

    conn.execute(
        "INSERT INTO words (word, translation, count, first_seen_at, last_seen_at)
         VALUES (?1, ?2, 1, ?3, ?3)
         ON CONFLICT(word) DO UPDATE SET
            count = count + 1,
            last_seen_at = excluded.last_seen_at,
            translation = excluded.translation",
        params![word, translation, now],
    )?;

    let count = conn.query_row(
        "SELECT count FROM words WHERE word = ?1",
        params![word],
        |row| row.get(0),
    )?;

    log_event(
        LogLevel::Debug,
        "database.word_upsert_completed",
        &format!("count={count}"),
    );
    Ok(count)
}

fn delete_word(state: &AppState, word: &str) -> Result<()> {
    log_event(
        LogLevel::Debug,
        "database.word_delete_started",
        "result=started",
    );
    let conn = state
        .conn
        .lock()
        .map_err(|_| anyhow!("database lock poisoned"))?;

    let affected = conn.execute("DELETE FROM words WHERE word = ?1", params![word])?;
    log_event(
        LogLevel::Debug,
        "database.word_delete_completed",
        &format!("affected={affected}"),
    );
    Ok(())
}

fn init_db(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS words (
            id            INTEGER PRIMARY KEY,
            word          TEXT UNIQUE,
            translation   TEXT NOT NULL,
            count         INTEGER NOT NULL,
            first_seen_at TEXT NOT NULL,
            last_seen_at  TEXT NOT NULL
        );",
    )
    .context("failed to initialize words table")?;
    Ok(())
}

fn load_settings(app_dir: &Path) -> Result<StoredSettings> {
    let path = app_dir.join("settings.json");
    if !path.exists() {
        return Ok(StoredSettings::default());
    }

    let raw = fs::read_to_string(path).context("failed to read settings.json")?;
    let mut settings: StoredSettings =
        serde_json::from_str(&raw).context("failed to parse settings.json")?;
    if settings.base_url.trim().is_empty() {
        settings.base_url = StoredSettings::default().base_url;
    }
    if settings.model.trim().is_empty() {
        settings.model = StoredSettings::default().model;
    }
    if settings.hotkey.trim().is_empty() {
        settings.hotkey = StoredSettings::default().hotkey;
    }
    if settings.target_lang.trim().is_empty() {
        settings.target_lang = StoredSettings::default().target_lang;
    }
    Ok(settings)
}

fn save_settings_file(app_dir: &Path, settings: &StoredSettings) -> Result<()> {
    fs::create_dir_all(app_dir)?;
    let raw = serde_json::to_string_pretty(settings)?;
    let path = app_dir.join("settings.json");
    fs::write(&path, raw)?;
    // 文件以明文形式保存 API Key，因此将访问权限限制为文件所有者。
    restrict_file_permissions(&path);
    Ok(())
}

fn settings_payload(settings: StoredSettings) -> SettingsPayload {
    let has_key = !settings.api_key.trim().is_empty();
    SettingsPayload {
        base_url: settings.base_url,
        model: settings.model,
        hotkey: settings.hotkey,
        target_lang: settings.target_lang,
        api_key_preview: if has_key {
            Some(mask_api_key(&settings.api_key))
        } else {
            None
        },
        api_key_set: has_key,
    }
}

fn mask_api_key(api_key: &str) -> String {
    let tail: String = api_key
        .chars()
        .rev()
        .take(4)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    format!("•••••••••••• {}", tail)
}

fn app_data_dir(app: &AppHandle) -> Result<PathBuf> {
    app.path()
        .app_data_dir()
        .context("failed to resolve app data directory")
}

fn normalize_base_url(input: &str) -> String {
    let trimmed = input.trim().trim_end_matches('/');
    let with_scheme = if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        trimmed.to_string()
    } else if trimmed.is_empty() {
        StoredSettings::default().base_url
    } else {
        format!("https://{trimmed}")
    };
    with_scheme.trim_end_matches('/').to_string()
}

fn chat_completions_url(base_url: &str) -> String {
    let base = normalize_base_url(base_url);
    if base.ends_with("/chat/completions") {
        base
    } else {
        format!("{base}/chat/completions")
    }
}

fn is_single_english_word(text: &str) -> bool {
    let trimmed = text.trim();
    !trimmed.is_empty()
        && !trimmed.chars().any(char::is_whitespace)
        && trimmed
            .chars()
            .all(|ch| ch.is_ascii_alphabetic() || ch == '-')
        && trimmed.chars().any(|ch| ch.is_ascii_alphabetic())
}

fn format_recent(value: &str) -> String {
    let parsed = DateTime::parse_from_rfc3339(value)
        .map(|date| date.with_timezone(&Local))
        .unwrap_or_else(|_| Local::now());
    let today = Local::now().date_naive();
    let date = parsed.date_naive();

    if date == today {
        parsed.format("%H:%M").to_string()
    } else if date == today.pred_opt().unwrap_or(today) {
        "昨天".to_string()
    } else {
        parsed.format("%-m/%-d").to_string()
    }
}

fn lock_err<T>(_: std::sync::PoisonError<T>) -> String {
    "internal state lock poisoned".to_string()
}

fn enigo_err(error: enigo::InputError) -> anyhow::Error {
    anyhow!("failed to simulate keyboard input: {error:?}")
}

fn to_string<E: std::fmt::Display>(error: E) -> String {
    error.to_string()
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::{
        chat_completions_url, float_is_dismissible, is_single_english_word, mask_api_key,
        normalize_base_url, sanitize_log_field, session_log_filename, AppLogger, LogLevel,
    };

    #[test]
    fn names_session_log_files_with_start_time_and_process_id() {
        assert_eq!(
            session_log_filename("2026-08-12_14-35-22.123", 4567),
            "2026-08-12_14-35-22.123_pid-4567.log"
        );
    }

    #[test]
    fn sanitizes_control_characters_and_limits_log_fields() {
        assert_eq!(
            sanitize_log_field("first\nsecond\tvalue"),
            "first second value"
        );
        assert_eq!(sanitize_log_field(&"a".repeat(300)).chars().count(), 240);
    }

    #[test]
    fn writes_all_levels_to_the_same_session_file() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should follow Unix epoch")
            .as_nanos();
        let directory =
            std::env::temp_dir().join(format!("wordsnap-log-test-{}-{unique}", std::process::id()));
        fs::create_dir_all(&directory).expect("test log directory should be created");

        let started_at = "2026-08-12_14-35-22.123";
        let process_id = 4567;
        let mut logger = AppLogger::new_with_identity(directory.clone(), started_at, process_id)
            .expect("logger should initialize");
        logger
            .write(LogLevel::Debug, "test.debug", "result=success")
            .expect("debug log should be written");
        logger
            .write(LogLevel::Info, "test.info", "result=success")
            .expect("info log should be written");
        logger
            .write(LogLevel::Error, "test.error", "result=failed")
            .expect("error log should be written");
        drop(logger);

        let path = directory.join(session_log_filename(started_at, process_id));
        let contents = fs::read_to_string(&path).expect("session log should be readable");
        assert!(contents.contains("DEBUG test.debug result=success"));
        assert!(contents.contains("INFO test.info result=success"));
        assert!(contents.contains("ERROR test.error result=failed"));

        fs::remove_file(path).expect("test log should be removed");
        fs::remove_dir(directory).expect("test log directory should be removed");
    }

    #[test]
    fn dismisses_only_completed_float_states() {
        for state in ["word", "sentence", "error"] {
            assert!(float_is_dismissible(state), "expected dismissible: {state}");
        }
        for state in ["idle", "loading", "unknown"] {
            assert!(!float_is_dismissible(state), "expected persistent: {state}");
        }
    }

    #[test]
    fn normalizes_openai_compatible_base_urls() {
        assert_eq!(
            normalize_base_url(" api.example.com/v1/ "),
            "https://api.example.com/v1"
        );
        assert_eq!(
            normalize_base_url("http://localhost:11434/v1/"),
            "http://localhost:11434/v1"
        );
        assert_eq!(normalize_base_url(""), "https://api.openai.com/v1");
    }

    #[test]
    fn builds_chat_completions_url_once() {
        assert_eq!(
            chat_completions_url("api.example.com/v1"),
            "https://api.example.com/v1/chat/completions"
        );
        assert_eq!(
            chat_completions_url("https://api.example.com/v1/chat/completions"),
            "https://api.example.com/v1/chat/completions"
        );
    }

    #[test]
    fn identifies_only_single_ascii_english_words() {
        for word in ["hello", " well-known ", "A"] {
            assert!(is_single_english_word(word), "expected word: {word}");
        }
        for text in ["", "two words", "中文", "hello!", "-"] {
            assert!(!is_single_english_word(text), "expected non-word: {text}");
        }
    }

    #[test]
    fn masks_api_keys_without_exposing_more_than_the_tail() {
        assert_eq!(mask_api_key("sk-example-1234"), "•••••••••••• 1234");
        assert_eq!(mask_api_key("abc"), "•••••••••••• abc");
    }
}
