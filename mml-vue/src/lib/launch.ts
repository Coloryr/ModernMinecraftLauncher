// 启动实例的统一入口
//
// 「自动打开日志窗口」必须在前端按当前窗口模式处理：单窗口模式是应用内切页，
// 多窗口模式才是开新窗口；后端直接建窗会把单窗口模式撑出第二个窗口。
import { api, modpackCheck } from "./api";
import { askAccountLock } from "./accountLock";
import { askAutoSave } from "./autoSavePrompt";
import { askLaunchCheck } from "./launchCheck";
import { askServerPackUpdate } from "./serverPackPrompt";
import { saveGuiConfig } from "./guiConfig";
import { openWindow } from "../windows/windowManager";

/**
 * 启动实例；自动进入存档与账户占用都要先问，实例开了「自动打开日志窗口」时启动后把日志带出来
 *
 * - `uuid`: 实例 uuid
 * - `world`: 启动后直接进入的存档目录名，可选；这是用户明确点的「进入存档」，不再问自动进入
 *
 * # 返回值
 *
 * 启动请求已交给后端时返回 `true`；用户在内存提醒或账户占用弹窗里取消返回 `false`；
 * 其它失败抛出后端错误文案
 */
export async function launchGame(uuid: string, world?: string): Promise<boolean> {
  // 启动前检查：有模组却用原版加载器、模组多而最大内存偏低时各问一次
  const check = await api.checkLaunch(uuid).catch(() => null);
  if (check) {
    if (check.checkLoader && check.mods > 0 && check.loaderNormal) {
      const answer = await askLaunchCheck("loader", check);
      if (!answer.ok) return false;
      if (answer.disable) {
        await saveGuiConfig({ launchCheck: { checkLoader: false } }).catch(() => { });
      }
    }
    if (check.checkMemory && !check.loaderNormal && check.memoryLow) {
      const answer = await askLaunchCheck("memory", check);
      if (!answer.ok) return false;
      if (answer.disable) {
        await saveGuiConfig({ launchCheck: { checkMemory: false } }).catch(() => { });
      }
    }
  }

  // 在线服务器包：启动时后端本来会直接按发布地址同步，这里先问一句，别悄悄升级
  const serverPack = await askServerPack(uuid);
  if (serverPack === null) return false;

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
        await saveGuiConfig({ client: { autoSaveNotice: false } }).catch(() => { });
      }
      if (answer.disable) {
        await api.updateInstance(uuid, { autoSave: null }).catch(() => { });
      }
      // 选"进入"就按这个目录显式进（配置可能刚被清掉）；选"不进入"则让后端忽略配置
      if (answer.enter) enterWorld = target.dir;
      else autoSave = false;
    }
  }

  try {
    await api.launchGame(uuid, enterWorld, undefined, autoSave, serverPack);
  } catch (e) {
    if (String(e) !== "err.accountInUse") throw e;

    const answer = await askAccountLock();
    if (!answer.ok) return false;
    if (!answer.keep) {
      await saveGuiConfig({ client: { accountCheck: false } }).catch(() => { });
    }
    // 这一次先放行；检查关掉后后续启动不会再被拦
    await api.launchGame(uuid, enterWorld, true, autoSave, serverPack);
  }

  if (autoShow) {
    openWindow("log", { uuid });
  }
  return true;
}

/**
 * 启动前查服务器包有没有新版本，有就问用户
 *
 * 只对填了发布地址的在线服务器包实例查；其它实例直接返回 `undefined`，不发请求。
 *
 * - `uuid`: 实例 uuid
 *
 * # 返回值
 *
 * `"update"` 先同步再启动 / `"skip"` 直接启动 / `null` 用户取消启动 / `undefined` 不用问
 */
async function askServerPack(uuid: string): Promise<"update" | "skip" | null | undefined> {
  const list = await api.getInstances().catch(() => []);
  const instance = list.find((item) => item.uuid === uuid);
  if (instance?.modpackType !== "serverpack" || !instance.serverUrl) return undefined;

  // 查不到（地址不通 / 清单坏了）就当没更新：启动本身不该被一次查询挡住
  const result = await modpackCheck(uuid).catch(() => null);
  if (!result?.hasUpdate) return undefined;

  const answer = await askServerPackUpdate(result);
  if (!answer.ok) return null;
  return answer.update ? "update" : "skip";
}
