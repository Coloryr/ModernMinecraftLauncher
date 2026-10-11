// 界面外观增强：
// - 动画开关：全局禁用 CSS 过渡 / 动画（前端自持，本地存储）
// - 背景图：来源与显示参数存 gui_config.json（Rust 侧持久化），处理后的图片
//   由后端落盘并经 IPC 下发 dataURL，改动经 bg-change 事件同步到所有窗口；
//   浏览器环境回退本地存储（仅 dataURL 直显，无缩放处理）
import { computed, ref, watch } from "vue";
import { listen } from "@tauri-apps/api/event";
import { isTauri } from "../windows/windowManager";
import { commands } from "./bindings";
import { saveGuiConfig } from "./guiConfig";
import { KEYS, readFlag, readNumber, readString, writeFlag, writeString } from "./storage";
import { BgChange } from "./listens";
import { t } from "./i18n";
import { showToast } from "./toast";

// ---------------- 动画 ----------------

export const animations = ref(readFlag(KEYS.animations, true));

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

/** 显示用背景图 dataURL（Tauri 下是后端处理落盘的图） */
export const bgImage = ref(readString(KEYS.bgImage));

/** 背景图来源（文件路径 / 网址，空串 = 无；与 gui_config.json 的 bgSource 一致） */
export const bgSource = ref("");

/** 背景图层不透明度，单位 %：默认 100 即完整显示，越高图越显 */
export const bgOpacity = ref(readNumber(KEYS.bgOpacity, 100));

/** 背景模糊（px） */
export const bgBlur = ref(readNumber(KEYS.bgBlur, 0));

/** 原始分辨率，单位 %，范围 10–100：后端把源图缩放到源图的百分之多少 */
export const bgNativeSize = ref(100);

/** 后端正在加载 / 处理背景图（加载提示与按钮禁用用） */
export const bgLoading = ref(false);

/** 用后端返回的信息更新本地状态（restoreBg / bg-change 共用） */
function applyBgInfo(info: {
  source: string;
  dataUrl: string;
  opacity: number;
  blur: number;
  nativeSize: number;
}) {
  bgSource.value = info.source;
  bgImage.value = info.dataUrl;
  bgOpacity.value = info.opacity;
  bgBlur.value = info.blur;
  bgNativeSize.value = info.nativeSize;
  // 缓存一份 dataURL：下次启动挂载时立即有背景，不闪底色
  // （dataURL 很大，配额写满时静默失败，仅当次会话生效，见 storage.writeRaw）
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

/** 清除背景（传空串；Tauri 下同时删掉后端落盘的图与配置里的来源） */
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

/** 设置背景图来源（文件路径 / 网址；浏览器模式下直接收 dataURL）：
 *  Tauri 下由后端加载 → 按当前原始分辨率缩放 → 落盘并记入配置 */
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
  } catch (err) {
    // 带上后端的具体错误，方便定位加载失败的原因
    showToast(`${t("winSettings.bgLoadFailed")}：${String(err)}`);
  } finally {
    bgLoading.value = false;
  }
}

/** 调整原始分辨率（点「应用」后调用）：后端用缓存的原图重新缩放处理（不重新下载，
 *  随机图网址不会换图）；只改图片分辨率，不改变图在窗口里的显示（显示始终铺满居中） */
export async function resizeBg(percent: number) {
  bgNativeSize.value = Math.min(Math.max(Math.round(percent), 10), 100);
  if (!isTauri() || !bgSource.value) return;
  bgLoading.value = true;
  try {
    bgImage.value = await commands.settings.resizeBg(bgNativeSize.value);
  } catch (err) {
    showToast(`${t("winSettings.bgResizeFailed")}：${String(err)}`);
  } finally {
    bgLoading.value = false;
  }
}

export function setBgOpacity(v: number) {
  bgOpacity.value = v;
  writeString(KEYS.bgOpacity, String(v));
  void saveGuiConfig({ bgOpacity: v });
}

export function setBgBlur(v: number) {
  bgBlur.value = v;
  writeString(KEYS.bgBlur, String(v));
  void saveGuiConfig({ bgBlur: v });
}

/** 背景图层的内联样式（App.vue 的 .app-bg 绑定用）：始终铺满窗口（cover）并居中，
 *  标题栏（.frame-head / .topbar）与各表面一样半透明透出此层（见 themes.css 的 has-bg） */
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
