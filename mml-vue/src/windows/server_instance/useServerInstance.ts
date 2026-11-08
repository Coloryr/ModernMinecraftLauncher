
// 服务器实例生成窗口：左侧分页 + 右侧内容
//
// 分页：总体设定 / 模组 / 资源包 / 配置文件 / 开始生成。
// 配置文件不是文件树，而是"添加要同步的目录"，每个目录各自选同步方式：
// 压缩包同步是整目录打一个 zip，逐文件下载是目录里的文件一条一条列进清单。
//
// 目标实例按优先级取：openWindow 带入的参数 → server-instance-focus 事件 → 标题栏下拉。
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import type { UnlistenFn } from "@tauri-apps/api/event";
import { api, onServerInstanceFocus, onServerInstanceProgress } from "../../lib/api";
import { t, tErr } from "../../lib/i18n";
import { showToast } from "../../lib/toast";
import { useWindowRefresh } from "../../composables/useWindowRefresh";
import { useInstanceChange } from "../../composables/useInstanceChange";
import { targetUuid, windowParams } from "../windowManager";
import type {
  ExportEntryDto,
  InstanceInfoDto,
  ServerInstanceIdDto,
  ServerInstanceMetaDto,
  ServerInstanceOptionsDto,
  ServerInstanceProgressDto,
  ServerInstanceResultDto,
  ServerInstanceSnapshotDto,
} from "../../lib/bindings";

/** 文件夹的同步方式 */
export type ConfigMode =
  | "none"
  | "archiveAll"
  | "archiveSelf"
  | "filesAll"
  | "filesSelf"
  | "delete";
/** 左侧分页 */
export type Section = "general" | "base" | "mods" | "packs" | "configs" | "start";
/** 编号类型 */
export type IdType = "online" | "custom";

/** 配置树展开后的一行 */
export interface ConfigRow {
  /** 相对游戏目录的路径 */
  path: string;
  /** 显示名，路径最后一段 */
  name: string;
  /** 是否目录 */
  dir: boolean;
  /** 缩进层级，顶层为 0 */
  depth: number;
  /** 文件或整个目录的大小，单位字节 */
  size: number;
  /** 所属顶层目录，用于取同步方式与高亮变化 */
  root: string;
}

/** 模组表格的行高，单位 px；必须与样式里的 .si-mod-row 一致 */
export const MOD_ROW_H = 36;


/** 列表里不当作"配置文件目录"的顶层目录：模组与资源包各有自己的页，其余是运行期产物 */
const SKIP_DIRS = [
  "mods", "resourcepacks", "saves", "logs", "crash-reports", "backups", "backup", "cache",
  ".cache", "temp", "versions", "libraries", "assets", "resources", "natives", "downloads",
];

/** 默认逐文件下发的目录：kubejs 是几千个小脚本，服主要逐个比对，打成压缩包反而没法看 */
const FILES_DIRS = ["kubejs"];

/** 实例根目录里不能当基础文件同步的启动器文件 */
const BASE_SKIP = [
  "game.json",
  "icon.png",
  "server.json",
  "server.json.sha1",
  "server.json.old",
  "server_instance_ids.json",
];
/** 实例根目录里不列出的运行期目录 */
const BASE_DIR_SKIP = ["cache", "temp", "backup"];

/** 服务器实例操作指导 */
const GUIDE_URL =
  "https://github.com/Coloryr/ModernMinecraftLauncher/blob/master/docs/server-instance.md";


export function useServerInstance() {
const instances = ref<InstanceInfoDto[]>([]);
const currentUuid = ref("");
const section = ref<Section>("general");

// ---------------- 总体设定 ----------------

const name = ref("");
const version = ref("1.0.0");
const text = ref("");
const url = ref("");
const folder = ref("");

// ---------------- 勾选 ----------------

/** 实例根目录下、与 .minecraft 同级的文件 */
const baseEntries = ref<ExportEntryDto[]>([]);
/** 勾选的基础文件路径 */
const pickedBase = ref(new Set<string>());
/** 模组目录里的文件 */
const mods = ref<ExportEntryDto[]>([]);
/** 勾选的模组路径 */
const pickedMods = ref(new Set<string>());
/** 标成可选的模组路径：客户端首次同步时装成禁用，玩家可自行启用 */
const optionalMods = ref(new Set<string>());
/** 每个模组的下载地址，路径 → 地址；载入时用实例记录的那份铺底 */
const modUrls = ref(new Map<string, string>());
/** 每个模组手填的项目编号 / 文件编号，路径 → 编号 */
const modPids = ref(new Map<string, string>());
const modFids = ref(new Map<string, string>());
/** 编号被用户改过的模组路径 */
const modIdEdits = ref(new Set<string>());
/** 项目编号被用户改过的模组路径 */
const modPidEdits = ref(new Set<string>());
/** 模组编号类型 */
const modIdTypes = ref(new Map<string, IdType>());
/** 编号类型被用户改过的模组路径 */
const modIdTypeEdits = ref(new Set<string>());
/** 已经生成过、项目编号锁定的路径 */
const lockedProjectIds = ref(new Set<string>());
/** 资源包目录里的文件 */
const packs = ref<ExportEntryDto[]>([]);
/** 勾选的资源包路径 */
const pickedPacks = ref(new Set<string>());
/** 每个资源包手填的下载地址与编号 */
const packUrls = ref(new Map<string, string>());
const packPids = ref(new Map<string, string>());
const packFids = ref(new Map<string, string>());
/** 编号被用户改过的资源包路径 */
const packIdEdits = ref(new Set<string>());
/** 项目编号被用户改过的资源包路径 */
const packPidEdits = ref(new Set<string>());
/** 资源包编号类型 */
const packIdTypes = ref(new Map<string, IdType>());
/** 编号类型被用户改过的资源包路径 */
const packIdTypeEdits = ref(new Set<string>());
/** 可同步的顶层目录，配置树的根 */
const configDirs = ref<ExportEntryDto[]>([]);
/** 目录路径 → 同步方式；没配过的子目录按不同步处理 */
const configModes = ref(new Map<string, ConfigMode>());
/** 取消勾选的文件，相对游戏目录 */
const configExcluded = ref(new Set<string>());
/** 展开后加载到的子项，目录路径 → 直接子项 */
const configChildren = ref(new Map<string, ExportEntryDto[]>());
/** 展开着的目录 */
const configOpened = ref(new Set<string>());
/** 正在读取的目录，防重复请求 */
const configLoading = ref(new Set<string>());

// ---------------- 运行状态 ----------------

const busy = ref(false);
/** 切换实例 / 重扫目录中：内容区盖加载并禁用交互 */
const loading = ref(false);
/** 导出服务器包中；与生成分开，导出时结果区要留着 */
const packing = ref(false);
const progress = ref<ServerInstanceProgressDto | null>(null);
const result = ref<ServerInstanceResultDto | null>(null);
/** 上次生成保存的快照，用来在生成前预览变化 */
const lastSnapshot = ref<ServerInstanceSnapshotDto | null>(null);

/** 按当前勾选与上次快照比较，生成前就能看到模组变化 */
const previewModChanges = computed(() => {
  const snapshot = lastSnapshot.value;
  if (!snapshot?.generated) return null;
  const old = new Map(snapshot.mods.map((item) => [item.path, item.sha1]));
  const current = mods.value.filter((item) => pickedMods.value.has(item.path));
  const added: string[] = [];
  const modified: string[] = [];
  for (const item of current) {
    if (!old.has(item.path)) added.push(item.path);
    else if (old.get(item.path) !== item.sha1) modified.push(item.path);
  }
  const currentPaths = new Set(current.map((item) => item.path));
  const removed = [...old.keys()].filter((path) => !currentPaths.has(path));
  return { added, removed, modified };
});

const previewPackChanges = computed(() => {
  const snapshot = lastSnapshot.value;
  if (!snapshot?.generated) return null;
  const old = new Map(snapshot.packs.map((item) => [item.path, item.sha1]));
  const current = packs.value.filter((item) => pickedPacks.value.has(item.path));
  const added: string[] = [];
  const modified: string[] = [];
  for (const item of current) {
    if (!old.has(item.path)) added.push(item.path);
    else if (old.get(item.path) !== item.sha1) modified.push(item.path);
  }
  const currentPaths = new Set(current.map((item) => item.path));
  const removed = [...old.keys()].filter((path) => !currentPaths.has(path));
  return { added, removed, modified };
});

/** 生成后用后端结果；生成前用快照预览 */
const modChanges = computed(() => result.value?.modChanges ?? previewModChanges.value);
const addedMods = computed(() => new Set(modChanges.value?.added ?? []));
const removedMods = computed(() => new Set(modChanges.value?.removed ?? []));
const modifiedMods = computed(() => new Set(modChanges.value?.modified ?? []));
/** 生成后用后端结果；生成前用快照预览 */
const packChanges = computed(() => result.value?.packChanges ?? previewPackChanges.value);
const addedPacks = computed(() => new Set(packChanges.value?.added ?? []));
const removedPacks = computed(() => new Set(packChanges.value?.removed ?? []));
const modifiedPacks = computed(() => new Set(packChanges.value?.modified ?? []));
/** 与上次生成相比有变化的路径 / 配置目录，用于行高亮 */
const changedPaths = ref(new Set<string>());
const error = ref("");

/** 进度事件取消订阅 */
let unlistenProgress: UnlistenFn | null = null;
/** 切换目标实例事件取消订阅 */
let unlistenFocus: UnlistenFn | null = null;

/** 当前选中的实例 */
const current = computed(() => instances.value.find((i) => i.uuid === currentUuid.value));

/** 左侧分页 */
const sections = computed<Array<{ id: Section; labelKey: string; icon: "gear" | "folder" | "cube" | "package" | "sliders" | "play" }>>(() => [
  { id: "general", labelKey: "serverInstance.tab.general", icon: "gear" },
  { id: "base", labelKey: "serverInstance.tab.base", icon: "folder" },
  { id: "mods", labelKey: "serverInstance.tab.mods", icon: "cube" },
  { id: "packs", labelKey: "serverInstance.tab.packs", icon: "package" },
  { id: "configs", labelKey: "serverInstance.tab.configs", icon: "sliders" },
  { id: "start", labelKey: "serverInstance.tab.start", icon: "play" },
]);

/** 配置树展开后的可见行：目录按需展开，子项由 listExportDir 补上 */
const configRows = computed<ConfigRow[]>(() => {
  const rows: ConfigRow[] = [];
  const walk = (entries: ExportEntryDto[], depth: number, root: string) => {
    for (const entry of entries) {
      rows.push({
        path: entry.path,
        name: entry.path.split("/").pop() ?? entry.path,
        dir: entry.dir,
        depth,
        size: entry.size,
        root: depth === 0 ? entry.path : root,
      });
      if (!entry.dir || !configOpened.value.has(entry.path)) continue;
      // 删除目录的子树不展开：里面既不能选方式，也不能勾文件
      if (configModeOf(entry.path) === "delete") continue;
      walk(configChildren.value.get(entry.path) ?? [], depth + 1, root);
    }
  };
  walk(configDirs.value, 0, "");
  return rows;
});

/** 实际会生成条目的目录数：被上级覆盖的子目录不重复计 */
const configCount = computed(
  () =>
    [...configModes.value].filter(([path, mode]) => mode !== "none" && !configCovered(path)).length,
);

/** 进度百分比；总数未知时返回 -1 */
const percent = computed(() => {
  const p = progress.value;
  if (!p || p.total <= 0) return -1;
  return Math.min(100, Math.max(0, Math.round((p.now / p.total) * 100)));
});

/** 必填项齐不齐 */
const ready = computed(() => !!currentUuid.value && !!name.value.trim() && !!url.value.trim() && !!folder.value.trim());

/** 是否已经选了要打包的东西 */
const pickedCount = computed(
  () =>
    pickedMods.value.size + pickedPacks.value.size + configCount.value + pickedBase.value.size,
);

/** 这个模组当前填的下载地址 */
function urlOf(path: string): string {
  return modUrls.value.get(path) ?? "";
}

/** 编号显示时去掉 custom- 前缀；Custom 标记单独表示类型 */
function displayId(value: string): string {
  return value.replace(/^custom-/, "");
}

/** 这个模组当前的项目编号 / 文件编号 */
function modPidOf(path: string): string {
  return displayId(modPids.value.get(path) ?? "");
}

function modFidOf(path: string): string {
  return displayId(modFids.value.get(path) ?? "");
}

/** 模组当前的编号类型 */
function modIdTypeOf(path: string, fallbackCustom: boolean): IdType {
  return modIdTypes.value.get(path) ?? (fallbackCustom ? "custom" : "online");
}

/** 模组是否按自定义编号处理 */
function isCustomType(path: string, fallbackCustom: boolean): boolean {
  return modIdTypeOf(path, fallbackCustom) === "custom";
}

/** 改模组的编号类型 */
function setModIdType(path: string, event: Event) {
  const value = (event.target as HTMLSelectElement).value as IdType;
  const next = new Map(modIdTypes.value);
  next.set(path, value);
  modIdTypes.value = next;
  modIdTypeEdits.value = new Set([...modIdTypeEdits.value, path]);
}

/** 改某一行的下载地址 */
function setModUrl(path: string, event: Event) {
  const value = (event.target as HTMLInputElement).value;
  const next = new Map(modUrls.value);
  next.set(path, value);
  modUrls.value = next;
}

/** 改模组的项目编号 / 文件编号；记下改过，生成时优先用这份值 */
function setModId(path: string, field: "pid" | "fid", event: Event) {
  const value = (event.target as HTMLInputElement).value;
  const target = field === "pid" ? modPids : modFids;
  const next = new Map(target.value);
  next.set(path, value);
  target.value = next;
  modIdEdits.value = new Set([...modIdEdits.value, path]);
  if (field === "pid") {
    modPidEdits.value = new Set([...modPidEdits.value, path]);
  }
}

/** 资源包当前填的地址与编号 */
function packUrlOf(path: string): string {
  return packUrls.value.get(path) ?? "";
}

function packPidOf(path: string): string {
  return displayId(packPids.value.get(path) ?? "");
}

function packFidOf(path: string): string {
  return displayId(packFids.value.get(path) ?? "");
}

/** 资源包当前的编号类型 */
function packIdTypeOf(path: string, fallbackCustom: boolean): IdType {
  return packIdTypes.value.get(path) ?? (fallbackCustom ? "custom" : "online");
}

/** 资源包是否按自定义编号处理 */
function isPackCustomType(path: string, fallbackCustom: boolean): boolean {
  return packIdTypeOf(path, fallbackCustom) === "custom";
}

/** 改资源包的编号类型 */
function setPackIdType(path: string, event: Event) {
  const value = (event.target as HTMLSelectElement).value as IdType;
  const next = new Map(packIdTypes.value);
  next.set(path, value);
  packIdTypes.value = next;
  packIdTypeEdits.value = new Set([...packIdTypeEdits.value, path]);
}

/** 改资源包的下载地址 */
function setPackUrl(path: string, event: Event) {
  const value = (event.target as HTMLInputElement).value;
  const next = new Map(packUrls.value);
  next.set(path, value);
  packUrls.value = next;
}

/** 改资源包的编号；生成时优先用这份值 */
function setPackId(path: string, field: "pid" | "fid", event: Event) {
  const value = (event.target as HTMLInputElement).value;
  const target = field === "pid" ? packPids : packFids;
  const next = new Map(target.value);
  next.set(path, value);
  target.value = next;
  packIdEdits.value = new Set([...packIdEdits.value, path]);
  if (field === "pid") {
    packPidEdits.value = new Set([...packPidEdits.value, path]);
  }
}

/** 文件名 */
function baseName(path: string): string {
  return path.split("/").pop() ?? path;
}

/** 勾选 / 取消一个路径，返回新集合以触发响应式 */
function toggled(set: Set<string>, path: string): Set<string> {
  const next = new Set(set);
  if (next.has(path)) next.delete(path);
  else next.add(path);
  return next;
}

/** 打开服务器实例操作指导 */
function openGuide() {
  void api.openUrl(GUIDE_URL);
}

/** 全选 / 全不选基础文件 */
function pickAllBase(all: boolean) {
  pickedBase.value = all ? new Set(baseEntries.value.map((item) => item.path)) : new Set();
}

/** 勾选 / 取消一个基础文件 */
function toggleBase(path: string) {
  pickedBase.value = toggled(pickedBase.value, path);
}

/** 这个模组有没有标成可选 */
function optionalOf(path: string): boolean {
  return optionalMods.value.has(path);
}

/** 勾选 / 取消可选标记 */
function toggleModOptional(path: string) {
  optionalMods.value = toggled(optionalMods.value, path);
}

/** 全选 / 全不选模组 */
function pickAllMods(all: boolean) {
  pickedMods.value = all ? new Set(mods.value.map((item) => item.path)) : new Set();
}

/** 全选 / 全不选资源包 */
function pickAllPacks(all: boolean) {
  pickedPacks.value = all ? new Set(packs.value.map((item) => item.path)) : new Set();
}

/** 新目录默认的同步方式：kubejs 这类小脚本目录逐文件更合适 */
function defaultConfigMode(dir: string): ConfigMode {
  return FILES_DIRS.includes(dir) ? "filesAll" : "archiveAll";
}

/** 目录的同步方式；没配过的顶层按默认值，子目录按不同步 */
function configModeOf(path: string): ConfigMode {
  return configModes.value.get(path) ?? (path.includes("/") ? "none" : defaultConfigMode(path));
}

/**
 * 上级目录对当前行的锁定类型
 *
 * - `none`：上级选了不同步，整棵子树都锁成不同步；
 * - `recursive`：上级选了整包 / 整目录，方式跟着上级走；
 * - `delete`：上级要删目录，子树不能展开也不能选；
 * - `null`：没有上级锁定，按自己的方式同步。
 */
function configAncestorLock(path: string): "none" | "recursive" | "delete" | null {
  const parts = path.split("/");
  for (let index = 1; index < parts.length; index += 1) {
    const dir = parts.slice(0, index).join("/");
    const mode = configModeOf(dir);
    if (mode === "none") return "none";
    if (mode === "delete") return "delete";
    if (mode === "archiveAll" || mode === "filesAll") return "recursive";
  }
  return null;
}

/** 是否被上级目录锁定，自己设置的方式不生效 */
function configCovered(path: string): boolean {
  return configAncestorLock(path) !== null;
}

/** 这个目录是否在删除目录的覆盖范围内；在的话不能展开也不能勾文件 */
function configDeleteLocked(path: string): boolean {
  const parts = path.split("/");
  for (let index = 1; index <= parts.length; index += 1) {
    const dir = parts.slice(0, index).join("/");
    if (configModeOf(dir) === "delete") return true;
  }
  return false;
}

/** 文件归属的同步条目；没有任何目录覆盖时返回 null */
function configFileOwner(path: string): { dir: string; mode: ConfigMode } | null {
  const parts = path.split("/");
  const parent = parts.slice(0, -1).join("/");
  for (let index = 1; index < parts.length; index += 1) {
    const dir = parts.slice(0, index).join("/");
    const mode = configModeOf(dir);
    // 上级不同步时整棵子树都不参与，里面的文件没有归属
    if (mode === "none") return null;
    if (mode === "delete") return { dir, mode };
    if (mode === "archiveAll" || mode === "filesAll") return { dir, mode };
    if ((mode === "archiveSelf" || mode === "filesSelf") && dir === parent) {
      return { dir, mode };
    }
  }
  return null;
}

/** 文件勾选状态：没归属、被删除目录锁定、已勾、取消勾选 */
function configFileState(path: string): "on" | "off" | "locked" | "none" {
  const owner = configFileOwner(path);
  if (!owner) return "none";
  if (owner.mode === "delete") return "locked";
  return configExcluded.value.has(path) ? "off" : "on";
}

/** 勾选 / 取消一个文件 */
function setConfigFile(path: string, on: boolean) {
  const next = new Set(configExcluded.value);
  if (on) next.delete(path);
  else next.add(path);
  configExcluded.value = next;
}

/** 点文件名时切换文件勾选；目录名由调用方负责展开 */
function toggleConfigFile(path: string) {
  const state = configFileState(path);
  if (state === "on" || state === "off") setConfigFile(path, state !== "on");
}

/** 改一个目录的同步方式 */
function setConfigMode(dir: string, event: Event) {
  // 被上级锁定的行是禁用状态，这里再挡一次，别让程序调用改掉
  if (configAncestorLock(dir) !== null) return;
  const mode = (event.target as HTMLSelectElement).value as ConfigMode;
  const next = new Map(configModes.value);
  next.set(dir, mode);
  configModes.value = next;
}

/** 全部整包同步 / 全部不同步；清掉子目录各自的设置 */
function setAllConfigs(on: boolean) {
  const next = new Map<string, ConfigMode>();
  for (const entry of configDirs.value) {
    next.set(entry.path, on ? defaultConfigMode(entry.path) : "none");
  }
  configModes.value = next;
}

/** 旧数据里的同步方式转成新值 */
function normalizeConfigMode(value: string): ConfigMode {
  switch (value.trim()) {
    case "archive":
    case "archiveAll":
      return "archiveAll";
    case "archiveSelf":
      return "archiveSelf";
    case "files":
    case "filesAll":
      return "filesAll";
    case "filesSelf":
      return "filesSelf";
    case "delete":
      return "delete";
    default:
      return "none";
  }
}

/** 某个配置目录是否展开着 */
function isConfigOpen(path: string): boolean {
  return configOpened.value.has(path);
}

/** 某个目录是否正在读子项 */
function isConfigLoading(path: string): boolean {
  return configLoading.value.has(path);
}

/** 读一个目录的直接子项，读过的缓存起来 */
async function loadConfigChildren(path: string) {
  if (configChildren.value.has(path) || configLoading.value.has(path)) return;
  const uuid = currentUuid.value;
  if (!uuid) return;
  const pending = new Set(configLoading.value);
  pending.add(path);
  configLoading.value = pending;
  try {
    const list = await api.listExportDir(uuid, path);
    if (uuid !== currentUuid.value) return;
    const next = new Map(configChildren.value);
    next.set(path, list);
    configChildren.value = next;
  } catch (e) {
    showToast(tErr(e));
  } finally {
    const done = new Set(configLoading.value);
    done.delete(path);
    configLoading.value = done;
  }
}

/** 展开 / 收起一个目录，首次展开时读子项 */
async function toggleConfigExpand(path: string) {
  // 删除目录的子树不展开，避免对着马上要被删的内容勾选
  if (configDeleteLocked(path)) return;
  const next = new Set(configOpened.value);
  if (next.has(path)) {
    next.delete(path);
    configOpened.value = next;
    return;
  }
  next.add(path);
  configOpened.value = next;
  await loadConfigChildren(path);
}

/** 加载序号：快速切实例时只认最后一次请求的结果 */
let loadSeq = 0;

/** 重新读一个实例的模组、资源包与顶层目录，勾选与配置列表重置成默认值 */
async function loadInstance(uuid: string) {
  const seq = ++loadSeq;
  loading.value = true;
  try {
  lastSnapshot.value = null;
  result.value = null;
  progress.value = null;
  changedPaths.value = new Set();
  const fallbackName = current.value?.name ?? "";
  // 名字先用实例列表里的值铺上，不等慢的目录扫描
  name.value = fallbackName;

  // 上次生成保存的名字 / 版本 / 说明 / 网址等，跟编号映射在同一份文件里
  const meta: ServerInstanceMetaDto | null = await api
    .getServerInstanceMeta(uuid)
    .catch(() => null);
  if (seq !== loadSeq) return;

  if (meta) {
    name.value = meta.name || fallbackName;
    version.value = meta.version || "1.0.0";
    text.value = meta.text;
    url.value = meta.url;
    folder.value = meta.folder;
  } else {
    version.value = "1.0.0";
    text.value = "";
    url.value = "";
    folder.value = "";
  }

  // 编号与快照都只读同一份小文件，先取完再开始慢的目录扫描
  const savedIds: ServerInstanceIdDto[] = await api
    .getServerInstanceIds(uuid)
    .catch(() => []);
  if (seq !== loadSeq) return;
  const snapshot = await api.getServerInstanceSnapshot(uuid).catch(() => null);
  if (seq !== loadSeq) return;

  let instanceName = "";
  try {
    const info = await api.getExportInfo(uuid);
    if (seq !== loadSeq) return;
    instanceName = info.name;
    mods.value = info.mods.filter((item) => !item.dir);
    pickedMods.value = new Set(mods.value.map((item) => item.path));
    modUrls.value = new Map(mods.value.map((item) => [item.path, item.url ?? ""]));
  } catch (e) {
    if (seq !== loadSeq) return;
    mods.value = [];
    pickedMods.value = new Set();
    modUrls.value = new Map();
    showToast(tErr(e));
  }
  // 没有保存过名字时，用刚读到的实例名补上
  if (!meta?.name) {
    name.value = instanceName || fallbackName;
  }

  const savedById = new Map(savedIds.map((item) => [item.path, item]));
  lockedProjectIds.value = new Set(savedById.keys());
  modPids.value = new Map(
    mods.value.map((item) => [
      item.path,
      item.customId
        ? (savedById.get(item.path)?.projectId ?? item.customProjectId ?? "")
        : (item.modId ?? ""),
    ]),
  );
  modFids.value = new Map(
    mods.value.map((item) => [
      item.path,
      item.customId
        ? (savedById.get(item.path)?.fileId ?? item.customFileId ?? "")
        : (item.fileId ?? ""),
    ]),
  );
  modIdEdits.value = new Set();
  modPidEdits.value = new Set();
  modIdTypes.value = new Map(
    mods.value.map((item) => [item.path, item.customId ? "custom" : "online"]),
  );
  modIdTypeEdits.value = new Set();

  let packsNext: ExportEntryDto[] = [];
  try {
    packsNext = (await api.listExportDir(uuid, "resourcepacks")).filter((item) => !item.dir);
  } catch {
    // 没有 resourcepacks 目录就是空表
  }
  if (seq !== loadSeq) return;
  packs.value = packsNext;
  pickedPacks.value = new Set(packs.value.map((item) => item.path));
  packUrls.value = new Map(packs.value.map((item) => [item.path, item.url ?? ""]));
  packPids.value = new Map(
    packs.value.map((item) => [
      item.path,
      item.customId
        ? (savedById.get(item.path)?.projectId ?? item.customProjectId ?? "")
        : (item.modId ?? ""),
    ]),
  );
  packFids.value = new Map(
    packs.value.map((item) => [
      item.path,
      item.customId
        ? (savedById.get(item.path)?.fileId ?? item.customFileId ?? "")
        : (item.fileId ?? ""),
    ]),
  );
  packIdEdits.value = new Set();
  packPidEdits.value = new Set();
  packIdTypes.value = new Map(
    packs.value.map((item) => [item.path, item.customId ? "custom" : "online"]),
  );
  packIdTypeEdits.value = new Set();
  // 等两张表都填好再启用快照预览，避免资源包加载前闪一下全部删除
  lastSnapshot.value = snapshot;
  // 可选标记跟着上次生成走：生成窗口关掉再开，服务端发布者的选择要还在
  optionalMods.value = new Set(
    (snapshot?.mods ?? []).filter((item) => item.optional).map((item) => item.path),
  );

  let dirsNext: ExportEntryDto[] = [];
  try {
    // 顶层目录也要递归统计大小，树里才有文件夹体积
    dirsNext = await api.listExportDirSized(uuid, "");
  } catch {
    // 读不到就当没有可同步的目录
  }
  if (seq !== loadSeq) return;
  let baseNext: ExportEntryDto[] = [];
  try {
    baseNext = await api.listExportInstanceDir(uuid, "");
  } catch {
    // 实例根目录读不到就当没有基础文件
  }
  if (seq !== loadSeq) return;
  baseEntries.value = baseNext.filter((entry) => {
    // 顶层目录都可以整目录带上，但缓存 / 临时 / 备份目录不列
    if (entry.dir) return !BASE_DIR_SKIP.includes(entry.path);
    return !entry.path.includes("/") && !BASE_SKIP.includes(entry.path);
  });
  // 保存的是勾选时的相对路径，可能展开到了 .minecraft 里面，不能按根条目过滤
  pickedBase.value = new Set(meta?.baseFiles ?? []);

  configDirs.value = dirsNext.filter((entry) => entry.dir && !SKIP_DIRS.includes(entry.path));
  configChildren.value = new Map();
  configOpened.value = new Set();
  configLoading.value = new Set();

  const modes = new Map<string, ConfigMode>();
  if (Array.isArray(meta?.configs)) {
    // 生成过的实例按保存的方式恢复；没保存过的顶层目录保持不同步
    for (const entry of configDirs.value) modes.set(entry.path, "none");
    const roots = new Set(configDirs.value.map((entry) => entry.path));
    const restored = await Promise.all(
      meta.configs.map(async (item) => {
        const dir = item.dir.trim().replace(/\\/g, "/");
        if (!dir) return null;
        // 只恢复配置树里列得出来的目录：模组 / 日志这类运行目录不参与同步
        if (![...roots].some((root) => dir === root || dir.startsWith(`${root}/`))) return null;
        // 子目录可能已经被删掉：读不到就不恢复，免得生成时指着一个不存在的目录
        if (!roots.has(dir)) {
          try {
            await api.listExportDir(uuid, dir);
          } catch {
            return null;
          }
        }
        return [dir, normalizeConfigMode(item.mode)] as const;
      }),
    );
    if (seq !== loadSeq) return;
    for (const entry of restored) {
      if (entry) modes.set(entry[0], entry[1]);
    }
  } else {
    // 首次打开沿用老口径：顶层目录默认整包同步，kubejs 这类走逐文件
    for (const entry of configDirs.value) modes.set(entry.path, defaultConfigMode(entry.path));
  }
  configModes.value = modes;
  // 取消勾选跟着上次生成走：发布者只排掉个别文件的设置要还在
  const excluded = new Set<string>();
  if (Array.isArray(meta?.configs)) {
    for (const item of meta.configs) {
      for (const path of item.exclude) excluded.add(path);
    }
  }
  configExcluded.value = excluded;
  result.value = null;
  progress.value = null;
  changedPaths.value = new Set();
  error.value = "";
  } finally {
    if (seq === loadSeq) loading.value = false;
  }
}

/** 重新扫描模组：保留已有勾选与手填内容，新出现的文件默认勾上 */
async function refreshMods() {
  const uuid = currentUuid.value;
  if (!uuid || busy.value || loading.value) return;
  loading.value = true;
  try {
    const info = await api.getExportInfo(uuid);
    if (uuid !== currentUuid.value) return;
    const next = info.mods.filter((item) => !item.dir);
    const oldPaths = new Set(mods.value.map((item) => item.path));
    const nextPaths = new Set(next.map((item) => item.path));
    pickedMods.value = new Set(
      next
        .filter((item) => !oldPaths.has(item.path) || pickedMods.value.has(item.path))
        .map((item) => item.path),
    );
    for (const item of next) {
      if (oldPaths.has(item.path)) continue;
      modUrls.value.set(item.path, item.url ?? "");
      modPids.value.set(
        item.path,
        item.customId ? (item.customProjectId ?? "") : (item.modId ?? ""),
      );
      modFids.value.set(
        item.path,
        item.customId ? (item.customFileId ?? "") : (item.fileId ?? ""),
      );
      modIdTypes.value.set(item.path, item.customId ? "custom" : "online");
    }
    for (const path of [...modUrls.value.keys()]) {
      if (!nextPaths.has(path)) modUrls.value.delete(path);
    }
    for (const path of [...modPids.value.keys()]) {
      if (!nextPaths.has(path)) modPids.value.delete(path);
    }
    for (const path of [...modFids.value.keys()]) {
      if (!nextPaths.has(path)) modFids.value.delete(path);
    }
    for (const path of [...modIdTypes.value.keys()]) {
      if (!nextPaths.has(path)) modIdTypes.value.delete(path);
    }
    mods.value = next;
    result.value = null;
    changedPaths.value = new Set();
  } catch (e) {
    showToast(tErr(e));
  } finally {
    loading.value = false;
  }
}

/** 重新扫描资源包：口径与模组相同 */
async function refreshPacks() {
  const uuid = currentUuid.value;
  if (!uuid || busy.value || loading.value) return;
  loading.value = true;
  try {
    const next = (await api.listExportDir(uuid, "resourcepacks")).filter((item) => !item.dir);
    if (uuid !== currentUuid.value) return;
    const oldPaths = new Set(packs.value.map((item) => item.path));
    const nextPaths = new Set(next.map((item) => item.path));
    pickedPacks.value = new Set(
      next
        .filter((item) => !oldPaths.has(item.path) || pickedPacks.value.has(item.path))
        .map((item) => item.path),
    );
    for (const item of next) {
      if (oldPaths.has(item.path)) continue;
      packUrls.value.set(item.path, item.url ?? "");
      packPids.value.set(
        item.path,
        item.customId ? (item.customProjectId ?? "") : (item.modId ?? ""),
      );
      packFids.value.set(
        item.path,
        item.customId ? (item.customFileId ?? "") : (item.fileId ?? ""),
      );
      packIdTypes.value.set(item.path, item.customId ? "custom" : "online");
    }
    for (const path of [...packUrls.value.keys()]) {
      if (!nextPaths.has(path)) packUrls.value.delete(path);
    }
    for (const path of [...packPids.value.keys()]) {
      if (!nextPaths.has(path)) packPids.value.delete(path);
    }
    for (const path of [...packFids.value.keys()]) {
      if (!nextPaths.has(path)) packFids.value.delete(path);
    }
    for (const path of [...packIdTypes.value.keys()]) {
      if (!nextPaths.has(path)) packIdTypes.value.delete(path);
    }
    packs.value = next;
    result.value = null;
    changedPaths.value = new Set();
  } catch (e) {
    showToast(tErr(e));
  } finally {
    loading.value = false;
  }
}

/** 切到某个实例 */
function focusInstance(uuid: string) {
  if (!instances.value.some((i) => i.uuid === uuid)) return;
  const changed = currentUuid.value !== uuid;
  currentUuid.value = uuid;
  const inst = instances.value.find((i) => i.uuid === uuid);
  if (inst) name.value = inst.name;
  // 切回来还是同一个实例时不重铺，别把用户刚挑好的东西清掉
  if (changed) void loadInstance(uuid);
}

/** 下拉换实例 */
function pickInstance(uuid: string) {
  // 选了同一个实例不用重铺：用户刚挑好的模组 / 配置目录不能被清掉
  if (currentUuid.value === uuid) return;
  currentUuid.value = uuid;
  const inst = current.value;
  if (inst) name.value = inst.name;
  void loadInstance(uuid);
}

/** 定位到 openWindow 带入的目标实例 */
function applyTarget() {
  const uuid = targetUuid();
  if (uuid && instances.value.some((i) => i.uuid === uuid)) focusInstance(uuid);
}

/** 清掉目标实例与它带出来的勾选：实例没了，界面上不能留着上一份快照 */
function clearInstance() {
  // 在途的加载结果作废，否则它回来又会把已删实例的数据铺上
  loadSeq += 1;
  currentUuid.value = "";
  name.value = "";
  mods.value = [];
  baseEntries.value = [];
  pickedBase.value = new Set();
  pickedMods.value = new Set();
  optionalMods.value = new Set();
  modUrls.value = new Map();
  modPids.value = new Map();
  modFids.value = new Map();
  modIdEdits.value = new Set();
  modPidEdits.value = new Set();
  modIdTypes.value = new Map();
  modIdTypeEdits.value = new Set();
  lockedProjectIds.value = new Set();
  packs.value = [];
  pickedPacks.value = new Set();
  packUrls.value = new Map();
  packPids.value = new Map();
  packFids.value = new Map();
  packIdEdits.value = new Set();
  packPidEdits.value = new Set();
  packIdTypes.value = new Map();
  packIdTypeEdits.value = new Set();
  configModes.value = new Map();
  configExcluded.value = new Set();
  configDirs.value = [];
  configChildren.value = new Map();
  configOpened.value = new Set();
  configLoading.value = new Set();
  result.value = null;
  lastSnapshot.value = null;
  progress.value = null;
  changedPaths.value = new Set();
  error.value = "";
}

/** 重新拉实例列表并定位目标实例；当前实例被删掉时落到第一个 */
async function syncInstances() {
  try {
    // 服务器实例本身已经是同步结果，不能再当源实例导出
    instances.value = (await api.getInstances()).filter((item) => item.modpackType !== "serverpack");
  } catch (e) {
    console.error("[ServerInstanceWindow] 读取实例列表失败", e);
    return;
  }
  // 当前实例可能在别的窗口里被删掉了：先清空再重新定位，别留一个不存在的选中项
  if (currentUuid.value && !instances.value.some((i) => i.uuid === currentUuid.value)) {
    clearInstance();
  }
  const uuid = targetUuid();
  if (uuid && instances.value.some((i) => i.uuid === uuid)) {
    focusInstance(uuid);
    return;
  }
  if (!currentUuid.value && instances.value.length) focusInstance(instances.value[0].uuid);
}

/** 选一个输出文件夹 */
async function pickFolder() {
  try {
    const { open } = await import("@tauri-apps/plugin-dialog");
    const picked = await open({
      title: t("serverInstance.folder"),
      directory: true,
      multiple: false,
    });
    if (typeof picked === "string") folder.value = picked;
  } catch (e) {
    showToast(tErr(e));
  }
}

/** 生成成功后重读已保存编号：新条目从这一轮开始锁项目编号 */
async function refreshSavedIds() {
  if (!currentUuid.value) return;
  try {
    const list = await api.getServerInstanceIds(currentUuid.value);
    const byPath = new Map(list.map((item) => [item.path, item]));
    lockedProjectIds.value = new Set(byPath.keys());
    modPids.value = new Map(
      mods.value.map((item) => [
        item.path,
        item.customId
          ? (byPath.get(item.path)?.projectId ?? modPids.value.get(item.path) ?? "")
          : (modPids.value.get(item.path) ?? item.modId ?? ""),
      ]),
    );
    modFids.value = new Map(
      mods.value.map((item) => [
        item.path,
        item.customId
          ? (byPath.get(item.path)?.fileId ?? modFids.value.get(item.path) ?? "")
          : (modFids.value.get(item.path) ?? item.fileId ?? ""),
      ]),
    );
    packPids.value = new Map(
      packs.value.map((item) => [
        item.path,
        item.customId
          ? (byPath.get(item.path)?.projectId ?? packPids.value.get(item.path) ?? "")
          : (packPids.value.get(item.path) ?? item.modId ?? ""),
      ]),
    );
    packFids.value = new Map(
      packs.value.map((item) => [
        item.path,
        item.customId
          ? (byPath.get(item.path)?.fileId ?? packFids.value.get(item.path) ?? "")
          : (packFids.value.get(item.path) ?? item.fileId ?? ""),
      ]),
    );
  } catch {
    // 读不到就保留当前值；下一轮生成仍由后端锁定项目编号
  }
}

/** 检查当前表格里是否有重复的自定义项目编号 */
function hasDuplicateProjectId(): boolean {
  const seen = new Map<string, boolean>();
  for (const row of mods.value) {
    if (!isCustomType(row.path, row.customId)) continue;
    const projectId = displayId((modPids.value.get(row.path) ?? "").trim());
    if (!projectId) continue;
    const edited = modPidEdits.value.has(row.path);
    if (seen.has(projectId)) {
      if (edited || seen.get(projectId)) return true;
    } else {
      seen.set(projectId, edited);
    }
  }
  for (const row of packs.value) {
    if (!isPackCustomType(row.path, row.customId)) continue;
    const projectId = displayId((packPids.value.get(row.path) ?? "").trim());
    if (!projectId) continue;
    const edited = packPidEdits.value.has(row.path);
    if (seen.has(projectId)) {
      if (edited || seen.get(projectId)) return true;
    } else {
      seen.set(projectId, edited);
    }
  }
  return false;
}

/**
 * 按当前填表与勾选拼出生成选项，生成与导出服务器包共用
 *
 * # 返回值
 *
 * 生成选项，见 `ServerInstanceOptionsDto`
 */
function buildOptions(): ServerInstanceOptionsDto {
  return {
    folder: folder.value.trim(),
    url: url.value.trim(),
    name: name.value.trim(),
    version: version.value.trim(),
    text: text.value.trim(),
    mods: [...pickedMods.value].map((path) => {
      const edited = modIdEdits.value.has(path);
      const row = mods.value.find((item) => item.path === path);
      return {
        path,
        exclude: false,
        url: urlOf(path),
        pid: edited ? modPidOf(path) : "",
        fid: edited ? modFidOf(path) : "",
        custom: isCustomType(path, row?.customId ?? false),
        edited,
        typeChanged: modIdTypeEdits.value.has(path),
        optional: optionalMods.value.has(path),
      };
    }),
    // 被上级递归方式覆盖的子目录不重复下发，交给上级那一条
    configs: [...configModes.value]
      .filter(([path, mode]) => mode !== "none" && !configCovered(path))
      .map(([dir, mode]) => ({
        dir,
        mode,
        // 只把这条目录负责的范围里的取消勾选传下去，子目录自己的条目另算
        exclude: [...configExcluded.value].filter(
          (path) => path === dir || path.startsWith(`${dir}/`),
        ),
      })),
    baseFiles: [...pickedBase.value],
    resourcePacks: packs.value
      .filter((row) => pickedPacks.value.has(row.path))
      .map((row) => {
        const edited = packIdEdits.value.has(row.path);
        return {
          path: row.path,
          exclude: false,
          url: packUrlOf(row.path),
          pid: edited ? packPidOf(row.path) : "",
          fid: edited ? packFidOf(row.path) : "",
          custom: isPackCustomType(row.path, row.customId),
          edited,
          typeChanged: packIdTypeEdits.value.has(row.path),
          // 可选只对模组生效，资源包一律照常启用
          optional: false,
        };
      }),
  };
}

/** 开始生成，进度经 server-instance-progress 上报 */
async function generate() {
  if (busy.value || packing.value || !ready.value) return;
  if (hasDuplicateProjectId()) {
    error.value = t("err.serverInstanceProjectIdDuplicate");
    showToast(error.value);
    return;
  }
  busy.value = true;
  error.value = "";
  result.value = null;
  progress.value = null;
  changedPaths.value = new Set();
  section.value = "start";
  try {
    result.value = await api.generateServerInstance(currentUuid.value, buildOptions());
    changedPaths.value = new Set(result.value.changed);
    await refreshSavedIds();
    const snapshot = await api.getServerInstanceSnapshot(currentUuid.value).catch(() => null);
    if (snapshot) lastSnapshot.value = snapshot;
    showToast(t("serverInstance.done", { path: folder.value.trim() }));
  } catch (e) {
    error.value = tErr(e);
  } finally {
    busy.value = false;
  }
}

/**
 * 把当前实例导出成服务器包 zip
 *
 * 导出前要先生成过一次：包里会附带生成出来的 `server.json`，没有时后端直接报错。
 *
 * - `complete`: 为 true 时导出完整服务器包，否则只带基础文件
 */
async function exportPack(complete: boolean) {
  if (busy.value || packing.value || !ready.value) return;
  let picked: string | null;
  try {
    const { save } = await import("@tauri-apps/plugin-dialog");
    picked = await save({
      title: t("serverInstance.packSaveTitle"),
      defaultPath: `${name.value || "server"}-${complete ? "full" : "server"}.zip`,
      filters: [{ name: "zip", extensions: ["zip"] }],
    });
  } catch (e) {
    showToast(tErr(e));
    return;
  }
  if (!picked) return;
  packing.value = true;
  progress.value = null;
  try {
    await api.exportServerPack(currentUuid.value, buildOptions(), picked, complete);
    showToast(t("serverInstance.packSaved", { file: picked }));
  } catch (e) {
    showToast(tErr(e));
  } finally {
    packing.value = false;
  }
}

// 单窗口模式：本页被 KeepAlive 缓存，切回不会重新挂载，自己补一次列表与定位
useWindowRefresh(() => {
  if (!busy.value) {
    result.value = null;
    progress.value = null;
  }
  return syncInstances();
});

// 实例在别的窗口里被增删改（删掉的那个）→ 下拉与勾选跟着变
useInstanceChange(syncInstances);

// 已经停在本窗口时又被打开：openWindow 只更新参数、不换组件，跟着参数换实例
watch(() => windowParams.value.uuid, applyTarget);

// 生成后改了勾选，旧结果就不再代表当前选择；清掉后走快照预览
watch([pickedMods, pickedPacks], () => {
  if (result.value) {
    result.value = null;
    changedPaths.value = new Set();
  }
});

onMounted(async () => {
  await syncInstances();

  unlistenProgress = await onServerInstanceProgress((p) => {
    progress.value = p;
  });
  unlistenFocus = await onServerInstanceFocus((uuid) => {
    if (instances.value.some((i) => i.uuid === uuid)) focusInstance(uuid);
  });
});

onBeforeUnmount(() => {
  unlistenProgress?.();
  unlistenFocus?.();
});

  return {
    instances, currentUuid, section, sections, busy, loading, refreshMods, refreshPacks,
    name, version, text, url, folder, pickFolder,
    baseEntries, pickedBase, pickAllBase, toggleBase,
    mods, pickedMods, pickAllMods, toggled, modPidOf, modFidOf, setModId, baseName,
    modIdTypeOf, isCustomType, setModIdType,
    optionalMods, optionalOf, toggleModOptional,
    urlOf, setModUrl, lockedProjectIds, changedPaths,
    packs, pickedPacks, pickAllPacks, packPidOf, packFidOf, packUrlOf,
    setPackId, setPackUrl, packIdTypeOf, isPackCustomType, setPackIdType,
    configDirs, configModes, configRows, configCount, configModeOf, configAncestorLock,
    configDeleteLocked, configFileState, setConfigFile, toggleConfigFile,
    setAllConfigs, setConfigMode,
    toggleConfigExpand, isConfigOpen, isConfigLoading,
    pickInstance, openGuide,
    current, ready, pickedCount, generate, exportPack, packing, percent, progress, result, error,
    modChanges, addedMods, removedMods, modifiedMods,
    packChanges, addedPacks, removedPacks, modifiedPacks,
  };
}
