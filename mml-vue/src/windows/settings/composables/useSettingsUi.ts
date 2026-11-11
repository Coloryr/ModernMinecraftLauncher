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
  bgFrosted,
  bgImage,
  bgLoading,
  bgNativeSize,
  bgOpacity,
  bgSource,
  copyBgSource,
  setBgBlur,
  setBgFrosted,
  setBgImage,
  setBgImageFromOriginal,
  setBgOpacity,
  resizeBg,
} from "../../../lib/appearance";
import { sidebarSide, setSidebarSide, setViewMode, startupPage, setStartupPage, viewMode, type SidebarSide, type StartupPage, type ViewMode } from "../../../lib/settings";
import {
  ACCENTS,
  accent,
  customAccent,
  gradientMode,
  setAccent,
  setCustomAccent,
  setGradientMode,
  setTheme,
  theme,
  type Theme,
} from "../../../lib/theme";
import { fontFamily, setFontFamily } from "../../../lib/fonts";
import { showToast } from "../../../lib/toast";
import { isTauri, multiWindow, resetToMain, setMultiWindow } from "../../windowManager";
import { commands } from "../../../lib/bindings";

/** 界面标签里"即改即存"的默认值，与 Rust 侧 GuiConfig::default 一致 */
const DEFAULT_THEME: Theme = "System";
const DEFAULT_ACCENT = "blue";
const DEFAULT_CUSTOM_ACCENT = "#4f8cff";
const DEFAULT_LOCALE = "zh_cn" as const;
const DEFAULT_SIDE: SidebarSide = "Left";
const DEFAULT_STARTUP_PAGE: StartupPage = "Home";
const DEFAULT_VIEW_MODE: ViewMode = "list";

/** 拖滑杆时的落盘延迟，单位毫秒；原来每次 input 都发一次 IPC */
const SLIDER_COMMIT_DELAY = 250;

/**
 * 界面标签（含窗口设置）的状态与保存
 *
 * # 返回值
 *
 * - `windowMode` / `onModeChange`: 单窗口 / 多窗口，切换后重载界面
 * - `locale` / `onLangChange` / `side` / `onSideChange` / `themeValue` / `onThemeChange`: 语言、侧栏、主题
 * - `startup` / `onStartupChange`: 启动器启动时先显示启动器主页还是项目列表
 * - `view` / `onViewModeChange`: 实例列表按分组 / 列表哪种方式显示
 * - `fonts` / `fontsLoading` / `loadFonts` / `fontPick` / `setFontFamily`: 系统字体
 * - `animations` / `setAnimations` / `accent` / `setAccent` / `customAccent` / `setCustomAccent` / `ACCENTS`: 动画与强调色
 * - `bgImage` / `setBgImage` / `bgLoading` / `bgFileInput` / `bgSourceInput` / `loadBgSource` / `pickBgImage` / `onBgFile`: 背景图加载与清除
 * - `bgCopyAsk` / `answerBgCopy`: 选完本地图后问要不要复制进启动器目录
 * - `bgSizeDraft` / `applyBgSize` / `bgNativeSize` / `bgOpacity` / `setBgOpacity` / `bgBlur` / `setBgBlur`: 背景图缩放、不透明度与模糊，均为草稿加延迟落盘
 * - `inTauri` / `resetGroup`: 环境判断与分组级"恢复默认"
 */
export function useSettingsUi() {
  const inTauri = isTauri();

  // ---------- 快照（分段控件 / 下拉的双向绑定） ----------

  const windowMode = ref(multiWindow.value ? "Multi" : "Single");
  const side = ref<SidebarSide>(sidebarSide.value);
  const startup = ref<StartupPage>(startupPage.value);
  const view = ref<ViewMode>(viewMode.value);
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

  /**
   * 窗口模式：换的只是"新窗口怎么开"，多窗口开真实窗口、单窗口走应用内页面
   *
   * 所以就地改掉模式、把旧模式建出来的窗口收掉就够：**不重载界面、不重启进程** ——
   * 重载会让窗口在几秒里先落回原生标题栏，游戏在跑时重启更会把它一起带走
   */
  async function onModeChange(v: string) {
    const multi = v === "Multi";
    windowMode.value = v;
    if (multi === multiWindow.value) return;
    // 先等配置写下去：别的地方（以及下次启动）都按这个值走
    await setMultiWindow(multi);
    // 浏览器预览没有真实窗口要关
    if (inTauri) {
      try {
        await commands.windows.closeOtherWindows();
      } catch (e) {
        showToast(tErr(e));
      }
    }
    // 切到多窗口：当前这一页会变成主窗口里的一页，先回主页面 ——
    // 否则在那页点标题栏的 ✕ 会被当成关主窗口。切到单窗口时这边是设置窗口，马上被关掉
    if (multi) resetToMain();
  }

  /** 语言：只认 zh_cn / en_us，其余回落到默认 */
  function onLangChange(v: string) {
    setLocale(v === "en_us" ? "en_us" : DEFAULT_LOCALE);
  }

  /** 侧栏停靠到左边还是右边 */
  function onSideChange(v: string) {
    side.value = v === "Right" ? "Right" : DEFAULT_SIDE;
    setSidebarSide(side.value);
  }

  /** 启动页：启动器主页或项目列表 */
  function onStartupChange(v: string) {
    startup.value = v === "List" ? "List" : DEFAULT_STARTUP_PAGE;
    setStartupPage(startup.value);
  }

  /** 实例列表显示模式：改完落盘，主窗口由后端广播的 gui-view-mode-change 跟着换视图 */
  function onViewModeChange(v: string) {
    view.value = v === "group" || v === "grid" ? "group" : DEFAULT_VIEW_MODE;
    setViewMode(view.value);
  }

  /** 主题：跟随系统 / 浅色 / 深色 */
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

  /** 选完图、加载生效前的复制询问；true 表示弹窗开着 */
  const bgCopyAsk = ref(false);
  /** 等待回答的回调；再次询问时按"不复制"放掉上一次 */
  let bgCopyResolve: ((copy: boolean) => void) | null = null;

  /**
   * 问一句要不要把这张图复制进启动器目录（在加载生效之前问）
   *
   * # 返回值
   *
   * 用户选择复制返回 true
   */
  function askBgCopy(): Promise<boolean> {
    bgCopyResolve?.(false);
    bgCopyResolve = null;
    return new Promise((resolve) => {
      bgCopyResolve = resolve;
      bgCopyAsk.value = true;
    });
  }

  /**
   * 回答"是否复制进启动器目录"
   *
   * - `copy`: true 表示先复制成 `bg/` 下的副本再加载
   */
  function answerBgCopy(copy: boolean) {
    bgCopyAsk.value = false;
    const resolve = bgCopyResolve;
    bgCopyResolve = null;
    resolve?.(copy);
  }

  /** 这个地址是不是启动器目录下的副本（相对路径 `bg/...`） */
  function isCopyPath(source: string) {
    return source.replace(/\\/g, "/").startsWith("bg/");
  }

  /** 按输入框里的地址（本地路径或网址）加载背景 */
  async function loadBgSource() {
    const source = bgSourceInput.value.trim();
    if (!source) return;
    // 本地图在生效前先问一句：复制的话先把文件落进 bg/，再用相对路径加载
    let target = source;
    const local = !source.startsWith("http://") && !source.startsWith("https://");
    if (inTauri && local && !isCopyPath(source) && (await askBgCopy())) {
      try {
        target = await copyBgSource(source);
      } catch (err) {
        showToast(`${t("winSettings.bgCopyFailed")}${t("common.detailSep")}${String(err)}`);
        return;
      }
      bgSourceInput.value = target;
    }
    // 后端加载 → 缩放 → 落盘并记入配置，失败时由 setBgImageFromOriginal 内部提示；
    // 输入框里的地址保留不清空，方便看到当前用的是哪张图
    await setBgImageFromOriginal(target);
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
        setAnimations(true);
        setGradientMode(true);
        setBgFrosted(true);
        await onModeChange("Multi");
        return true;
      case "mainWindow":
        setSidebarSide(DEFAULT_SIDE);
        side.value = DEFAULT_SIDE;
        setStartupPage(DEFAULT_STARTUP_PAGE);
        startup.value = DEFAULT_STARTUP_PAGE;
        setViewMode(DEFAULT_VIEW_MODE);
        view.value = DEFAULT_VIEW_MODE;
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
    startup,
    onStartupChange,
    view,
    onViewModeChange,
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
    gradientMode,
    setGradientMode,
    ACCENTS,
    bgImage,
    setBgImage,
    bgCopyAsk,
    answerBgCopy,
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
    bgFrosted,
    setBgFrosted,
    resetGroup,
  };
}
