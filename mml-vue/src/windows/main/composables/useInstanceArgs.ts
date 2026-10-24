// 主窗口的实例启动参数：内存 / JVM 与游戏参数 / 服务器地址 / 代理等，经 IPC 读写核心实例配置。
//
// 两条约定：
// - `argsOf` 对**尚未加载**的实例返回一份骨架默认值并塞进 argsMap，模板可以直接读、
//   不必到处判空；真实值由 `loadArgs` 回来后覆盖。
// - 编辑走**防抖写回**：每个按键都写盘会产生大量 I/O，窗口内只发最后一次。
import { ref, watch, type Ref } from "vue";

import { api } from "../../../lib/api";
import { t, tErr } from "../../../lib/i18n";
import { showToast } from "../../../lib/toast";
import type { InstanceArgsDto, InstanceInfoDto } from "../../../lib/bindings";

/** 写回后端的防抖窗口，单位 ms */
const SAVE_DEBOUNCE = 600;

/** 未加载时的骨架默认值，字段与后端 InstanceArgsDto 一一对应 */
const ARGS_DEFAULTS: InstanceArgsDto = {
  memory: 4096,
  minMemory: 512,
  fullscreen: false,
  width: 1280,
  height: 720,
  javaName: "",
  javaPath: "",
  gc: "auto",
  gcCustom: "",
  mainClass: "",
  jvmArgs: [],
  gameArgs: [],
  classPath: [],
  envVars: [],
  isolateHome: true,
  lang: "zh_cn",
  logEncoding: "utf8",
  preEnabled: false,
  preCmd: "",
  postEnabled: false,
  postCmd: "",
  proxyIp: "",
  proxyPort: 1080,
  proxyUser: "",
  proxyPass: "",
  serverIp: "",
  serverPort: 25565,
  joinServer: false,
};

/**
 * 主窗口的实例启动参数状态
 *
 * - `selected`: 当前选中实例；参数按实例 uuid 分别缓存
 *
 * # 返回值
 *
 * - `argsMap`: 实例 uuid → 启动参数
 * - `argsLoaded`: 实例 uuid → 是否已从后端拉到真实值
 * - `argsOf`: 取某实例的参数，未加载时先落一份骨架默认值
 * - `loadArgs`: 拉取某实例的参数并覆盖骨架值
 * - `updateArgs`: 更新参数并防抖写回后端
 * - `patchArgs`: 只改部分字段，内部转 `updateArgs`
 * - `onServerIp` / `onServerJoin`: 服务器地址与"启动时加入"两个输入的写入口
 */
export function useInstanceArgs(opts: { selected: Ref<InstanceInfoDto | null> }) {
  const { selected } = opts;

  const argsMap = ref<Record<string, InstanceArgsDto>>({});
  const argsLoaded = ref<Record<string, boolean>>({});

  /** 取实例参数（未加载则先落一份骨架默认值） */
  function argsOf(uuid: string): InstanceArgsDto {
    if (!argsMap.value[uuid]) {
      argsMap.value[uuid] = { ...ARGS_DEFAULTS };
    }
    return argsMap.value[uuid];
  }

  async function loadArgs(uuid: string) {
    try {
      argsMap.value[uuid] = await api.getInstanceArgs(uuid);
      argsLoaded.value[uuid] = true;
    } catch {
      showToast(t("args.loadFailed"));
    }
  }

  // 切换选中实例后拉取该实例的启动参数
  watch(
    () => selected.value?.uuid,
    (uuid) => {
      if (uuid && !argsLoaded.value[uuid]) loadArgs(uuid);
    },
    { immediate: true },
  );

  let argsSaveTimer: ReturnType<typeof setTimeout> | null = null;

  /**
   * 更新参数并防抖写回后端。
   *
   * 注意**故意不做卸载清理**：窗口/页面关掉时若还有待写回的一次编辑，
   * 让定时器照常跑完才是对的 —— 清掉它就等于把用户刚改的设置丢了。
   */
  function updateArgs(v: InstanceArgsDto) {
    const uuid = selected.value?.uuid;
    if (!uuid) return;
    argsMap.value[uuid] = v;
    if (argsSaveTimer) clearTimeout(argsSaveTimer);
    argsSaveTimer = setTimeout(() => {
      api.updateInstanceArgs(uuid, argsMap.value[uuid]).catch((e) => showToast(tErr(e)));
    }, SAVE_DEBOUNCE);
  }

  function patchArgs(patch: Partial<InstanceArgsDto>) {
    if (selected.value) updateArgs({ ...argsOf(selected.value.uuid), ...patch });
  }

  function onServerIp(value: string) {
    patchArgs({ serverIp: value });
  }

  function onServerJoin(checked: boolean) {
    patchArgs({ joinServer: checked });
  }

  return {
    argsMap,
    argsLoaded,
    argsOf,
    loadArgs,
    updateArgs,
    patchArgs,
    onServerIp,
    onServerJoin,
  };
}
