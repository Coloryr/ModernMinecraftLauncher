// 启动阶段 → 界面文案
//
// 主界面那颗启动按钮、日志窗口的测试启动按钮都走这一份；阶段名由后端 `launch-state` 事件给，
// 取值见 mml-game 的 `LaunchState`（GUI 侧 state_key 一一对应）。
import { t } from "./i18n";

/**
 * 启动阶段的界面文案
 *
 * - `state`: 后端给的阶段名，如 launching / check / download / end
 *
 * # 返回值
 *
 * 该阶段的文案；i18n 里没有这个键时原样返回阶段名
 */
export function stateText(state: string): string {
  const key = `state.${state}`;
  const msg = t(key);
  return msg === key ? state : msg;
}
