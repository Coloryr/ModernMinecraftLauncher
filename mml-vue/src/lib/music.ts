// 自定义音乐：配置里的那一份来源（本地文件或 http(s) 直链）交给一个 <audio> 播放，
// 标题栏那条 MusicTitleIndicator 就是它的播放 / 暂停 / 停止入口。
//
// **只让一个文档持有播放器**：多窗口模式下每个窗口都是独立 webview，各播各的会叠成
// 好几层声音，所以只有主窗口持有（单窗口模式只有一个文档，谁持有都一样）。
// 本地文件经 `mml-music` 自定义协议取，见 src-tauri 的 music.rs；直链直接把原地址给 <audio>。
//
// 配置变了走 `client-config-change` 事件（设置窗口与主窗口是两个 webview），
// 所以改完立刻换歌 / 停播，不必重启。
import { computed, ref } from "vue";
import { getMusicBaseUrl, onClientConfigChange } from "./api";
import { loadGuiConfig } from "./guiConfig";
import { currentKind, multiWindow } from "../windows/windowManager";
/** 播放器只关心这两项：前端自己的 ClientConfig 与 binding 的 ClientConfigDto 都有 */
export interface MusicConfig {
  /** 是否启用 */
  musicOn: boolean;
  /** 来源：本地路径或 http(s) 直链 */
  musicSource: string;
}

/** 播放状态：没在播 / 播放中 / 暂停 */
export type MusicState = "idle" | "playing" | "paused";

/** 本窗口是不是那个持有播放器的窗口，见文件头；启动时定一次，之后不变 */
const OWNER = !multiWindow.value || currentKind.value === "main";

/** 播放状态 */
const state = ref<MusicState>("idle");
/** 当前来源：配置里那一份，已 trim */
const source = ref("");
/** `mml-music` 协议前缀，本地文件才用得上，取一次就缓存 */
let baseUrl = "";
/** 已经解析好、准备交给 `<audio>` 的地址；换来源时清掉重算 */
let resolved = "";
/** 上一次配置里音乐是不是开着：用来区分"刚打开开关"与"配置没变" */
let wasOn = false;
/** 播放器：真的要播时才建，没配音乐的窗口一个 DOM 都不多 */
let audio: HTMLAudioElement | null = null;

/** 播放地址：本地文件走协议（带时间戳，换文件不吃浏览器缓存），直链原样用 */
function srcOf(raw: string): string {
  if (/^https?:\/\//i.test(raw)) return raw;
  return `${baseUrl}?v=${Date.now()}`;
}

/** 建播放器（只建一次）并把它的事件接回状态 */
function ensureAudio(): HTMLAudioElement | null {
  if (!OWNER) return null;
  if (!audio) {
    audio = new Audio();
    // 背景音乐：循环放，直到用户停或退出
    audio.loop = true;
    audio.preload = "none";
    audio.addEventListener("playing", () => {
      state.value = "playing";
    });
    audio.addEventListener("pause", () => {
      if (state.value === "playing") state.value = "paused";
    });
    audio.addEventListener("error", () => {
      state.value = "idle";
    });
  }
  return audio;
}

/** 播放；浏览器拦自动播放时不抛给外面，落回未播放态等用户点标题栏那枚按钮 */
export async function playMusic(): Promise<void> {
  const raw = source.value;
  if (!OWNER || !raw) return;
  const el = ensureAudio();
  if (!el) return;

  // 地址只解析一次：暂停之后继续播放不重设 src，否则会从头开始
  if (!resolved) {
    if (!/^https?:\/\//i.test(raw) && !baseUrl) {
      try {
        baseUrl = await getMusicBaseUrl();
      } catch {
        return;
      }
    }
    resolved = srcOf(raw);
  }
  if (el.src !== resolved) el.src = resolved;

  try {
    await el.play();
  } catch {
    // 自动播放要用户先给一次手势：停在未播放态，标题栏那枚按钮就是入口
    state.value = "idle";
  }
}

/** 暂停，保留进度 */
export function pauseMusic(): void {
  audio?.pause();
}

/** 停止并回到开头 */
export function stopMusic(): void {
  if (!audio) {
    state.value = "idle";
    return;
  }
  audio.pause();
  audio.currentTime = 0;
  state.value = "idle";
}

/** 在播就暂停，否则播放：标题栏那枚按钮用 */
export function toggleMusic(): void {
  if (state.value === "playing") pauseMusic();
  else void playMusic();
}

/**
 * 应用客户端设置里的自定义音乐
 *
 * 关掉或清空来源就停；换了来源重新起播；来源没变而当前没在播（自动播放被拦）不自动重试，
 * 等用户点标题栏那枚按钮。
 *
 * - `cfg`: 客户端设置
 */
export function applyMusicConfig(cfg: MusicConfig): void {
  const target = (cfg.musicSource ?? "").trim();
  const changed = target !== source.value;
  source.value = target;
  // 换了来源：地址重算（带新时间戳，不吃旧文件的缓存）
  if (changed) resolved = "";

  if (!cfg.musicOn || !target) {
    stopMusic();
    wasOn = false;
    return;
  }
  // 刚打开开关、或换了来源：起播；配置没动的那次事件不打断当前播放
  if (changed || !wasOn) void playMusic();
  wasOn = true;
}

/** 启动时读一遍配置并订阅变更 */
export async function initMusic(): Promise<() => void> {
  try {
    const cfg = await loadGuiConfig();
    if (cfg) applyMusicConfig(cfg.client);
  } catch {
    // 读不到配置就按"没配音乐"处理
  }
  return onClientConfigChange((cfg) => applyMusicConfig(cfg));
}

/** 给标题栏那枚按钮用的一组状态 */
export function useMusic() {
  return {
    /** 本窗口要不要显示这枚按钮：持有播放器且配了来源 */
    available: computed(() => OWNER && !!source.value),
    /** 是不是正在播 */
    playing: computed(() => state.value === "playing"),
    /** 有没有在播 / 暂停，即要不要给"停止" */
    active: computed(() => state.value !== "idle"),
    play: () => void playMusic(),
    pause: pauseMusic,
    stop: stopMusic,
    toggle: toggleMusic,
  };
}
