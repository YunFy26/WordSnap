use std::{
    fs,
    path::PathBuf,
    sync::{mpsc, Mutex},
    thread,
    time::Duration,
};

use anyhow::{anyhow, Context, Result};
use arboard::Clipboard;
use chrono::{DateTime, Local, Utc};
use enigo::{
    Direction::{Click, Press, Release},
    Enigo, Key, Keyboard, Mouse, Settings as EnigoSettings,
};
use reqwest::Client;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::image::Image;
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{
    AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, PhysicalPosition, State,
    WindowEvent,
};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

struct AppState {
    conn: Mutex<Connection>,
    settings: Mutex<StoredSettings>,
    float: Mutex<FloatPayload>,
    /// Screen point (logical) the float should anchor to. Captured once when a
    /// translation is triggered so the popup stays put across loading/result/
    /// retry redraws instead of jumping to wherever the cursor now is.
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
    // Stored in this app's own settings.json (owner-only readable), not the
    // system keychain, so saving a key never triggers a macOS permission prompt.
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
        let conn = Connection::open(app_dir.join("wordsnap.sqlite3"))
            .context("failed to open WordSnap SQLite database")?;
        init_db(&conn)?;

        Ok(Self {
            conn: Mutex::new(conn),
            settings: Mutex::new(settings),
            float: Mutex::new(FloatPayload::default()),
            anchor: Mutex::new((0, 0)),
            // Fail fast on an unreachable host (common when the base URL points at
            // a blocked/wrong endpoint) instead of hanging on the spinner.
            client: Client::builder()
                .connect_timeout(Duration::from_secs(8))
                .timeout(Duration::from_secs(45))
                .build()
                .unwrap_or_else(|_| Client::new()),
        })
    }
}

#[tauri::command]
fn get_settings(state: State<'_, AppState>) -> Result<SettingsPayload, String> {
    let settings = state.settings.lock().map_err(lock_err)?.clone();
    Ok(settings_payload(settings))
}

#[tauri::command]
fn save_settings(
    app: AppHandle,
    state: State<'_, AppState>,
    request: SaveSettingsRequest,
) -> Result<SettingsPayload, String> {
    // Keep the previously saved key unless the user typed a new one.
    let existing_key = state.settings.lock().map_err(lock_err)?.api_key.clone();
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

    let app_dir = app_data_dir(&app).map_err(to_string)?;
    save_settings_file(&app_dir, &next).map_err(to_string)?;
    *state.settings.lock().map_err(lock_err)? = next.clone();
    Ok(settings_payload(next))
}

#[tauri::command]
fn list_words(state: State<'_, AppState>) -> Result<WordListPayload, String> {
    read_words(&state).map_err(to_string)
}

#[tauri::command]
fn current_float(state: State<'_, AppState>) -> Result<FloatPayload, String> {
    state
        .float
        .lock()
        .map_err(lock_err)
        .map(|payload| payload.clone())
}

#[tauri::command]
fn show_words(app: AppHandle) -> Result<(), String> {
    hide_menu_window(&app);
    show_window(&app, "words")
}

#[tauri::command]
fn show_settings(app: AppHandle) -> Result<(), String> {
    hide_menu_window(&app);
    show_window(&app, "settings")?;
    // The settings webview is created once and reused, so nudge it to reload its
    // fields every time it opens instead of showing whatever state it was left in.
    if let Some(window) = app.get_webview_window("settings") {
        let _ = window.emit("settings-refresh", ());
    }
    Ok(())
}

#[tauri::command]
fn hide_menu(app: AppHandle) {
    hide_menu_window(&app);
}

#[tauri::command]
fn hide_settings(app: AppHandle) {
    if let Some(window) = app.get_webview_window("settings") {
        let _ = window.hide();
    }
}

#[tauri::command]
fn hide_float(app: AppHandle) {
    if let Some(window) = app.get_webview_window("float") {
        let _ = window.hide();
    }
}

#[tauri::command]
fn resize_float(app: AppHandle, width: u32, height: u32) -> Result<(), String> {
    let window = app
        .get_webview_window("float")
        .ok_or_else(|| "float window not found".to_string())?;
    let width = width.clamp(240, 400) as f64;
    let height = height.clamp(70, 420) as f64;
    window
        .set_size(LogicalSize::new(width, height))
        .map_err(to_string)?;

    // The card may measure taller than the backend's initial guess; re-clamp the
    // origin against the real size so it never grows off the bottom of the screen.
    let (anchor_x, anchor_y) = app
        .try_state::<AppState>()
        .and_then(|state| state.anchor.lock().ok().map(|anchor| *anchor))
        .unwrap_or((620, 260));
    let (x, y) = float_origin(&window, anchor_x as f64, anchor_y as f64, width, height);
    let _ = window.set_position(LogicalPosition::new(x, y));
    Ok(())
}

#[tauri::command]
fn quit_app(app: AppHandle) {
    app.exit(0);
}

#[tauri::command]
async fn translate_text(
    app: AppHandle,
    state: State<'_, AppState>,
    text: String,
) -> Result<FloatPayload, String> {
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
        return Err("没有可重试的文本。".to_string());
    }

    translate_selection(app, state.inner(), original)
        .await
        .map_err(to_string)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            #[cfg(target_os = "macos")]
            {
                app.handle()
                    .set_activation_policy(tauri::ActivationPolicy::Accessory)?;
                app.handle().set_dock_visibility(false)?;
            }

            app.manage(AppState::new(app.handle())?);
            setup_window_events(app);
            setup_tray(app)?;
            setup_global_shortcut(app)?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            current_float,
            get_settings,
            hide_float,
            hide_menu,
            hide_settings,
            list_words,
            quit_app,
            resize_float,
            retry_translation,
            save_settings,
            show_settings,
            show_words,
            translate_text
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn setup_global_shortcut(app: &mut tauri::App) -> Result<()> {
    let shortcut = Shortcut::new(Some(Modifiers::ALT), Code::KeyT);
    let registered_shortcut = shortcut.clone();

    app.handle().plugin(
        tauri_plugin_global_shortcut::Builder::new()
            .with_handler(move |app, shortcut, event| {
                if shortcut == &registered_shortcut && event.state() == ShortcutState::Pressed {
                    let app = app.clone();
                    tauri::async_runtime::spawn(async move {
                        if let Err(error) = process_hotkey(app.clone()).await {
                            show_error_float(&app, "无法读取选区", friendly_hotkey_error(&error));
                        }
                    });
                }
            })
            .build(),
    )?;

    app.global_shortcut().register(shortcut)?;
    Ok(())
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
        .on_tray_icon_event(|tray, event| match event {
            TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                position,
                ..
            } => {
                toggle_menu_window(tray.app_handle(), position.x as i32, position.y as i32);
            }
            _ => {}
        })
        .build(app)?;

    Ok(())
}

fn tray_template_icon() -> Result<Image<'static>> {
    Image::from_bytes(include_bytes!("../icons/tray-template.png"))
        .map_err(|error| anyhow!("failed to load WordSnap menu bar icon: {error}"))
}

fn setup_window_events(app: &mut tauri::App) {
    for label in ["words", "settings"] {
        if let Some(window) = app.get_webview_window(label) {
            let window_for_event = window.clone();
            window.on_window_event(move |event| {
                if let WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    let _ = window_for_event.hide();
                }
            });
        }
    }

    for label in ["float", "menu"] {
        if let Some(window) = app.get_webview_window(label) {
            let window_for_event = window.clone();
            window.on_window_event(move |event| {
                if let WindowEvent::Focused(false) = event {
                    let _ = window_for_event.hide();
                }
            });
        }
    }
}

async fn process_hotkey(app: AppHandle) -> Result<()> {
    // Anchor the popup to where the selection is *now*, before anything can move
    // the cursor, and reuse it for every later redraw so it never jumps around.
    let anchor = cursor_position_on_main(&app);
    if let Some(state) = app.try_state::<AppState>() {
        if let Ok(mut stored) = state.anchor.lock() {
            *stored = anchor;
        }
    }

    let selected =
        capture_selected_text_on_main(&app).context("failed to capture selected text")?;
    if selected.trim().is_empty() {
        #[cfg(target_os = "macos")]
        let error_msg = "请先选中英文文本，再按 ⌥T。若已选中，请在系统设置中允许 WordSnap 使用辅助功能。";
        #[cfg(not(target_os = "macos"))]
        let error_msg = "请先选中英文文本，再按 Alt+T。";

        show_error_float(
            &app,
            "未读取到文本",
            error_msg,
        );
        return Ok(());
    }

    let state = app.state::<AppState>();
    translate_selection(app.clone(), state.inner(), selected).await?;
    Ok(())
}

async fn translate_selection(
    app: AppHandle,
    state: &AppState,
    selected: String,
) -> Result<FloatPayload> {
    let original = selected.trim().to_string();
    let is_word = is_single_english_word(&original);
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
        .map_err(|_| anyhow!("settings lock poisoned"))?
        .clone();
    let api_key = settings.api_key.trim().to_string();
    if api_key.is_empty() {
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
        Some(upsert_word(state, &original.to_lowercase(), &translated)?)
    } else {
        None
    };

    if is_word {
        let _ = app.emit("words-updated", ());
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
    Ok(payload)
}

fn show_error_float(app: &AppHandle, original: impl Into<String>, error: impl Into<String>) {
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
        return "无法读取当前选中文本。请确认已选中英文，并在系统设置中允许 WordSnap 使用辅助功能。";
        #[cfg(not(target_os = "macos"))]
        return "无法读取当前选中文本。请确认已选中英文。";
    } else {
        "翻译失败，请检查网络或 API 设置。"
    }
}

async fn call_translation_api(
    client: &Client,
    settings: &StoredSettings,
    api_key: &str,
    text: &str,
    is_word: bool,
) -> Result<String> {
    let target_lang = if settings.target_lang.trim().is_empty() {
        StoredSettings::default().target_lang
    } else {
        settings.target_lang.trim().to_string()
    };

    let prompt = if is_word {
        format!(
            "请把下面的英文单词或短语翻译成{}。\n只返回{}中最常见的释义或译法,多个释义用分号分隔。\n不要例句,不要词性,不要解释,不要 Markdown。\n\n文本:\n{}",
            target_lang, target_lang, text
        )
    } else {
        format!(
            "请把下面的英文翻译成{}。\n只返回{}译文,不要解释,不要例句,不要 Markdown。\n\n文本:\n{}",
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
            return Err(anyhow!("{reason}：请检查网络，或确认「模型地址」是否正确可达。"));
        }
    };

    let status = response.status();
    if !status.is_success() {
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

    let raw = response
        .text()
        .await
        .context("无法读取 API 响应内容。")?;
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
        Err(anyhow!("API 返回了空的翻译结果。"))
    } else {
        Ok(trimmed)
    }
}

/// Pulls a readable message out of an OpenAI-style error body
/// (`{"error":{"message":"…"}}`), falling back to the raw (truncated) text so the
/// user sees the actual server reason instead of a generic "translation failed".
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

fn capture_selected_text() -> Result<String> {
    let mut clipboard = Clipboard::new().context("failed to open clipboard")?;
    let previous = clipboard.get_text().ok();

    let mut enigo = Enigo::new(&EnigoSettings::default())
        .map_err(|error| anyhow!("failed to initialize input simulator: {error:?}"))?;

    #[cfg(target_os = "macos")]
    let modifier = Key::Meta;
    #[cfg(not(target_os = "macos"))]
    let modifier = Key::Control;

    #[cfg(target_os = "macos")]
    let key_c = Key::Unicode('c');
    #[cfg(target_os = "windows")]
    let key_c = Key::Other(0x43); // VK_C
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let key_c = Key::Unicode('c');

    enigo.key(modifier, Press).map_err(enigo_err)?;
    enigo.key(key_c, Click).map_err(enigo_err)?;
    enigo.key(modifier, Release).map_err(enigo_err)?;
    thread::sleep(Duration::from_millis(140));

    let selected = clipboard.get_text().unwrap_or_default();
    if let Some(previous_text) = previous {
        let _ = clipboard.set_text(previous_text);
    }

    Ok(selected)
}

fn capture_selected_text_on_main(app: &AppHandle) -> Result<String> {
    let (tx, rx) = mpsc::channel();

    app.run_on_main_thread(move || {
        let result = capture_selected_text();
        let _ = tx.send(result);
    })
    .context("failed to schedule selection capture on main thread")?;

    rx.recv_timeout(Duration::from_secs(3))
        .context("timed out while capturing selected text on main thread")?
}

fn cursor_position() -> (i32, i32) {
    let enigo = Enigo::new(&EnigoSettings::default());
    enigo
        .ok()
        .and_then(|input| input.location().ok())
        .unwrap_or((620, 260))
}

/// Reads the cursor location on the main thread. On macOS the input APIs must be
/// touched from the main thread, so calling `cursor_position()` from a spawned
/// task can silently return the fallback and drop the popup in the wrong place.
fn cursor_position_on_main(app: &AppHandle) -> (i32, i32) {
    let (tx, rx) = mpsc::channel();
    if app
        .run_on_main_thread(move || {
            let _ = tx.send(cursor_position());
        })
        .is_ok()
    {
        if let Ok(pos) = rx.recv_timeout(Duration::from_secs(1)) {
            return pos;
        }
    }
    cursor_position()
}

fn set_float_payload(app: &AppHandle, payload: FloatPayload, width: u32, height: u32) {
    if let Some(state) = app.try_state::<AppState>() {
        if let Ok(mut current) = state.float.lock() {
            *current = payload.clone();
        }
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
        let _ = window.set_size(LogicalSize::new(width as f64, height as f64));
        let _ = window.set_position(LogicalPosition::new(x, y));
        let _ = window.emit("float-updated", payload);
        let _ = window.show();
        let _ = window.set_focus();
    }
}

/// Places the float just below and slightly left of the selection/cursor,
/// keeping the whole card within the monitor it lands on. All maths is in
/// logical points so it stays correct on Retina displays.
fn float_origin(
    window: &tauri::WebviewWindow,
    cursor_x: f64,
    cursor_y: f64,
    width: f64,
    height: f64,
) -> (f64, f64) {
    // Nudge left of the cursor and drop below it so the popup doesn't cover the
    // text the user just selected.
    let mut x = cursor_x - 24.0;
    let mut y = cursor_y + 20.0;

    // Clamp to the monitor the *selection* is on (not wherever the float window
    // happens to sit), so a selection on a secondary display stays on it.
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

/// Logical bounds `(left, top, right, bottom)` of the monitor containing the
/// given logical point, falling back to the current/primary monitor. Monitor
/// geometry is physical, so it is divided by each monitor's own scale factor to
/// match the logical-point coordinate space the cursor is reported in.
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
        if window.is_visible().unwrap_or(false) {
            let _ = window.hide();
            return;
        }

        let scale_factor = window.scale_factor().unwrap_or(1.0);

        // Find the monitor containing the tray icon click position
        let monitor = if let Ok(monitors) = window.available_monitors() {
            monitors.into_iter().find(|m| {
                let pos = m.position();
                let size = m.size();
                tray_x >= pos.x && tray_x < pos.x + size.width as i32 &&
                tray_y >= pos.y && tray_y < pos.y + size.height as i32
            })
        } else {
            None
        }.or_else(|| {
            window.current_monitor().ok().flatten()
        }).or_else(|| {
            window.primary_monitor().ok().flatten()
        });

        if let Some(monitor) = monitor {
            let work_area = monitor.work_area();
            let window_size = window.outer_size().unwrap_or_default();
            let w = window_size.width as i32;
            let h = window_size.height as i32;

            // Center the window horizontally on the tray icon click position.
            let mut x = tray_x - w / 2;

            // Determine if the taskbar is at the top or bottom of the screen.
            let monitor_center_y = work_area.position.y + (work_area.size.height as i32) / 2;
            let gap = (8.0 * scale_factor) as i32;
            let mut y = if tray_y > monitor_center_y {
                // Taskbar is near the bottom: pop the window above the tray icon
                tray_y - h - gap
            } else {
                // Taskbar is near the top: drop the window below the tray icon
                tray_y + gap
            };

            // Clamp both coordinates to ensure the entire window stays within the work area.
            x = x.clamp(work_area.position.x, work_area.position.x + work_area.size.width as i32 - w);
            y = y.clamp(work_area.position.y, work_area.position.y + work_area.size.height as i32 - h);

            let _ = window.set_position(PhysicalPosition::new(x, y));
        } else {
            // Fallback to legacy static offsets if monitor info is unavailable.
            let x = tray_x.saturating_sub(196);
            let y = tray_y.saturating_add(24);
            let _ = window.set_position(PhysicalPosition::new(x, y));
        }

        let _ = window.show();
        let _ = window.set_focus();
    }
}

fn hide_menu_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("menu") {
        let _ = window.hide();
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
    Ok(WordListPayload { total, words })
}

fn upsert_word(state: &AppState, word: &str, translation: &str) -> Result<i64> {
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

    Ok(count)
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

fn load_settings(app_dir: &PathBuf) -> Result<StoredSettings> {
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

fn save_settings_file(app_dir: &PathBuf, settings: &StoredSettings) -> Result<()> {
    fs::create_dir_all(app_dir)?;
    let raw = serde_json::to_string_pretty(settings)?;
    let path = app_dir.join("settings.json");
    fs::write(&path, raw)?;
    // The file holds the API key in plain text, so lock it down to the owner.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
    }
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
