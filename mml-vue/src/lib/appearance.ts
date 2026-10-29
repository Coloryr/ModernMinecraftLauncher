// 界面外观增强：
// - 动画开关：全局禁用 CSS 过渡 / 动画（前端自持，本地存储）
// - 背景图：来源与显示参数存 gui_config.json（Rust 侧持久化），处理后的图片
//   由后端落盘并经 IPC 下发 dataURL，改动经 bg-change 事件同步到所有窗口；
//   浏览器环境回退本地存储，仅 dataURL 直显，无缩放处理
import { computed, ref, watch } from "vue";
import { listen } from "@tauri-apps/api/event";
import { isTauri } from "./util";
import { commands } from "./bindings";
import type { ClientConfigDto } from "./bindings";
import { saveGuiConfig } from "./guiConfig";
import { KEYS, readFlag, readNumber, readString, writeFlag, writeString } from "./storage";
import { BgChange, ClientConfigChange } from "./listens";
import { t } from "./i18n";
import { showToast } from "./toast";

// ---------------- 动画 ----------------

export const animations = ref(readFlag(KEYS.animations, true));

/**
 * 开关全局动画
 *
 * - `v`: 是否启用动画
 */
export function setAnimations(v: boolean) {
  animations.value = v;
  writeFlag(KEYS.animations, v);
  document.documentElement.classList.toggle("no-anim", !v);
}

/** 启动恢复：只同步内存与 DOM，不回写本地存储 */
export function applyAnimations() {
  document.documentElement.classList.toggle("no-anim", !animations.value);
}

// ---------------- 背景图 ----------------

/** 显示用背景图 dataURL；Tauri 下是后端处理落盘的图 */
export const bgImage = ref(readString(KEYS.bgImage));

/** 背景图来源：文件路径或网址，空串 = 无；与 gui_config.json 的 bgSource 一致 */
export const bgSource = ref("");

/** 背景图层不透明度，单位 %：默认 100 即完整显示，越高图越显 */
export const bgOpacity = ref(readNumber(KEYS.bgOpacity, 100));

/** 背景模糊，单位 px */
export const bgBlur = ref(readNumber(KEYS.bgBlur, 0));

/** 原始分辨率，单位 %，范围 10–100：后端把源图缩放到源图的百分之多少 */
export const bgNativeSize = ref(100);

/** 后端正在加载 / 处理背景图（加载提示与按钮禁用用） */
export const bgLoading = ref(false);

/** 后端返回的背景图状态 */
interface BgInfo {
  /** 来源：文件路径或网址，空串表示无 */
  source: string;
  /** 处理后的 dataURL，空串表示无 */
  dataUrl: string;
  /** 图层不透明度，单位 % */
  opacity: number;
  /** 模糊半径，单位 px */
  blur: number;
  /** 原始分辨率，单位 % */
  nativeSize: number;
}

/** 用后端返回的信息更新本地状态，restoreBg 与 bg-change 共用 */
function applyBgInfo(info: BgInfo) {
  bgSource.value = info.source;
  bgImage.value = info.dataUrl;
  bgOpacity.value = info.opacity;
  bgBlur.value = info.blur;
  bgNativeSize.value = info.nativeSize;
  // 缓存一份 dataURL：下次启动挂载时立即有背景，不闪底色。
  // dataURL 很大，配额写满时会静默失败，仅当次会话生效，见 storage.writeRaw
  writeString(KEYS.bgImage, info.dataUrl);
}

/** 清掉本窗口的背景状态（不影响后端配置） */
function clearLocalBg() {
  bgSource.value = "";
  bgImage.value = "";
  writeString(KEYS.bgImage, "");
}

/** 启动恢复 / bg-change 刷新：Tauri 下背景图以后端配置为准 */
export async function restoreBg() {
  if (!isTauri()) return;
  try {
    const info = await commands.settings.getBg();
    if (info) applyBgInfo(info);
    else clearLocalBg();
  } catch {
    /* 后端未就绪：保持现状 */
  }
}

/**
 * 设置背景图
 *
 * - `v`: 直接给 dataURL 的浏览器回退路径；空串表示清除，Tauri 下同时删掉后端落盘的图与配置里的来源
 */
export async function setBgImage(v: string) {
  if (v) {
    // 直接给 dataURL（浏览器回退路径）
    bgImage.value = v;
    writeString(KEYS.bgImage, v);
    return;
  }
  clearLocalBg();
  if (isTauri()) {
    try {
      await commands.settings.clearBg();
    } catch {
      /* 忽略 */
    }
  }
}

/**
 * 设置背景图来源
 *
 * Tauri 下由后端加载、按当前原始分辨率缩放、落盘并记入配置；
 * 浏览器模式下把 `source` 当 dataURL 直接显示。
 *
 * - `source`: 文件路径或网址
 */
export async function setBgImageFromOriginal(source: string) {
  if (!isTauri()) {
    bgImage.value = source;
    writeString(KEYS.bgImage, source);
    return;
  }
  bgLoading.value = true;
  try {
    const dataUrl = await commands.settings.setBg(source, bgNativeSize.value);
    bgSource.value = source;
    bgImage.value = dataUrl;
    // 与 applyBgInfo 一样缓存一份：下次启动首帧就有背景，不用等后端
    writeString(KEYS.bgImage, dataUrl);
  } catch (err) {
    // 带上后端的具体错误，方便定位加载失败的原因
    showToast(`${t("winSettings.bgLoadFailed")}${t("common.detailSep")}${String(err)}`);
  } finally {
    bgLoading.value = false;
  }
}

/**
 * 调整背景图的原始分辨率，点「应用」后调用
 *
 * 后端用缓存的原图重新缩放，不重新下载，随机图网址不会换图。
 * 只改图片分辨率，不改变图在窗口里的显示，显示始终铺满居中。
 *
 * - `percent`: 原始分辨率百分比，会夹到 10–100
 */
export async function resizeBg(percent: number) {
  bgNativeSize.value = Math.min(Math.max(Math.round(percent), 10), 100);
  if (!isTauri() || !bgSource.value) return;
  bgLoading.value = true;
  try {
    const dataUrl = await commands.settings.resizeBg(bgNativeSize.value);
    bgImage.value = dataUrl;
    writeString(KEYS.bgImage, dataUrl);
  } catch (err) {
    showToast(`${t("winSettings.bgResizeFailed")}${t("common.detailSep")}${String(err)}`);
  } finally {
    bgLoading.value = false;
  }
}

/**
 * 设置背景图层不透明度
 *
 * - `v`: 不透明度，单位 %
 */
export function setBgOpacity(v: number) {
  bgOpacity.value = v;
  writeString(KEYS.bgOpacity, String(v));
  void saveGuiConfig({ bgOpacity: v });
}

/**
 * 设置背景模糊
 *
 * - `v`: 模糊半径，单位 px
 */
export function setBgBlur(v: number) {
  bgBlur.value = v;
  writeString(KEYS.bgBlur, String(v));
  void saveGuiConfig({ bgBlur: v });
}

/**
 * 背景图层的内联样式，App.vue 的 .app-bg 绑定用
 *
 * 始终铺满窗口并居中；标题栏与各表面一样半透明透出这层，见 themes.css 的 has-bg。
 */
export const bgLayerStyle = computed(() => {
  if (!bgImage.value) return undefined;
  return {
    backgroundImage: `url("${bgImage.value}")`,
    opacity: String(bgOpacity.value / 100),
    filter: `blur(${bgBlur.value}px)`,
  };
});

// 有背景图时给 <html> 挂 has-bg：themes.css 里的半透明表面变量生效
watch(bgImage, (v) => {
  document.documentElement.classList.toggle("has-bg", !!v);
}, { immediate: true });

// 背景图变更（设置窗口改动 / 清除）时所有窗口统一向后端取最新状态
if (isTauri()) {
  void listen(BgChange, () => {
    void restoreBg();
  });
}

// ---------------- 窗口透明度 ----------------

/**
 * 窗口透明度写进 CSS 变量
 *
 * 材质是系统在窗口背后合成的，底色越不透明越看不见它，见 base.css 的 body。
 *
 * - `v`: 不透明度百分比，60–100
 */
export function applyWindowAlpha(v: number) {
  document.documentElement.style.setProperty("--window-alpha", `${v}%`);
}

// 材质由壳层就地重铺所有窗口（见 windows/effects.rs），透明度与"表面透不透"是各窗口自己的
// 样式：不跟着广播换，就只有设置窗口会变，主窗口要等下次启动
if (isTauri()) {
  void listen<ClientConfigDto>(ClientConfigChange, (e) => {
    applyWindowAlpha(e.payload.windowOpacity);
    applyWindowMaterial(e.payload.windowEffect);
  });
}

// ---------------- 窗口材质 ----------------

// 材质由系统在窗口背后合成：Linux 上没人给它做模糊（见 Rust 侧 windows/effects.rs），
// 表面调半透明只会变成"透出桌面"，那就保持不透明
const MATERIAL_PLATFORM = /Windows|Macintosh/.test(navigator.userAgent);

/**
 * 窗口材质开关写进 `<html class="has-material">`
 *
 * 材质躲在窗口背后，表面不透明就只在卡片之间的缝里透出来，看着像没生效，
 * 所以材质开着时各表面转半透明，见 themes.css。
 *
 * - `effect`: 窗口材质，`none` / `auto` / `mica` / `acrylic` / `blur`
 */
export function applyWindowMaterial(effect: string) {
  document.documentElement.classList.toggle("has-material", MATERIAL_PLATFORM && effect !== "none");
}
