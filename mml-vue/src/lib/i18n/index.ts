// 国际化：zh_cn / en_us，状态持久化到 gui_config.json，与 core Lang 同名
// 用法：模板中 {{ t("launch.play") }}；带参数 {{ t("detail.launchCount", { count: 3 }) }}
import { ref } from "vue";
import { formatCountIn } from "../countFormat";
import zhCN from "./locales/zh-CN";
import enUS from "./locales/en-US";
import { KEYS, readString, writeString, onStorageChange } from "../storage";
import { saveGuiConfig, type Locale } from "../guiConfig";
export type { Locale } from "../guiConfig";

/** 当前界面语言；初始值取自本地存储，挂载前再由 gui_config 校正 */
export const locale = ref<Locale>(readString(KEYS.locale) === "en_us" ? "en_us" : "zh_cn");

/** 两套扁平文案表 */
const messages: Record<Locale, Record<string, string>> = {
  zh_cn: zhCN as Record<string, string>,
  en_us: enUS as Record<string, string>,
};

/**
 * 切换语言
 *
 * - `l`: 目标语言
 */
export function setLocale(l: Locale) {
  locale.value = l;
  writeString(KEYS.locale, l);
  applyLocale();
  saveGuiConfig({ locale: l });
}

/**
 * 把当前语言写到 `<html lang>`
 *
 * 取值保持 `zh_cn` / `en_us`：styles 里按 `:root[lang="en_us"]` 选字体，
 * 不要改成 `zh-CN` 这类 BCP 47 写法。
 */
export function applyLocale() {
  document.documentElement.lang = locale.value;
}

// 跨窗口同步：某个窗口改了语言后，其它已打开的窗口实时生效
onStorageChange(KEYS.locale, (value) => {
  if (value === "zh_cn" || value === "en_us") {
    locale.value = value;
    applyLocale();
  }
});

/**
 * 翻译
 *
 * - `key`: 点号分隔的扁平键
 * - `params`: 插值参数，键对应文案里的 `{param}` 占位
 *
 * # 返回值
 *
 * 译文；键不存在时原样返回 `key`
 */
export function t(key: string, params?: Record<string, string | number>): string {
  const msg = messages[locale.value][key];
  if (msg === undefined) return key;
  if (!params) return msg;
  return msg.replace(/\{(\w+)\}/g, (_, k: string) =>
    params[k] !== undefined ? String(params[k]) : `{${k}}`,
  );
}

/**
 * 翻译后端命令返回的错误
 *
 * 后端 `Err(...)` 返回的是 i18n 键（`err.*`）；`t()` 对不认识的键原样返回，
 * 所以网络 / 第三方返回的普通错误串传进来也不会有副作用。
 *
 * - `e`: 后端返回的错误，通常是 i18n 键
 *
 * # 返回值
 *
 * 译文；不是已知键时原样返回
 */
export function tErr(e: unknown): string {
  return t(String(e));
}

/**
 * 按当前语言简化下载量这类大数字，见 `formatCountIn`
 *
 * 与 `t()` 同一处导入：界面里凡是显示下载量的地方都该用它，
 * 各处自己拼"万 / K"迟早会拼出两套口径。
 *
 * - `n`: 原始数字
 *
 * # 返回值
 *
 * 形如 `1万+` / `10K+`
 */
export function formatCount(n: number): string {
  return formatCountIn(n, locale.value);
}

/**
 * 取本模块的国际化入口
 *
 * # 返回值
 *
 * `t` / `tErr` 两个翻译函数、`formatCount` 大数字简化，
 * `locale` 是当前语言的 ref，`setLocale` 用来切换语言
 */
export function useI18n() {
  return { t, tErr, formatCount, locale, setLocale };
}
