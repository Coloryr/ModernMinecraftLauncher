// 主题管理：暗色 / 亮色 + 强调色预设，状态持久化到 gui_config.json
import { ref } from "vue";
import { KEYS, readString, writeString, onStorageChange } from "./storage";
import { saveGuiConfig, type Theme } from "./guiConfig";
export type { Theme } from "./guiConfig";

export type AccentId =
  | "custom"
  | "blue"
  | "sky"
  | "cyan"
  | "teal"
  | "green"
  | "lime"
  | "yellow"
  | "orange"
  | "red"
  | "pink"
  | "fuchsia"
  | "purple"
  | "indigo";

/** 强调色预设列表：color 用于设置界面色块预览，check 为选中对勾颜色 */
export const ACCENTS: { id: AccentId; color: string; check: string }[] = [
  { id: "blue", color: "#4f8cff", check: "#fff" },
  { id: "sky", color: "#38bdf8", check: "#fff" },
  { id: "cyan", color: "#22d3ee", check: "#fff" },
  { id: "teal", color: "#2dd4bf", check: "#fff" },
  { id: "green", color: "#34d399", check: "#fff" },
  { id: "lime", color: "#a3e635", check: "#1d2229" },
  { id: "yellow", color: "#facc15", check: "#1d2229" },
  { id: "orange", color: "#fb923c", check: "#fff" },
  { id: "red", color: "#f87171", check: "#fff" },
  { id: "pink", color: "#f472b6", check: "#fff" },
  { id: "fuchsia", color: "#e879f9", check: "#fff" },
  { id: "purple", color: "#a78bfa", check: "#fff" },
  { id: "indigo", color: "#818cf8", check: "#fff" },
];

/**
 * 系统当前的应用深浅色，无显式选择时的默认来源
 *
 * # 返回值
 *
 * `Dark` 或 `Light`
 */
export function systemTheme(): Theme {
  return matchMedia("(prefers-color-scheme: dark)").matches ? "Dark" : "Light";
}

const storedTheme = readString(KEYS.theme);
// 显式存过的用存储值，含 System；否则跟随系统。配置加载前先用它首绘，避免闪一下错误主题
export const theme = ref<Theme>(
  storedTheme === "Light" || storedTheme === "Dark" || storedTheme === "System"
    ? storedTheme
    : systemTheme(),
);

const storedAccent = readString(KEYS.accent);
export const accent = ref<AccentId>(
  ACCENTS.some((a) => a.id === storedAccent) ? (storedAccent as AccentId) : "blue",
);

/** 自定义强调色（#rrggbb）：accent === "custom" 时生效 */
export const customAccent = ref(readString(KEYS.accentCustom, "#4f8cff"));

// ---------- 自定义强调色的派生色计算 ----------

/** #rrggbb → [r, g, b] */
function hexToRgb(hex: string): [number, number, number] {
  const n = parseInt(hex.slice(1), 16);
  return [(n >> 16) & 0xff, (n >> 8) & 0xff, n & 0xff];
}

/** [r, g, b] → #rrggbb */
function rgbToHex(r: number, g: number, b: number): string {
  return "#" + [r, g, b].map((v) => Math.round(Math.min(255, Math.max(0, v))).toString(16).padStart(2, "0")).join("");
}

/** 按比例靠拢：ratio > 0 向白，ratio < 0 向黑 */
function shade(hex: string, ratio: number): string {
  const [r, g, b] = hexToRgb(hex);
  const target = ratio > 0 ? 255 : 0;
  const k = Math.abs(ratio);
  return rgbToHex(r + (target - r) * k, g + (target - g) * k, b + (target - b) * k);
}

/** 用自定义色推导一套和预设同构的强调色变量，直接内联到 <html> 上
 *  内联样式优先级最高，能盖过 themes.css 里的 data-accent 预设块 */
function applyCustomAccentVars(hex: string) {
  const root = document.documentElement;
  root.style.setProperty("--accent", hex);
  root.style.setProperty("--accent-hover", shade(hex, -0.14));
  // 渐变第二段往亮偏一点，观感接近预设的双色渐变
  root.style.setProperty("--accent-grad", `linear-gradient(120deg, ${shade(hex, -0.08)}, ${shade(hex, 0.14)})`);
  const [r, g, b] = hexToRgb(hex);
  root.style.setProperty("--accent-soft", `rgba(${r}, ${g}, ${b}, 0.14)`);
  root.style.setProperty("--accent-border", `rgba(${r}, ${g}, ${b}, 0.4)`);
}

/** 移除内联的自定义强调色变量，交还给主题表的预设值 */
function clearCustomAccentVars() {
  const root = document.documentElement;
  for (const name of ["--accent", "--accent-hover", "--accent-grad", "--accent-soft", "--accent-border"]) {
    root.style.removeProperty(name);
  }
}

/**
 * 当前实际生效的主题，System 解析成系统当前的深浅色
 *
 * # 返回值
 *
 * `Dark` 或 `Light`
 */
export function resolvedTheme(): Theme {
  return theme.value === "System" ? systemTheme() : theme.value;
}

/** 把当前主题与强调色写到根元素，CSS 变量按 Dark / Light 定义 */
export function applyTheme() {
  const root = document.documentElement;
  // CSS 变量按 Dark / Light 定义，System 在这里解析成具体值
  root.dataset.theme = resolvedTheme();
  // 强调色：blue 是默认，theme.css 基础块已定义；其余通过 data-accent 覆盖；
  // custom 走内联变量，不用 data-accent
  if (accent.value === "custom") {
    delete root.dataset.accent;
    applyCustomAccentVars(customAccent.value);
  } else if (accent.value === "blue") {
    clearCustomAccentVars();
    delete root.dataset.accent;
  } else {
    clearCustomAccentVars();
    root.dataset.accent = accent.value;
  }
}

/**
 * 显式设置主题，设置窗口用，主页面不再提供切换入口
 *
 * - `v`: 主题
 */
export function setTheme(v: Theme) {
  theme.value = v;
  writeString(KEYS.theme, v);
  applyTheme();
  saveGuiConfig({ theme: v });
}

/**
 * 设置强调色
 *
 * - `a`: 预设 id 或 `custom`；`custom` 时用已存的自定义色
 */
export function setAccent(a: AccentId) {
  accent.value = a;
  writeString(KEYS.accent, a);
  applyTheme();
}

/**
 * 设置自定义强调色：存色值并把当前强调色切到 custom
 *
 * - `hex`: 颜色，`#rrggbb`
 */
export function setCustomAccent(hex: string) {
  customAccent.value = hex;
  writeString(KEYS.accentCustom, hex);
  accent.value = "custom";
  writeString(KEYS.accent, "custom");
  applyTheme();
}

// 跨窗口同步：某个窗口改了主题/颜色后，其它已打开的窗口实时生效
onStorageChange(KEYS.theme, (value) => {
  if (value === "Dark" || value === "Light" || value === "System") {
    theme.value = value;
    applyTheme();
  }
});

onStorageChange(KEYS.accent, (value) => {
  if (value === "custom") {
    // 其它窗口切到了自定义色：同步色值再应用
    customAccent.value = readString(KEYS.accentCustom, customAccent.value);
    accent.value = "custom";
  } else {
    accent.value = ACCENTS.some((x) => x.id === value) ? (value as AccentId) : "blue";
  }
  applyTheme();
});

// 跟随系统模式：系统深浅色切换时实时重应用
matchMedia("(prefers-color-scheme: dark)").addEventListener("change", () => {
  if (theme.value === "System") {
    applyTheme();
  }
});
