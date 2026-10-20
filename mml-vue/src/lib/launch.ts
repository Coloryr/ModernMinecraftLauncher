// 启动实例的统一入口
//
// 「自动打开日志窗口」必须在前端按当前窗口模式处理：单窗口模式是应用内切页，
// 多窗口模式才是开新窗口；后端直接建窗会把单窗口模式撑出第二个窗口。
import { api } from "./api";
import { askAccountLock } from "./accountLock";
import { saveGuiConfig } from "./guiConfig";
import { openWindow } from "../windows/windowManager";

/**
 * 启动实例；账户被占用时先询问，实例开了「自动打开日志窗口」时启动后把日志带出来
 *
 * - `uuid`: 实例 uuid
 * - `world`: 启动后直接进入的存档目录名，可选
 *
 * # 返回值
 *
 * 启动请求已交给后端时返回 `true`；用户在账户占用弹窗里取消返回 `false`；
 * 其它失败抛出后端错误文案
 */
export async function launchGame(uuid: string, world?: string): Promise<boolean> {
  let autoShow = false;
  try {
    autoShow = (await api.getLogSetting(uuid)).autoShow;
  } catch {
    // 读设置失败不影响启动本身
  }

  try {
    await api.launchGame(uuid, world);
  } catch (e) {
    if (String(e) !== "err.accountInUse") throw e;

    const answer = await askAccountLock();
    if (!answer.ok) return false;
    if (!answer.keep) {
      await saveGuiConfig({ client: { accountCheck: false } }).catch(() => {});
    }
    // 这一次先放行；检查关掉后后续启动不会再被拦
    await api.launchGame(uuid, world, true);
  }

  if (autoShow) {
    openWindow("log", { uuid });
  }
  return true;
}
