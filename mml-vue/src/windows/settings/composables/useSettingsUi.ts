// 设置窗口 · 界面标签（含窗口设置）
//
// 这些设置的"真源"分散在 lib/{i18n,fonts,theme,settings,appearance}.ts 里，
// 各家自己负责本地存储并 saveGuiConfig 双写，这里只收成窗口要用的接口：
// 1. 分段控件的快照 ref：windowMode / side / themeValue / fontPick；
// 2. 系统字体的懒加载：font-kit 扫字体是重活，进界面页才拉；
// 3. 背景图的加载 / 清除 / 缩放，以及滑杆草稿（拖动过程中不逐次落盘）；
// 4. resetGroup(id)：分组级"恢复默认"。
import { onUnmounted, ref, watch } from "vue";
import { locale, setLocale, t, tErr } from "../../../lib/i18n";
import {
  animations,
  setAnimations,
  bgBlur,
  bgImage,
  bgLoading,
  bgNativeSize,
  bgOpacity,
  bgSource,
  setBgBlur,
  setBgImage,
  setBgImageFromOriginal,
  setBgOpacity,
  resizeBg,
} from "../../../lib/appearance";
import { sidebarSide, setSidebarSide, type SidebarSide } from "../../../lib/settings";
import {
  ACCENTS,
  accent,
  customAccent,
  setAccent,
  setCustomAccent,
  setTheme,
  theme,
  type Theme,
} from "../../../lib/theme";
import { fontFamily, setFontFamily } from "../../../lib/fonts";
import { showToast } from "../../../lib/toast";
import { isTauri, multiWindow, setMultiWindow } from "../../windowManager";
import { commands } from "../../../lib/bindings";

/** 界面标签里"即改即存"的默认值，与 Rust 侧 GuiConfig::default 一致 */
const DEFAULT_THEME: Theme = "System";
const DEFAULT_ACCENT = "blue";
const DEFAULT_CUSTOM_ACCENT = "#4f8cff";
const DEFAULT_LOCALE = "zh_cn" as const;
const DEFAULT_SIDE: SidebarSide = "Left";

/** 拖滑杆时的落盘延迟，单位毫秒；原来每次 input 都发一次 IPC */
const SLIDER_COMMIT_DELAY = 250;

/**
 * 界面标签（含窗口设置）的状态与保存
 *
 * # 返回值
 *
 * - `windowMode` / `onModeChange`: 单窗口 / 多窗口，切换后要重启进程
 * - `locale` / `onLangChange` / `side` / `onSideChange` / `themeValue` / `onThemeChange`: 语言、侧栏、主题
 * - `fonts` / `fontsLoading` / `loadFonts` / `fontPick` / `setFontFamily`: 系统字体
 * - `animations` / `setAnimations` / `accent` / `setAccent` / `customAccent` / `setCustomAccent` / `ACCENTS`: 动画与强调色
 * - `bgImage` / `setBgImage` / `bgLoading` / `bgFileInput` / `bgSourceInput` / `loadBgSource` / `pickBgImage` / `onBgFile`: 背景图加载与清除
 * - `bgSizeDraft` / `applyBgSize` / `bgNativeSize` / `bgOpacity` / `setBgOpacity` / `bgBlur` / `setBgBlur`: 背景图缩放、不透明度与模糊，均为草稿加延迟落盘
 * - `inTauri` / `resetGroup`: 环境判断与分组级"恢复默认"
 */
export function useSettingsUi() {
  const inTauri = isTauri();

  // ---------- 快照（分段控件 / 下拉的双向绑定） ----------

  const windowMode = ref(multiWindow.value ? "Multi" : "Single");
  const side = ref<SidebarSide>(sidebarSide.value);
  const themeValue = ref<Theme>(theme.value);
  const fontPick = ref(fontFamily.value);

  // ---------- 系统字体（懒加载） ----------

  const fonts = ref<string[]>([]);
  const fontsLoading = ref(false);

  /** 枚举系统字体：只在界面页需要时拉一次，失败留空 */
  async function loadFonts() {
    if (!inTauri || fontsLoading.value || fonts.value.length) return;
    fontsLoading.value = true;
    try {
      fonts.value = await commands.settings.getSystemFonts();
    } catch {
      fonts.value = [];
    } finally {
      fontsLoading.value = false;
    }
  }

  // ---------- 各项改动（都经各自 lib 落盘） ----------

  /** 窗口模式：切换后要重启进程才干净 —— 已开着的窗口是旧模式下建的，就地切换它们留在旧模式里 */
  async function onModeChange(v: string) {
    const multi = v === "Multi";
    windowMode.value = v;
    if (multi === multiWindow.value) return;
    // 先等配置真写下去：下面重启会立刻刷盘退出，写请求还在路上就白改了
    await setMultiWindow(multi);
    // 浏览器预览没有进程可重启
    if (!inTauri) return;
    showToast(t("winSettings.restarting"));
    try {
      await commands.windows.restartApp();
    } catch (e) {
      showToast(tErr(e));
    }
  }

  function onLangChange(v: string) {
    setLocale(v === "en_us" ? "en_us" : DEFAULT_LOCALE);
  }

  function onSideChange(v: string) {
    side.value = v === "Right" ? "Right" : DEFAULT_SIDE;
    setSidebarSide(side.value);
  }

  function onThemeChange(v: string) {
    const next = v as Theme;
    themeValue.value = next;
    setTheme(next);
  }

  // ---------- 背景图 ----------

  const bgFileInput = ref<HTMLInputElement | null>(null);
  const bgSourceInput = ref("");
  const bgSizeDraft = ref(bgNativeSize.value);

  // 回显已设置的来源（打开窗口时不再是空的）；清除背景时输入框跟着清空
  watch(bgSource, (v) => (bgSourceInput.value = v), { immediate: true });

  /** 选择图片：Tauri 用系统文件对话框，拿到真实路径回填输入框后走统一加载管线；
   *  浏览器回退到文件输入 */
  async function pickBgImage() {
    if (inTauri) {
      const { open } = await import("@tauri-apps/plugin-dialog");
      const picked = await open({
        title: t("winSettings.bgPick"),
        multiple: false,
        filters: [{ name: "Image", extensions: ["png", "jpg", "jpeg", "webp"] }],
      });
      if (typeof picked === "string" && picked) {
        bgSourceInput.value = picked;
        await loadBgSource();
      }
      return;
    }
    bgFileInput.value?.click();
  }

  function onBgFile(e: Event) {
    const input = e.target as HTMLInputElement;
    const file = input.files?.[0];
    input.value = ""; // 允许重复选同一张图
    if (!file) return;
    bgSourceInput.value = file.name;
    const reader = new FileReader();
    reader.onload = () => setBgImageFromOriginal(String(reader.result));
    reader.readAsDataURL(file);
  }

  /** 按输入框里的地址（本地路径或网址）加载背景 */
  async function loadBgSource() {
    const source = bgSourceInput.value.trim();
    if (!source) return;
    // 后端加载 → 缩放 → 落盘并记入配置，失败时由 setBgImageFromOriginal 内部提示；
    // 输入框里的地址保留不清空，方便看到当前用的是哪张图
    await setBgImageFromOriginal(source);
  }

  /** 原始大小：拖动只改草稿，点「应用」才触发后端重新缩放 */
  function applyBgSize() {
    void resizeBg(bgSizeDraft.value);
  }

  // 不透明度 / 模糊：草稿即时反映到滑杆，停顿后再落盘
  const opacityDraft = ref(bgOpacity.value);
  const blurDraft = ref(bgBlur.value);
  let sliderTimer: number | null = null;

  function commitSliders() {
    sliderTimer = null;
    if (opacityDraft.value !== bgOpacity.value) setBgOpacity(opacityDraft.value);
    if (blurDraft.value !== bgBlur.value) setBgBlur(blurDraft.value);
  }

  watch([opacityDraft, blurDraft], () => {
    if (sliderTimer !== null) clearTimeout(sliderTimer);
    sliderTimer = window.setTimeout(commitSliders, SLIDER_COMMIT_DELAY);
  });

  onUnmounted(() => {
    if (sliderTimer !== null) {
      clearTimeout(sliderTimer);
      commitSliders();
    }
  });

  // ---------- 恢复默认 ----------

  /** 分组级"恢复默认"：返回是否处理了该分组 */
  async function resetGroup(id: string): Promise<boolean> {
    switch (id) {
      case "general":
        setLocale(DEFAULT_LOCALE);
        setAnimations(true);
        setFontFamily("");
        fontPick.value = "";
        return true;
      case "theme":
        setTheme(DEFAULT_THEME);
        themeValue.value = DEFAULT_THEME;
        setAccent(DEFAULT_ACCENT as (typeof ACCENTS)[number]["id"]);
        setCustomAccent(DEFAULT_CUSTOM_ACCENT);
        return true;
      case "window":
        // 与开关走同一条路：模式真变了就一起重启，否则会出现"设置显示多窗口、实际还是单窗口"
        await onModeChange("Multi");
        return true;
      case "mainWindow":
        setSidebarSide(DEFAULT_SIDE);
        side.value = DEFAULT_SIDE;
        return true;
      case "bgImage":
        setBgImage("");
        bgSourceInput.value = "";
        opacityDraft.value = 100;
        blurDraft.value = 0;
        return true;
      default:
        return false;
    }
  }

  return {
    inTauri,
    locale,
    onLangChange,
    windowMode,
    onModeChange,
    side,
    onSideChange,
    themeValue,
    onThemeChange,
    fonts,
    fontsLoading,
    loadFonts,
    fontPick,
    setFontFamily,
    animations,
    setAnimations,
    accent,
    setAccent,
    customAccent,
    setCustomAccent,
    ACCENTS,
    bgImage,
    setBgImage,
    bgLoading,
    bgFileInput,
    bgSourceInput,
    loadBgSource,
    pickBgImage,
    onBgFile,
    bgSizeDraft,
    applyBgSize,
    bgNativeSize,
    bgOpacity: opacityDraft,
    setBgOpacity: (v: number) => (opacityDraft.value = v),
    bgBlur: blurDraft,
    setBgBlur: (v: number) => (blurDraft.value = v),
    resetGroup,
  };
}
