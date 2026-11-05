
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
  ServerInstanceProgressDto,
  ServerInstanceResultDto,
} from "../../lib/bindings";

/** 生成格式：mml 写 server.json，hmcl 写 server-manifest.json */
export type Format = "mml" | "hmcl";
/** 配置目录的同步方式 */
export type ConfigMode = "archive" | "files";
/** 左侧分页 */
export type Section = "general" | "base" | "mods" | "packs" | "configs" | "start";
/** 编号类型 */
export type IdType = "online" | "custom";

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

const format = ref<Format>("mml");
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
/** 已添加的配置目录、同步方式与压缩包是否先删旧目录 */
const configs = ref<Array<{ dir: string; mode: ConfigMode; deleteOld: boolean }>>([]);
/** 游戏目录下的顶层目录与一级子目录，供"添加"下拉用 */
const topDirs = ref<ExportEntryDto[]>([]);
/** 下拉里当前选中的目录 */
const newDir = ref("");
/** 新添加目录默认的同步方式 */
const newMode = ref<ConfigMode>("archive");

// ---------------- 运行状态 ----------------

const busy = ref(false);
const progress = ref<ServerInstanceProgressDto | null>(null);
const result = ref<ServerInstanceResultDto | null>(null);
/** 上次生成后模组的新增 / 删除 / 修改汇总 */
const modChanges = computed(() => result.value?.modChanges ?? null);
const addedMods = computed(() => new Set(modChanges.value?.added ?? []));
const removedMods = computed(() => new Set(modChanges.value?.removed ?? []));
const modifiedMods = computed(() => new Set(modChanges.value?.modified ?? []));
/** 上次生成后资源包的新增 / 删除 / 修改汇总 */
const packChanges = computed(() => result.value?.packChanges ?? null);
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

/** 格式选项 */
const formatOptions = computed(() => [
  { value: "mml", label: t("serverInstance.mml") },
  { value: "hmcl", label: t("serverInstance.hmcl") },
]);

/** 当前格式的说明 */
const hint = computed(() => t(`serverInstance.hint.${format.value}`));

/** 还没加进列表的顶层目录 */
const dirChoices = computed(() =>
  topDirs.value.filter(
    (entry) => entry.dir && !SKIP_DIRS.includes(entry.path) && !configs.value.some((c) => c.dir === entry.path),
  ),
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
    pickedMods.value.size +
    pickedPacks.value.size +
    configs.value.length +
    (format.value === "mml" ? pickedBase.value.size : 0),
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

/** 文件大小，按档显示 */
function sizeText(size: number): string {
  if (size < 1024) return `${size} B`;
  if (size < 1024 * 1024) return `${(size / 1024).toFixed(1)} KB`;
  return `${(size / 1024 / 1024).toFixed(1)} MB`;
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

/** 全选 / 全不选模组 */
function pickAllMods(all: boolean) {
  pickedMods.value = all ? new Set(mods.value.map((item) => item.path)) : new Set();
}

/** 全选 / 全不选资源包 */
function pickAllPacks(all: boolean) {
  pickedPacks.value = all ? new Set(packs.value.map((item) => item.path)) : new Set();
}

/** 把下拉里选中的目录加进配置列表 */
function addConfig() {
  const dir = newDir.value;
  if (!dir || configs.value.some((item) => item.dir === dir)) return;
  const mode = FILES_DIRS.includes(dir) ? "files" : newMode.value;
  configs.value = [...configs.value, { dir, mode, deleteOld: true }];
  newDir.value = "";
}

/** 移除一个配置目录 */
function removeConfig(dir: string) {
  configs.value = configs.value.filter((item) => item.dir !== dir);
}

/** 改一行的同步方式 */
function setConfigMode(dir: string, mode: ConfigMode) {
  configs.value = configs.value.map((item) => (item.dir === dir ? { ...item, mode } : item));
}

/** 改一行「删除原有目录」；只对压缩包同步有效 */
function setConfigDeleteOld(dir: string, event: Event) {
  const deleteOld = (event.target as HTMLInputElement).checked;
  configs.value = configs.value.map((item) => (item.dir === dir ? { ...item, deleteOld } : item));
}

/**
 * 读取每个可同步顶层目录的一级子目录，只下钻一层
 *
 * - `uuid`: 实例 uuid
 * - `dirs`: 顶层目录列表
 *
 * # 返回值
 *
 * 一级子目录条目，路径形如 `config/setting`
 */
async function loadConfigChildren(uuid: string, dirs: ExportEntryDto[]): Promise<ExportEntryDto[]> {
  const parents = dirs.filter((entry) => entry.dir && !SKIP_DIRS.includes(entry.path));
  const lists = await Promise.all(
    parents.map(async (entry): Promise<ExportEntryDto[]> => {
      try {
        return await api.listExportDir(uuid, entry.path);
      } catch {
        // 某个目录读不到就只少它的子目录，不影响其它目录
        return [];
      }
    }),
  );
  return lists.flat().filter((entry) => entry.dir);
}

/** 加载序号：快速切实例时只认最后一次请求的结果 */
let loadSeq = 0;

/** 重新读一个实例的模组、资源包与顶层目录，勾选与配置列表重置成默认值 */
async function loadInstance(uuid: string) {
  const seq = ++loadSeq;
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

  // 上次生成保存的名字 / 版本 / 说明 / 网址等，跟编号映射在同一份文件里
  let meta: ServerInstanceMetaDto | null = null;
  try {
    meta = await api.getServerInstanceMeta(uuid);
  } catch {
    // 没有生成过或读不到就按默认值铺
  }
  let savedIds: ServerInstanceIdDto[] = [];
  try {
    savedIds = await api.getServerInstanceIds(uuid);
  } catch {
    // 没有生成过或读不到就按当前来源铺
  }
  if (seq !== loadSeq) return;
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

  const fallbackName = instanceName || current.value?.name || "";
  if (meta) {
    format.value = meta.format === "hmcl" ? "hmcl" : "mml";
    name.value = meta.name || fallbackName;
    version.value = meta.version || "1.0.0";
    text.value = meta.text;
    url.value = meta.url;
    folder.value = meta.folder;
  } else {
    format.value = "mml";
    name.value = fallbackName;
    version.value = "1.0.0";
    text.value = "";
    url.value = "";
    folder.value = "";
  }

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

  let dirsNext: ExportEntryDto[] = [];
  try {
    dirsNext = await api.listExportDir(uuid, "");
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
  const savedBase = new Set(meta?.baseFiles ?? []);
  pickedBase.value = new Set(
    baseEntries.value
      .map((entry) => entry.path)
      .filter((path) => savedBase.has(path)),
  );

  const children = await loadConfigChildren(uuid, dirsNext);
  if (seq !== loadSeq) return;
  topDirs.value = [...dirsNext, ...children];

  // 默认只把顶层文件夹加上，一级子目录按需手动添加
  configs.value = topDirs.value
    .filter((entry) => entry.dir && !entry.path.includes("/") && !SKIP_DIRS.includes(entry.path))
    .map((entry) => ({
      dir: entry.path,
      mode: (FILES_DIRS.includes(entry.path) ? "files" : "archive") as ConfigMode,
      deleteOld: true,
    }));
  newDir.value = "";
  newMode.value = "archive";
  result.value = null;
  progress.value = null;
  changedPaths.value = new Set();
  error.value = "";
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
  configs.value = [];
  topDirs.value = [];
  newDir.value = "";
  result.value = null;
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

/** 开始生成，进度经 server-instance-progress 上报 */
async function generate() {
  if (busy.value || !ready.value) return;
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
    result.value = await api.generateServerInstance(currentUuid.value, {
      format: format.value,
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
        };
      }),
      configs: configs.value.map((item) => ({
        dir: item.dir,
        mode: item.mode,
        deleteOld: item.deleteOld,
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
          };
        }),
    });
    changedPaths.value = new Set(result.value.changed);
    await refreshSavedIds();
    showToast(t("serverInstance.done", { path: folder.value.trim() }));
  } catch (e) {
    error.value = tErr(e);
  } finally {
    busy.value = false;
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

// 下拉换目录时把同步方式拨到该目录的常见做法，用户仍可自己改
watch(newDir, (dir) => {
  if (dir) newMode.value = FILES_DIRS.includes(dir) ? "files" : "archive";
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
    instances, currentUuid, section, sections, busy,
    format, formatOptions, name, version, text, url, folder, hint, pickFolder,
    baseEntries, pickedBase, pickAllBase, toggleBase,
    mods, pickedMods, pickAllMods, toggled, modPidOf, modFidOf, setModId, baseName,
    modIdTypeOf, isCustomType, setModIdType,
    urlOf, setModUrl, lockedProjectIds, changedPaths,
    packs, pickedPacks, pickAllPacks, packPidOf, packFidOf, packUrlOf,
    setPackId, setPackUrl, packIdTypeOf, isPackCustomType, setPackIdType,
    configs, newDir, newMode, dirChoices, addConfig, setConfigMode,
    setConfigDeleteOld, removeConfig,
    pickInstance, openGuide, sizeText,
    current, ready, pickedCount, generate, percent, progress, result, error,
    modChanges, addedMods, removedMods, modifiedMods,
    packChanges, addedPacks, removedPacks, modifiedPacks,
  };
}
