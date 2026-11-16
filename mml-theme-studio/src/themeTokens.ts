import { THEME_TOKENS } from '../../mml-vue/src/lib/themeTokens';
import type { Theme } from './api';

/**
 * 读出某个配色的主题变量
 *
 * 预览页是独立文档，拿不到编辑器这边的 CSS 变量，只能按配色算好再发过去。
 *
 * - `theme`: 目标配色，值与启动器的 `data-theme` 一致
 *
 * # 返回值
 *
 * 变量名 → 计算值；取不到或为空的不出现在结果里
 */
export function themeTokens(theme: Theme): Record<string, string> {
  const root = document.documentElement;
  const current = root.dataset.theme;
  root.dataset.theme = theme;
  const style = getComputedStyle(root);
  const tokens: Record<string, string> = {};
  for (const name of THEME_TOKENS) {
    const value = style.getPropertyValue(name).trim();
    if (value) tokens[name] = value;
  }
  // 临时切配色只为读值，读完立刻切回去，中间不会重绘
  if (current === undefined) delete root.dataset.theme;
  else root.dataset.theme = current;
  return tokens;
}
