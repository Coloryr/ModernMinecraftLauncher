// 启动实例的统一入口
//
// 「自动打开日志窗口」必须在前端按当前窗口模式处理：单窗口模式是应用内切页，
// 多窗口模式才是开新窗口；后端直接建窗会把单窗口模式撑出第二个窗口。
import { api } from "./api";
import { askAccountLock } from "./accountLock";
import { askAutoSave } from "./autoSavePrompt";
import { saveGuiConfig } from "./guiConfig";
import { openWindow } from "../windows/windowManager";

/**
 * 启动实例；自动进入存档与账户占用都要先问，实例开了「自动打开日志窗口」时启动后把日志带出来
 *
 * - `uuid`: 实例 uuid
 * - `world`: 启动后直接进入的存档目录名，可选；这是用户明确点的「进入世界」，不再问自动进入
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

  // 实例配了"启动后自动进入存档"时先确认；提醒开关、版本与存档是否存在都由后端判断，
  // 返回 null 表示这次不用问
  let enterWorld = world;
  let autoSave: boolean | undefined;
  if (!world) {
    const target = await api.autoSaveTarget(uuid).catch(() => null);
    if (target) {
      const answer = await askAutoSave(target);
      if (!answer.notice) {
        await saveGuiConfig({ client: { autoSaveNotice: false } }).catch(() => {});
      }
      if (answer.disable) {
        await api.updateInstance(uuid, { autoSave: null }).catch(() => {});
      }
      // 选"进入"就按这个目录显式进（配置可能刚被清掉）；选"不进入"则让后端忽略配置
      if (answer.enter) enterWorld = target.dir;
      else autoSave = false;
    }
  }

  try {
    await api.launchGame(uuid, enterWorld, undefined, autoSave);
  } catch (e) {
    if (String(e) !== "err.accountInUse") throw e;

    const answer = await askAccountLock();
    if (!answer.ok) return false;
    if (!answer.keep) {
      await saveGuiConfig({ client: { accountCheck: false } }).catch(() => {});
    }
    // 这一次先放行；检查关掉后后续启动不会再被拦
    await api.launchGame(uuid, enterWorld, true, autoSave);
  }

  if (autoShow) {
    openWindow("log", { uuid });
  }
  return true;
}
