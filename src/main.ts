import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

import "./styles.css";

type FloatPayload = {
  state: "idle" | "loading" | "word" | "sentence" | "error";
  original: string;
  translation: string | null;
  isWord: boolean;
  count: number | null;
  error: string | null;
};

type WordRecord = {
  word: string;
  translation: string;
  count: number;
  firstSeenAt: string;
  lastSeenAt: string;
  recent: string;
};

type WordListPayload = {
  total: number;
  words: WordRecord[];
};

type SettingsPayload = {
  baseUrl: string;
  model: string;
  hotkey: string;
  targetLang: string;
  apiKeySet: boolean;
  apiKeyPreview: string | null;
};

const TARGET_LANGUAGES = [
  "简体中文",
  "繁体中文",
  "英语",
  "日语",
  "韩语",
  "法语",
  "德语",
  "西班牙语",
  "葡萄牙语",
  "意大利语",
  "俄语",
  "荷兰语",
  "阿拉伯语",
  "印地语",
  "越南语",
  "泰语",
  "印度尼西亚语",
  "土耳其语",
];

const app = document.querySelector<HTMLDivElement>("#app");
if (!app) {
  throw new Error("Missing #app root");
}

type DesktopOS = "macos" | "windows" | "linux";

const os: DesktopOS = navigator.userAgent.includes("Windows")
  ? "windows"
  : navigator.userAgent.includes("Mac")
    ? "macos"
    : "linux";
document.body.classList.add(`is-${os}`);

const view = new URLSearchParams(window.location.search).get("view") ?? "words";
document.body.dataset.view = view;

switch (view) {
  case "float":
    mountFloat(app);
    break;
  case "settings":
    mountSettings(app);
    break;
  case "menu":
    mountMenu(app);
    break;
  case "words":
  default:
    mountWords(app);
    break;
}

function mountFloat(root: HTMLElement) {
  root.className = "float-root";
  root.innerHTML = `<div class="float-view" id="float-view"></div>`;
  const container = query<HTMLDivElement>("#float-view");

  const load = async () => {
    const payload = await command<FloatPayload>("current_float");
    renderFloat(container, payload);
  };

  listen<FloatPayload>("float-updated", (event) => {
    renderFloat(container, event.payload);
  });

  window.addEventListener("keydown", (event) => {
    if (event.key === "Escape") {
      command("hide_float");
    }
  });

  load();
}

function renderFloat(container: HTMLElement, payload: FloatPayload) {
  const original = escapeHtml(payload.original || "WordSnap");
  const translation = escapeHtml(payload.translation ?? "");

  if (payload.state === "loading") {
    container.innerHTML = `
      <section class="float-card float-card--loading">
        <div class="float-word float-clamp">${original}</div>
        <div class="float-loading-row">
          <span class="spinner"></span>
          <span>翻译中…</span>
        </div>
      </section>
    `;
    fitFloatWindow(container);
    return;
  }

  if (payload.state === "error") {
    const message = payload.error ?? "翻译失败,请检查网络或 API 设置。";
    // A missing API key can't be fixed by retrying — send the user to Settings.
    const needsSettings = message.includes("API Key");
    const hint = needsSettings ? "点按前往设置" : "未记录 · 点按重试";
    container.innerHTML = `
      <button class="float-card float-card--error" type="button" id="retry">
        <div class="float-word float-clamp">${original}</div>
        <div class="float-scroll">
          <div class="float-error-row">
            <span class="error-dot">!</span>
            <span>${escapeHtml(message)}</span>
          </div>
        </div>
        <div class="float-footer float-footer--muted">${hint}</div>
      </button>
    `;
    query<HTMLButtonElement>("#retry", container).addEventListener("click", () => {
      command(needsSettings ? "show_settings" : "retry_translation").catch(() => undefined);
    });
    fitFloatWindow(container);
    return;
  }

  if (payload.state === "sentence") {
    container.innerHTML = `
      <section class="float-card">
        <div class="float-scroll">
          <div class="float-source">${original}</div>
          <div class="float-translation float-translation--sentence">${translation}</div>
        </div>
        <div class="float-footer">
          <span class="skip-mark"><span></span></span>
          <span>整句翻译 · 未记入单词本</span>
        </div>
      </section>
    `;
    fitFloatWindow(container);
    return;
  }

  if (payload.state === "word") {
    const count = payload.count ?? 1;
    container.innerHTML = `
      <section class="float-card">
        <div class="float-word">${original}</div>
        <div class="float-scroll">
          <div class="float-translation">${translation}</div>
        </div>
        <div class="float-footer float-footer--recorded">
          <span class="record-mark">✓</span>
          <span>已记入词表 · 第 ${count} 次</span>
        </div>
      </section>
    `;
    fitFloatWindow(container);
    return;
  }

  container.innerHTML = "";
  fitFloatWindow(container);
}

function fitFloatWindow(container: HTMLElement) {
  requestAnimationFrame(() => {
    const card = container.querySelector<HTMLElement>(".float-card");
    if (!card) {
      return;
    }

    // Width is fixed by the backend per state; only refine the height. The card
    // is capped at the window height with the overflow inside `.float-scroll`,
    // so ask for the card's visible height plus whatever the scroll area hides.
    // The backend clamps the result; anything beyond the clamp stays scrollable.
    let height = card.getBoundingClientRect().height;
    const scroll = card.querySelector<HTMLElement>(".float-scroll");
    if (scroll) {
      height += Math.max(0, scroll.scrollHeight - scroll.getBoundingClientRect().height);
    }
    const width = Math.round(window.innerWidth);
    command("resize_float", { width, height: Math.ceil(height) }).catch(() => undefined);
  });
}

async function mountWords(root: HTMLElement) {
  root.className = "window-root words-root";
  root.innerHTML = `
    <main class="words-window">
      <header class="win-titlebar" data-tauri-drag-region>词表</header>
      <div class="words-head">
        <span>单词</span>
        <span>译文</span>
        <span>次数</span>
        <span>最近</span>
      </div>
      <div class="word-rows" id="word-rows"></div>
      <footer class="words-stat" id="words-stat">共 0 个单词</footer>
    </main>
  `;

  await refreshWords();
  listen("words-updated", () => {
    refreshWords();
  });
}

async function refreshWords() {
  const payload = await command<WordListPayload>("list_words");
  const rows = query<HTMLDivElement>("#word-rows");
  rows.innerHTML = payload.words
    .map(
      (word, index) => `
        <div class="word-row ${index === 0 ? "is-selected" : ""}">
          <span title="${escapeAttr(word.word)}">${escapeHtml(word.word)}</span>
          <span title="${escapeAttr(word.translation)}">${escapeHtml(word.translation)}</span>
          <span>${word.count}</span>
          <span>${escapeHtml(word.recent)}</span>
        </div>
      `,
    )
    .join("");
  query<HTMLElement>("#words-stat").textContent = `共 ${payload.total} 个单词`;
}

async function mountSettings(root: HTMLElement) {
  root.className = "window-root settings-root";
  root.innerHTML = `
    <main class="settings-window">
      <header class="win-titlebar" data-tauri-drag-region>设置</header>
      <form id="settings-form" class="settings-body">
        <p class="group-caption">翻译服务</p>
        <section class="settings-group">
          <label class="settings-row">
            <span>API Key</span>
            <input id="api-key" type="password" autocomplete="off" spellcheck="false" placeholder="sk-…" />
          </label>
          <label class="settings-row">
            <span>模型地址</span>
            <span class="field-stack">
              <input id="base-url" class="mono-input" spellcheck="false" placeholder="api.openai.com/v1" />
              <small>示例: api.openai.com/v1, 后续 /chat/completions 会自动补全。</small>
            </span>
          </label>
          <label class="settings-row">
            <span>模型</span>
            <input id="model" class="mono-input model-input" list="model-presets" spellcheck="false" placeholder="gpt-4o-mini" />
            <datalist id="model-presets">
              <option value="gpt-4o-mini"></option>
              <option value="gpt-4o"></option>
              <option value="gpt-4.1-mini"></option>
              <option value="gpt-4.1"></option>
              <option value="o4-mini"></option>
              <option value="o3-mini"></option>
              <option value="deepseek-chat"></option>
              <option value="deepseek-reasoner"></option>
              <option value="qwen-plus"></option>
              <option value="qwen-turbo"></option>
            </datalist>
          </label>
        </section>
        <p class="group-caption">偏好</p>
        <section class="settings-group">
          <div class="settings-row">
            <span>翻译快捷键</span>
            <span class="kbd-pair"><kbd>⌥</kbd><kbd>T</kbd></span>
          </div>
          <label class="settings-row">
            <span>目标语言</span>
            <span class="select-wrap">
              <select id="target-lang">
                ${TARGET_LANGUAGES.map((lang) => `<option value="${escapeAttr(lang)}">${escapeHtml(lang)}</option>`).join("")}
              </select>
            </span>
          </label>
        </section>
        <div class="settings-actions">
          <button class="primary-button" type="submit">完成</button>
        </div>
      </form>
    </main>
  `;

  const form = query<HTMLFormElement>("#settings-form");
  const doneButton = query<HTMLButtonElement>(".primary-button", form);
  const apiInput = query<HTMLInputElement>("#api-key");
  const baseInput = query<HTMLInputElement>("#base-url");
  const modelInput = query<HTMLInputElement>("#model");
  const langSelect = query<HTMLSelectElement>("#target-lang");

  // Re-read persisted settings and clear any transient state so reopening the
  // window never shows a stale "保存失败" or half-edited form.
  const load = async () => {
    const settings = await command<SettingsPayload>("get_settings");
    apiInput.value = "";
    apiInput.placeholder = settings.apiKeyPreview ?? "sk-…";
    baseInput.value = displayBaseUrl(settings.baseUrl);
    modelInput.value = settings.model;
    ensureSelectOption(langSelect, settings.targetLang);
    langSelect.value = settings.targetLang;
    doneButton.textContent = "完成";
    doneButton.disabled = false;
  };

  form.addEventListener("submit", async (event) => {
    event.preventDefault();
    doneButton.disabled = true;
    doneButton.textContent = "保存中…";
    try {
      const apiKey = apiInput.value;
      await command<SettingsPayload>("save_settings", {
        request: {
          apiKey: apiKey.trim() ? apiKey : null,
          baseUrl: baseInput.value,
          model: modelInput.value,
          hotkey: "Alt+T",
          targetLang: langSelect.value,
        },
      });
      // Close the window so clicking 完成 gives an obvious result; the form is
      // reset by `load()` the next time it opens.
      doneButton.textContent = "完成";
      doneButton.disabled = false;
      await command("hide_settings");
    } catch (error) {
      doneButton.textContent = "保存失败，重试";
      doneButton.disabled = false;
    }
  });

  listen("settings-refresh", () => {
    load();
  });

  await load();
}

function mountMenu(root: HTMLElement) {
  const modifier = os === "macos" ? "⌘" : "Ctrl+";
  const hotkey = os === "macos" ? "⌥T" : "Alt+T";

  root.className = "menu-root";
  root.innerHTML = `
    <nav class="tray-menu">
      <header class="tray-head">
        <span class="brand-icon"><span></span></span>
        <span class="tray-title">
          <strong>WordSnap</strong>
          <small>已就绪 · ${hotkey} 翻译</small>
        </span>
      </header>
      <div class="menu-separator"></div>
      <button class="menu-item" type="button" id="open-words">
        <span>打开词表</span><span>${modifier}L</span>
      </button>
      <button class="menu-item" type="button" id="open-settings">
        <span>设置…</span><span>${modifier},</span>
      </button>
      <div class="menu-separator"></div>
      <button class="menu-item" type="button" id="quit">
        <span>退出 WordSnap</span><span>${modifier}Q</span>
      </button>
    </nav>
  `;

  query("#open-words").addEventListener("click", () => command("show_words"));
  query("#open-settings").addEventListener("click", () => command("show_settings"));
  query("#quit").addEventListener("click", () => command("quit_app"));

  // Make the displayed shortcuts real while the menu has focus.
  window.addEventListener("keydown", (event) => {
    if (event.key === "Escape") {
      command("hide_menu");
      return;
    }
    const withModifier = os === "macos" ? event.metaKey : event.ctrlKey;
    if (!withModifier) {
      return;
    }
    if (event.key === "l" || event.key === "L") {
      event.preventDefault();
      command("show_words");
    } else if (event.key === ",") {
      event.preventDefault();
      command("show_settings");
    } else if (event.key === "q" || event.key === "Q") {
      event.preventDefault();
      command("quit_app");
    }
  });
}

async function command<T>(name: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(name, args);
  } catch (error) {
    if (isMissingTauriBridge(error)) {
      return mockCommand<T>(name, args);
    }
    throw error;
  }
}

function isMissingTauriBridge(error: unknown) {
  const message = String(error);
  return message.includes("TAURI") || message.includes("reading 'invoke'");
}

function mockCommand<T>(name: string, args?: Record<string, unknown>): T {
  if (name === "current_float" || name === "retry_translation") {
    return {
      state: "word",
      original: "manifest",
      translation: "清单;显示;表明",
      isWord: true,
      count: 3,
      error: null,
    } as T;
  }

  if (name === "list_words") {
    return {
      total: 128,
      words: [
        { word: "manifest", translation: "清单;显示;表明", count: 3, firstSeenAt: "", lastSeenAt: "", recent: "11:05" },
        { word: "ephemeral", translation: "短暂的;瞬息的", count: 2, firstSeenAt: "", lastSeenAt: "", recent: "09:14" },
        { word: "resilient", translation: "有弹性的;能复原的", count: 5, firstSeenAt: "", lastSeenAt: "", recent: "昨天" },
        { word: "throughput", translation: "吞吐量", count: 1, firstSeenAt: "", lastSeenAt: "", recent: "昨天" },
        { word: "ambiguous", translation: "模糊的;含糊的", count: 4, firstSeenAt: "", lastSeenAt: "", recent: "6/29" },
        { word: "concurrency", translation: "并发", count: 2, firstSeenAt: "", lastSeenAt: "", recent: "6/28" },
      ],
    } as T;
  }

  if (name === "get_settings" || name === "save_settings") {
    const request = args?.request as Partial<SettingsPayload> | undefined;
    return {
      baseUrl: request?.baseUrl ?? "https://api.openai.com/v1",
      model: request?.model ?? "gpt-4o-mini",
      hotkey: "Alt+T",
      targetLang: request?.targetLang ?? "简体中文",
      apiKeySet: true,
      apiKeyPreview: "•••••••••••• 3f2a",
    } as T;
  }

  return undefined as T;
}

function query<T extends Element>(selector: string, root: ParentNode = document): T {
  const element = root.querySelector<T>(selector);
  if (!element) {
    throw new Error(`Missing element: ${selector}`);
  }
  return element;
}

function displayBaseUrl(value: string) {
  return value.startsWith("https://") ? value.replace(/^https:\/\//, "") : value;
}

function ensureSelectOption(select: HTMLSelectElement, value: string) {
  if (!value || Array.from(select.options).some((option) => option.value === value)) {
    return;
  }
  select.add(new Option(value, value));
}

function escapeHtml(value: string) {
  return value
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&#039;");
}

function escapeAttr(value: string) {
  return escapeHtml(value).replaceAll("`", "&#096;");
}
