// 启动实例的统一入口
//
// 「自动打开日志窗口」必须在前端按当前窗口模式处理：单窗口模式是应用内切页，
// 多窗口模式才是开新窗口；后端直接建窗会把单窗口模式撑出第二个窗口。
import { api } from "./api";
import { openWindow } from "../windows/windowManager";

/**
 * 启动实例；实例开了「自动打开日志窗口」时，启动后把日志带出来
 *
 * - `uuid`: 实例 uuid
 * - `world`: 启动后直接进入的存档目录名，可选
 *
 * # 返回值
 *
 * 启动请求已交给后端时返回；失败时抛出后端错误文案
 */
export async function launchGame(uuid: string, world?: string): Promise<void> {
  let autoShow = false;
  try {
    autoShow = (await api.getLogSetting(uuid)).autoShow;
  } catch {
    // 读设置失败不影响启动本身
  }
  await api.launchGame(uuid, world);
  if (autoShow) {
    openWindow("log", { uuid });
  }
}
