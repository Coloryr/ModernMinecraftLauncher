// 启动实例的统一入口
//
// 「自动打开日志窗口」必须在前端按当前窗口模式处理：单窗口模式是应用内切页，
// 多窗口模式才是开新窗口；后端直接建窗会把单窗口模式撑出第二个窗口。
import { api, modpackCheck } from "./api";
import { askAccountLock } from "./accountLock";
import { setCurrentAccountByUuid } from "./accountStore";
import { askAutoSave } from "./autoSavePrompt";
import { askGamepadAsm } from "./gamepadAsmPrompt";
import { askLaunchCommand } from "./launchCommandPrompt";
import { askLaunchCheck } from "./launchCheck";
import { askServerPackUpdate } from "./serverPackPrompt";
import { askServerPackMods } from "./serverPackMods";
import { saveGuiConfig } from "./guiConfig";
import { openWindow } from "../windows/windowManager";

/**
 * 启动实例；启动命令、自动进入存档与账户占用都要先问，实例开了「自动打开日志窗口」时启动后把日志带出来
 *
 * - `uuid`: 实例 uuid
 * - `world`: 启动后直接进入的存档目录名，可选；这是用户明确点的「进入存档」，不再问自动进入
 * - `renderdoc`: 是否用 RenderDoc 启动
 * - `nsight`: 是否用本机安装的 Nsight Graphics 启动
 * - `jvmDebug`: JVM 远程调试端口；不给就不加 JDWP 参数
 *
 * # 返回值
 *
 * 启动请求已交给后端时返回 `true`；用户在任一弹窗里取消返回 `false`；
 * 其它失败抛出后端错误文案
 */
export async function launchGame(
  uuid: string,
  world?: string,
  renderdoc = false,
  nsight = false,
  jvmDebug?: number,
): Promise<boolean> {
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

  // 游戏内手柄要经 ColorASM 的 IPC 才能进游戏：没注入时先问一次。
  // 选「不开启」照常启动，只是手柄在游戏里不生效；后端判断要不要问
  const gamepadAsm = await api.gamepadNeedsAsm(uuid).catch(() => false);
  if (gamepadAsm) {
    const answer = await askGamepadAsm();
    if (!answer.ok) return false;
    if (answer.enable) await api.enableColorasm(uuid);
  }

  // 实例配了启动前 / 后命令时先问一次：跳过由后端记着不跑，取消则中止这次启动。
  // 提醒开关、两条命令有没有配都由后端判断，返回 null 表示这次不用问
  let runCommands: boolean | undefined;
  const launchCommands = await api.launchCommands(uuid).catch(() => null);
  if (launchCommands) {
    const answer = await askLaunchCommand(launchCommands);
    if (answer.action === "cancel") return false;
    if (!answer.notice && answer.action === "run") {
      await saveGuiConfig({ client: { commandNotice: false } }).catch(() => { });
    }
    runCommands = answer.action === "run";
  }

  // 在线服务器包：启动时后端本来会直接按发布地址同步，这里先问一句，别悄悄升级
  const serverPack = await askServerPack(uuid);
  if (serverPack === null) return false;
  const serverPackAction = serverPack?.action;
  const serverPackSkip = serverPack?.skip;

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

  // 账户占用检查：被拦下时问一次，可以继续用当前账户，也可以换一个账户再试。
  // 后端按「当前账户」取登录凭据，换账户要先切过去；切完照常检查新账户
  let force = false;
  for (;;) {
    try {
      await api.launchGame(
        uuid,
        enterWorld,
        force,
        autoSave,
        serverPackAction,
        serverPackSkip,
        runCommands,
        renderdoc,
        nsight,
        jvmDebug,
      );
      break;
    } catch (e) {
      if (String(e) !== "err.accountInUse") throw e;

      const answer = await askAccountLock();
      if (answer.action === "cancel") return false;
      if (!answer.keep) {
        await saveGuiConfig({ client: { accountCheck: false } }).catch(() => { });
      }
      if (answer.action === "switch") {
        // 换账户失败（后端拒绝，如联机大厅锁定）就中止这次启动，提示已经由账户存储弹过
        if (!(await setCurrentAccountByUuid(answer.uuid))) return false;
        force = false;
        continue;
      }
      // 继续启动：这一次先放行；检查关掉后后续启动也不会再被拦
      force = true;
    }
  }

  if (autoShow) {
    openWindow("log", { uuid });
  }
  return true;
}

/** 服务器包这次怎么处理 */
type ServerPackAnswer = {
  /** `update` 先同步再启动，`skip` 直接启动 */
  action: "update" | "skip";
  /** 同步时先不更新的文件，取清单里的相对路径；直接启动时为空 */
  skip: string[];
};

/**
 * 启动前查服务器包有没有新版本，有就问用户
 *
 * 只对填了发布地址的在线服务器包实例查；其它实例直接返回 `undefined`，不发请求。
 * 选「更新并启动」时再问一次可选模组：没勾的这次不下载新版本。
 *
 * - `uuid`: 实例 uuid
 *
 * # 返回值
 *
 * 见 [`ServerPackAnswer`]；用户取消启动返回 `null`，不用问返回 `undefined`
 */
async function askServerPack(uuid: string): Promise<ServerPackAnswer | null | undefined> {
  const list = await api.getInstances().catch(() => []);
  const instance = list.find((item) => item.uuid === uuid);
  if (instance?.modpackType !== "serverpack" || !instance.serverUrl) return undefined;

  // 查不到（地址不通 / 清单坏了）就当没更新：启动本身不该被一次查询挡住
  const result = await modpackCheck(uuid).catch(() => null);
  if (!result?.hasUpdate) return undefined;

  const answer = await askServerPackUpdate(result);
  if (!answer.ok) return null;
  if (!answer.update) return { action: "skip", skip: [] };

  const skip = result.mods.length ? await askServerPackMods(result.mods) : [];
  // 在可选模组那一层取消：当作取消启动，别带一半的选择去同步
  if (skip === null) return null;
  return { action: "update", skip };
}
