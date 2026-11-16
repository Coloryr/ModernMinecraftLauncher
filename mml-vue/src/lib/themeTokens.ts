// iframe 主题变量下发
//
// 组件型主题跑在宿主 DOM 里，--bg-card 这些变量天然可用；iframe 是独立文档，
// 只能由宿主读出来、postMessage 过去（发送侧见 lib/customHomeBridge.ts）。
// 白名单与 styles/tailwind-tokens.css 的映射一一对应：那边加了新 token，
// 这里补一行，否则 iframe 主题拿不到它。

/** 下发给 iframe 的主题变量 */
export const THEME_TOKENS: readonly string[] = [
  "--bg",
  "--bg-side",
  "--bg-card",
  "--bg-hover",
  "--bg-raised",
  "--bg-solid",
  "--bg-modal",
  "--overlay",
  "--border",
  "--text",
  "--text-dim",
  "--text-sub",
  "--accent",
  "--accent-soft",
  "--accent-border",
  "--on-accent",
  "--green",
  "--red",
  "--yellow",
  "--ctl-h",
  "--ctl-radius",
  "--ctl-font",
];

/**
 * 读出当前主题的变量值
 *
 * 值一律现取：主题、强调色、纯色模式一变，同一批变量算出来的结果就不同。
 *
 * # 返回值
 *
 * 变量名 → 计算值；取不到或为空的不出现在结果里
 */
export function readThemeTokens(): Record<string, string> {
  const style = getComputedStyle(document.documentElement);
  const out: Record<string, string> = {};
  for (const name of THEME_TOKENS) {
    const value = style.getPropertyValue(name).trim();
    if (value) out[name] = value;
  }
  return out;
}
