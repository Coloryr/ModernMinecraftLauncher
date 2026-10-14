// 界面字体设置：选择系统字体族后全局应用，写 CSS 变量 --app-font
// 字体列表经 settings_get_system_fonts 从 Rust 侧枚举（font-kit）
import { ref } from "vue";
import { KEYS, readString, writeString } from "./storage";
import { saveGuiConfig } from "./guiConfig";

/** 当前字体族名（空串 = 默认字体栈） */
export const fontFamily = ref(readString(KEYS.font));

/**
 * 设置界面字体，持久化到本地存储 + gui_config.json
 *
 * - `family`: 字体族名，空串恢复默认字体栈
 */
export function setFontFamily(family: string) {
  fontFamily.value = family;
  writeString(KEYS.font, family);
  applyFont();
  saveGuiConfig({ font: family });
}

/**
 * 启动恢复：只同步内存与镜像，不回写 gui_config.json
 *
 * - `family`: 字体族名，空串恢复默认字体栈
 */
export function restoreFontFamily(family: string) {
  fontFamily.value = family;
  writeString(KEYS.font, family);
  applyFont();
}

/**
 * 字体族名 → 可直接用的 font-family 值
 *
 * 名字本身排最前，后面跟全局那套回退栈（缺字时用系统字体兜底）；
 * 中英文界面的栈序不同，基线归属见 base.css 注释。
 *
 * - `family`: 字体族名，不要带引号
 * - `western`: 是否按西文界面排（默认按中文那套：CJK 字体在前）
 *
 * # 返回值
 *
 * `font-family` 的值，可直接塞进样式
 */
export function fontStack(family: string, western = false): string {
  const f = `"${family.replace(/"/g, '\\"')}"`;
  return western
    ? `${f}, "Segoe UI", "Microsoft YaHei", "PingFang SC", system-ui, sans-serif`
    : `${f}, "Microsoft YaHei", "PingFang SC", "Segoe UI", system-ui, sans-serif`;
}

/** 把当前字体写到根元素 CSS 变量，base.css 的 body 字体栈引用它 */
export function applyFont() {
  const root = document.documentElement;
  if (!fontFamily.value) {
    root.style.removeProperty("--app-font");
    root.style.removeProperty("--app-font-en");
    return;
  }
  root.style.setProperty("--app-font", fontStack(fontFamily.value));
  root.style.setProperty("--app-font-en", fontStack(fontFamily.value, true));
}

// 窗口启动即应用（配置恢复前的兜底用本地存储值）
applyFont();
