// 窗口管理器：负责窗口打开 / 关闭、单窗口 / 多窗口模式切换
//
// 多窗口模式：
// - Tauri 环境：统一调用 Rust 窗口管理器，即 src-tauri 的 windows/lifecycle 里的
//   open_window / close_window 命令创建 / 关闭真实窗口，逻辑全在 Rust 侧；
//   命令失败时回退到官方 JS API new WebviewWindow()，再失败回退应用内切换。
// - 浏览器环境：用新标签页模拟独立窗口
// 单窗口模式：应用内页面切换（history 同步，可返回）

import { ref } from "vue";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { commands } from "../lib/bindings";
import type { ProjectItemDto } from "../lib/bindings";
import { saveGuiConfig } from "../lib/guiConfig";
import { t, tErr } from "../lib/i18n";
import { showToast } from "../lib/toast";
import { KEYS, readString, writeString } from "../lib/storage";
import { isTauri } from "../lib/util";
import { isWindowKind, type WindowKind, WINDOW_REGISTRY } from "./registry";

const WIN_PARAM = "window";
const UUID_PARAM = "uuid";

/**
 * 「一个实例一个窗口」的窗口族 → 标签前缀，完整形式 `<前缀><实例uuid>`
 *
 * 这类窗口的身份由后端按实例 uuid 派生，见 src-tauri 的 windows/registry/family.rs
 * 窗口族 `WINDOW_FAMILIES`。前端只按这些前缀认它们，不再另立一套命名。
 */
const FAMILY_LABEL_PREFIX: Partial<Record<WindowKind, string>> = {
  log: "mml-log-",
  resource: "mml-resource-",
  add_resource: "mml-add_resource-",
  config: "mml-config-",
};

/**
 * 从窗口标签认出它属于哪个窗口族
 *
 * - `label`: 窗口标签，形如 `mml-log-<实例uuid>`
 *
 * # 返回值
 *
 * 窗口族的 kind；不是窗口族的标签时为 null
 */
function familyKindOfLabel(label: string): WindowKind | null {
  for (const [kind, prefix] of Object.entries(FAMILY_LABEL_PREFIX)) {
    if (prefix && label.startsWith(prefix)) return kind as WindowKind;
  }
  return null;
}

/** 是否多窗口模式（默认多窗口） */
export const multiWindow = ref(readString(KEYS.windowMode) !== "Single");

/**
 * 单窗口模式的返回栈：记着"上一层是谁"，关窗时先回上一层
 *
 * 多窗口模式不需要它：每个窗口都是独立窗口，关掉自然露出下面那个。
 * 只记 kind、不持久化——重启后没有可返回的历史。
 */
const backStack = ref<WindowKind[]>([]);

/** 返回栈上限，防御性；正常导航不会超过几个 */
const BACK_STACK_MAX = 16;

/**
 * 打开下载窗口时要直接跳转到的项目，收藏窗口的「下载」用它
 *
 * 下载整合包 / 下载资源两个窗口都是"按实例取数"的：光有项目还拉不出版本列表，
 * 所以目标窗口拿到这份参数后，等实例就绪再调自己的 openDetail 打开详情页。
 */
export interface WindowProjectParam {
  /** 下载源，取 `ModPackType::to_string()`，即 curseforge / modrinth */
  source: string;
  /** 项目 ID */
  pid: string;
  /** 资源类型线串，取 `FileType::to_string()` */
  fileType: string;
  /** 项目名 */
  name: string;
  /** 图标地址；项目没有图标时为 null */
  icon: string | null;
  /** 项目页面地址 */
  url: string;
}

/**
 * 拖拽导入带进来的路径：主窗口拖入文件 / 文件夹 / 登录快捷方式，送到目标窗口
 *
 * `kind` 由后端 `window_probe_drop` 探测定：
 * - `folder` / `archive` → 「添加实例」窗口进对应模式并填好地址；
 * - `shortcut` → 「账户管理」窗口用 `url` 预填外置登录，那是快捷方式里解析出的认证服务器地址。
 */
export interface WindowDropParam {
  /** 拖入项的绝对路径 */
  path: string;
  /** folder / archive / shortcut */
  kind: "folder" | "archive" | "shortcut";
  /** 从快捷方式里解析出的认证服务器地址；仅 `kind` 为 shortcut 时有值 */
  url?: string;
}

/** openWindow 带的参数；各字段按目标窗口需要填，不填的字段全部按"没给"处理 */
export interface WindowOpenParams {
  /** 目标实例 uuid；窗口族靠它定位实例，导出窗口靠它切目标，其余窗口不传 */
  uuid?: string;
  /** 要直接打开的项目，下载整合包 / 下载资源窗口用 */
  project?: WindowProjectParam;
  /** 拖拽导入带进去的路径，主窗口拖入文件 / 文件夹时用 */
  drop?: WindowDropParam;
  /** 要聚焦的存档目录名，配置文件窗口"右键存档 → 修改配置文件"用 */
  saveDir?: string;
  /** 要直接打开的日志文件绝对路径；日志窗口用 */
  logFile?: string;
  /** 打开日志窗口后滚到底部；日志窗口用 */
  logBottom?: boolean;
  /** 要直接打开的类别 id，资源管理窗口用；"右键实例 → 查看模组"靠它落到模组页 */
  category?: string;
}

/**
 * 单窗口模式下"本次打开带入的参数"
 *
 * 单窗口既没有新窗口、也没有新 URL，Tauri 下连 pushState 都不做，壳层的 focus 事件更是
 * 无从触发，所以目标实例只能这样送达：openWindow 写进来，窗口组件 watch / onActivated 读走。
 */
export interface WindowParamsState {
  /** 目标实例 uuid */
  uuid: string | null;
  /** 要直接打开的项目 */
  project: WindowProjectParam | null;
  /** 拖入的路径参数 */
  drop: WindowDropParam | null;
  /** 要聚焦的存档目录名，配置文件窗口用 */
  saveDir: string | null;
  /** 要直接打开的日志文件绝对路径 */
  logFile: string | null;
  /** 打开日志窗口后滚到底部 */
  logBottom: boolean;
  /** 要直接打开的资源类别 id */
  category: string | null;
}

/** 一份空参数，字段与 {@link WindowOpenParams} 一一对应；重置点共用它，别各抄一份 */
function emptyWindowParams(): WindowParamsState {
  return {
    uuid: null,
    project: null,
    drop: null,
    saveDir: null,
    logFile: null,
    logBottom: false,
    category: null,
  };
}

/** 单窗口模式下本次打开带入的参数，见 {@link WindowParamsState} */
export const windowParams = ref<WindowParamsState>(emptyWindowParams());

/**
 * 单窗口模式：下载管理以"悬浮弹窗"出现，而不是把当前页面换掉
 *
 * 它是个工具型窗口——下载在后台跑，用户常常要一边看别的页面一边盯进度，
 * 所以浮在任意页面之上，见 [`openWindow`] 里的特例；有任务时主窗口顶栏另有入口，
 * 即 `components/DownloadTitleIndicator.vue`。
 */
export const downloadPopupOpen = ref(false);

/** 打开下载管理弹窗；只有单窗口模式走这里 */
export function openDownloadPopup() {
  downloadPopupOpen.value = true;
}

/** 关闭下载管理弹窗；任务不受影响，仍在后台跑 */
export function closeDownloadPopup() {
  downloadPopupOpen.value = false;
}

/**
 * 单窗口模式：整合包安装进度以"悬浮弹窗"出现，入口是标题栏上的 ModpackTitleIndicator
 *
 * 与下载管理同一个路子：安装跑在后台，用户可能已经离开"下载整合包"那一页，
 * 所以进度不能只挂在那个页面上。弹窗只影响"看不看得到进度"，关掉它不影响安装。
 */
export const modpackPopupOpen = ref(false);

/** 打开整合包安装进度弹窗 */
export function openModpackPopup() {
  modpackPopupOpen.value = true;
}

/** 关闭整合包安装进度弹窗；安装照常继续 */
export function closeModpackPopup() {
  modpackPopupOpen.value = false;
}

/**
 * 资源下载进度以"悬浮弹窗"出现，入口是标题栏上的 ResourceTitleIndicator
 *
 * 与下载管理 / 整合包安装同一个路子：下载跑在后台，用户可能已经离开了"添加资源"那一页，
 * 所以进度不能只挂在那个页面上。弹窗只影响"看不看得见进度"，关掉它不影响下载。
 */
export const resourcePopupOpen = ref(false);

/** 打开资源下载进度弹窗 */
export function openResourcePopup() {
  resourcePopupOpen.value = true;
}

/** 关闭资源下载进度弹窗；任务不受影响，仍在后台跑 */
export function closeResourcePopup() {
  resourcePopupOpen.value = false;
}

/**
 * 取本窗口（窗口族）锁定的实例 uuid
 *
 * 多窗口模式下窗口标签是唯一权威来源——后端就是按实例派生出这个标签的；
 * 浏览器预览与单窗口模式没有这个标签，由调用方另给参数，见 [`targetUuid`]。
 *
 * - `kind`: 窗口族的 kind，如 `log` / `resource`
 *
 * # 返回值
 *
 * 标签不属于该窗口族时为 null
 */
export function familyInstanceFromLabel(kind: WindowKind): string | null {
  if (!isTauri()) return null;
  const prefix = FAMILY_LABEL_PREFIX[kind];
  if (!prefix) return null;
  try {
    const label = getCurrentWindow().label;
    return label.startsWith(prefix) ? label.slice(prefix.length) : null;
  } catch (e) {
    console.error("[windowManager] 读取窗口标签失败", e);
    return null;
  }
}

/**
 * 取本次要定位的实例 uuid
 *
 * 单窗口取本次 openWindow 带入的参数，多窗口取本窗口 URL 上的参数
 *
 * # 返回值
 *
 * 没有目标实例时为 null
 */
export function targetUuid(): string | null {
  return multiWindow.value ? uuidFromUrl() : windowParams.value.uuid;
}

/**
 * 取本次要直接打开的项目
 *
 * 单窗口取 openWindow 带入的参数，多窗口取本窗口 URL 上的参数
 *
 * # 返回值
 *
 * 没有目标项目时为 null
 */
export function targetProject(): WindowProjectParam | null {
  return multiWindow.value ? projectFromUrl() : windowParams.value.project;
}

/**
 * 取本次带入的拖拽路径
 *
 * 单窗口取 openWindow 带入的参数，多窗口取本窗口 URL 上的参数。用完要清，
 * 见「添加实例」窗口的 consumeDrop：多窗口模式下 URL 上的参数会一直留着。
 *
 * # 返回值
 *
 * 没有拖入参数时为 null
 */
export function targetDrop(): WindowDropParam | null {
  return multiWindow.value ? dropFromUrl() : windowParams.value.drop;
}

/**
 * 取本次要聚焦的存档目录名，配置文件窗口用
 *
 * 单窗口取 openWindow 带入的参数，多窗口取本窗口 URL 上的参数。
 * 配置窗口把它套到来源下拉后，单窗口模式就地清掉；多窗口模式是本窗口
 * URL 身份的一部分，随窗口留着。
 *
 * # 返回值
 *
 * 没有目标存档时为 null
 */
export function targetSaveDir(): string | null {
  return multiWindow.value ? saveDirFromUrl() : windowParams.value.saveDir;
}

/**
 * 取本次要直接打开的资源类别 id
 *
 * 单窗口取 openWindow 带入的参数，多窗口取本窗口 URL 上的参数。
 * 资源管理窗口据此落到指定分类，见「右键实例 → 查看模组」。
 *
 * # 返回值
 *
 * 没有指定类别时为 null，调用方按上次打开的分类处理
 */
export function targetCategory(): string | null {
  return multiWindow.value
    ? new URLSearchParams(window.location.search).get(CATEGORY_PARAM)
    : windowParams.value.category;
}

/**
 * 用掉本次的资源类别参数
 *
 * 目标窗口在第一次打开时按它落类别，之后用户在窗口里怎么切都不该被参数拽回去，
 * 所以读完就清：单窗口清内存，多窗口把它从本窗口 URL 上摘掉。
 */
export function clearTargetCategory() {
  windowParams.value = { ...windowParams.value, category: null };
  if (!multiWindow.value) return;
  const url = new URL(window.location.href);
  url.searchParams.delete(CATEGORY_PARAM);
  window.history.replaceState({}, "", url);
}

/**
 * 取本次要直接打开的日志文件
 *
 * 单窗口取 openWindow 带入的参数，多窗口取本窗口 URL 上的参数。
 *
 * # 返回值
 *
 * 没有指定文件时为 null，调用方按运行日志处理
 */
export function targetLogFile(): string | null {
  if (!multiWindow.value) return windowParams.value.logFile;
  return new URLSearchParams(window.location.search).get(LOG_PARAMS.file);
}

/**
 * 本次打开是否要求日志滚到底
 *
 * 单窗口取 openWindow 带入的参数，多窗口取本窗口 URL 上的参数。
 *
 * # 返回值
 *
 * 要求滚到底时为 true
 */
export function targetLogBottom(): boolean {
  if (!multiWindow.value) return windowParams.value.logBottom;
  return new URLSearchParams(window.location.search).get(LOG_PARAMS.bottom) === "1";
}

/**
 * 切换单窗口 / 多窗口模式
 *
 * - `v`: true 为多窗口，false 为单窗口
 *
 * # 返回值
 *
 * 配置落盘的 Promise；紧接着要重启进程的调用方必须等它，否则写请求会随重启丢掉
 */
export function setMultiWindow(v: boolean): Promise<void> {
  multiWindow.value = v;
  // 换了模式，之前的返回栈与带入参数都不再成立
  backStack.value = [];
  windowParams.value = emptyWindowParams();
  writeString(KEYS.windowMode, v ? "Multi" : "Single");
  // 返回保存的 Promise：要紧接着重启进程的调用方必须等这次写入真的落盘。
  // 重启会立刻刷盘并退出，写请求还在路上就丢了，见 useSettingsUi 的窗口模式切换
  return saveGuiConfig({ windowMode: v ? "Multi" : "Single" });
}

/** 是否运行在 Tauri 环境；实现见 lib/util，放那儿是为了不与 lib/storage 形成循环引用 */
export { isTauri };

/** 当前窗口标识 */
export const currentKind = ref<WindowKind>(resolveKind());

/** 认出当前窗口 kind：Tauri 看窗口标签，浏览器看 URL 查询参数 */
function resolveKind(): WindowKind {
  if (isTauri()) {
    try {
      // Tauri：用窗口标签识别，如 mml-settings → settings，main → main
      const label = getCurrentWindow().label;
      // 窗口族先判：标签是 `mml-<kind>-<实例uuid>`，去掉 `mml-` 也不是任何一个 kind
      const family = familyKindOfLabel(label);
      if (family) return family;
      const kind = label.replace(/^mml-/, "");
      if (isWindowKind(kind)) return kind;
    } catch (e) {
      console.error("[windowManager] 读取窗口标签失败", e);
    }
  }
  // 浏览器：从 URL 查询参数识别
  return kindFromUrl();
}

/** 项目参数在 URL 上的键；多窗口模式下新窗口只能从 URL 拿到"要打开哪个项目" */
const PROJ_PARAMS = {
  source: "psource",
  pid: "ppid",
  fileType: "ptype",
  name: "pname",
  icon: "picon",
  url: "purl",
} as const;

/** 拖拽导入参数在 URL 上的键；同样只有 URL 一条路能送进新窗口 */
const DROP_PARAMS = {
  path: "dpath",
  kind: "dkind",
  url: "durl",
} as const;

/** 存档聚焦参数在 URL 上的键；配置文件窗口"右键存档 → 修改配置文件"用 */
const SAVE_PARAM = "save";

/** 日志窗口参数在 URL 上的键：要打开哪个文件、是否滚到底 */
const LOG_PARAMS = {
  file: "lfile",
  bottom: "lbottom",
} as const;

/** 资源类别参数在 URL 上的键；资源管理窗口用 */
const CATEGORY_PARAM = "rcat";

/** 按 kind 与带入参数拼出目标窗口的 URL；多窗口模式下新窗口靠它取参数 */
function urlFor(
  kind: WindowKind,
  params?: WindowOpenParams,
): string {
  const url = new URL(window.location.href);
  if (kind === "main") {
    url.searchParams.delete(WIN_PARAM);
  } else {
    url.searchParams.set(WIN_PARAM, kind);
  }
  if (params?.uuid) {
    url.searchParams.set(UUID_PARAM, params.uuid);
  } else {
    url.searchParams.delete(UUID_PARAM);
  }
  if (params?.saveDir) {
    url.searchParams.set(SAVE_PARAM, params.saveDir);
  } else {
    url.searchParams.delete(SAVE_PARAM);
  }
  if (params?.logFile) {
    url.searchParams.set(LOG_PARAMS.file, params.logFile);
  } else {
    url.searchParams.delete(LOG_PARAMS.file);
  }
  if (params?.logBottom) {
    url.searchParams.set(LOG_PARAMS.bottom, "1");
  } else {
    url.searchParams.delete(LOG_PARAMS.bottom);
  }
  if (params?.category) {
    url.searchParams.set(CATEGORY_PARAM, params.category);
  } else {
    url.searchParams.delete(CATEGORY_PARAM);
  }
  if (params?.drop) {
    url.searchParams.set(DROP_PARAMS.path, params.drop.path);
    url.searchParams.set(DROP_PARAMS.kind, params.drop.kind);
    if (params.drop.url) {
      url.searchParams.set(DROP_PARAMS.url, params.drop.url);
    } else {
      url.searchParams.delete(DROP_PARAMS.url);
    }
  } else {
    for (const key of Object.values(DROP_PARAMS)) {
      url.searchParams.delete(key);
    }
  }
  if (params?.project) {
    const p = params.project;
    url.searchParams.set(PROJ_PARAMS.source, p.source);
    url.searchParams.set(PROJ_PARAMS.pid, p.pid);
    url.searchParams.set(PROJ_PARAMS.fileType, p.fileType);
    url.searchParams.set(PROJ_PARAMS.name, p.name);
    url.searchParams.set(PROJ_PARAMS.url, p.url);
    if (p.icon) {
      url.searchParams.set(PROJ_PARAMS.icon, p.icon);
    } else {
      url.searchParams.delete(PROJ_PARAMS.icon);
    }
  } else {
    for (const key of Object.values(PROJ_PARAMS)) {
      url.searchParams.delete(key);
    }
  }
  return url.toString();
}

/**
 * 从 URL 读取窗口 kind
 *
 * # 返回值
 *
 * 没有该参数或认不出时按 main 处理
 */
export function kindFromUrl(): WindowKind {
  const k = new URLSearchParams(window.location.search).get(WIN_PARAM);
  return isWindowKind(k) ? k : "main";
}

/**
 * 从 URL 读取目标实例 uuid
 *
 * # 返回值
 *
 * 没有该参数时为 null
 */
export function uuidFromUrl(): string | null {
  return new URLSearchParams(window.location.search).get(UUID_PARAM);
}

/**
 * 从 URL 读取"要直接打开的项目"，供下载整合包 / 下载资源窗口用
 *
 * # 返回值
 *
 * 源 / 项目 ID / 资源类型三项缺一即为 null
 */
export function projectFromUrl(): WindowProjectParam | null {
  const q = new URLSearchParams(window.location.search);
  const source = q.get(PROJ_PARAMS.source);
  const pid = q.get(PROJ_PARAMS.pid);
  const fileType = q.get(PROJ_PARAMS.fileType);
  if (!source || !pid || !fileType) {
    return null;
  }
  return {
    source,
    pid,
    fileType,
    name: q.get(PROJ_PARAMS.name) ?? "",
    icon: q.get(PROJ_PARAMS.icon),
    url: q.get(PROJ_PARAMS.url) ?? "",
  };
}

/**
 * 从 URL 读取拖拽导入的路径，供主窗口拖拽时开的目标窗口用
 *
 * # 返回值
 *
 * 没有路径参数或 `kind` 不合法时为 null
 */
export function dropFromUrl(): WindowDropParam | null {
  const q = new URLSearchParams(window.location.search);
  const path = q.get(DROP_PARAMS.path);
  const kind = q.get(DROP_PARAMS.kind);
  if (!path || (kind !== "folder" && kind !== "archive" && kind !== "shortcut")) {
    return null;
  }
  return { path, kind, url: q.get(DROP_PARAMS.url) ?? "" };
}

/**
 * 从 URL 读取要聚焦的存档目录名，配置文件窗口用
 *
 * # 返回值
 *
 * 没有该参数时为 null
 */
function saveDirFromUrl(): string | null {
  return new URLSearchParams(window.location.search).get(SAVE_PARAM);
}

/**
 * 把窗口参数里的项目还原成窗口能直接用的项目条目
 *
 * - `p`: 窗口参数里的项目，字段来自收藏夹存下来的条目
 *
 * # 返回值
 *
 * 只有名字 / 图标 / 网址 / 源 / 项目 ID 有值；作者、标签、截图、下载次数这些
 * 一律留空 —— 两个窗口的"打开项目"只用到 name / image / url / source.pid。
 */
export function projectParamToItem(p: WindowProjectParam): ProjectItemDto {
  return {
    name: p.name,
    summary: "",
    image: p.icon,
    authors: [],
    tag: [],
    screenshots: [],
    downloadCount: 0,
    date: "",
    download: false,
    canStar: false,
    isStar: false,
    downloadNow: false,
    url: p.url,
    mcmod: null,
    source: { fileType: p.fileType, source: p.source, pid: p.pid, fid: "" },
  };
}

/**
 * 报给后端的"是哪个实例"参数
 *
 * 只有窗口族、导出窗口与服务器包窗口认它：窗口族靠它定位到族里那一个窗口，
 * 导出 / 服务器包窗口靠它在已开窗口上切目标实例。其余窗口传了会被后端
 * `kind_parse_check` 拒掉，所以这里一律给 null。
 *
 * - `kind`: 窗口类型
 * - `params`: 本次打开带入的参数
 *
 * # 返回值
 *
 * 实例 uuid；其余窗口为 null
 */
function instanceArgOf(kind: WindowKind, params?: WindowOpenParams): string | null {
  return FAMILY_LABEL_PREFIX[kind] ||
    kind === "export" ||
    kind === "server_pack" ||
    kind === "server_instance"
    ? (params?.uuid ?? null)
    : null;
}

/** 大厅验证期间防止重复打开 */
export const lobbyOpening = ref(false);
/** 上一次大厅会话释放的 Promise，下一次打开前先等它收尾 */
let lobbyClosing: Promise<void> = Promise.resolve();

/**
 * 等上一页释放账户后，再验证本次大厅会话
 *
 * # 返回值
 *
 * 大厅会话就绪的 Promise；验证失败时 reject，由调用方弹提示
 */
export async function prepareLobby() {
  await lobbyClosing;
  return commands.lobby.prepare();
}

/** 单窗口模式先释放旧会话，再允许下一次打开 */
export function releaseLobby() {
  lobbyClosing = commands.lobby.close().catch(error => console.error("[lobby] 释放会话失败", error));
}

/**
 * 打开一个窗口，功能入口与各处的跳转都走它
 *
 * - `kind`: 目标窗口
 * - `params`: 见 [`WindowOpenParams`]，各字段按目标窗口需要填
 */
export async function openWindow(kind: WindowKind, params?: WindowOpenParams) {
  if (kind === "lobby" && isTauri()) {
    if (lobbyOpening.value) return;
    lobbyOpening.value = true;
    try { await prepareLobby(); }
    catch (error) { showToast(tErr(error)); return; }
    finally { lobbyOpening.value = false; }
  }
  // 单窗口模式（浏览器 / Tauri 一致）：应用内页面切换。
  // 窗口族是"一个实例一个"：实例是它的窗口身份（后端按它派生 uuid 与标签），
  // 没有实例就开不出正确的窗口，两种模式都直接作罢
  const isFamily = !!FAMILY_LABEL_PREFIX[kind];
  // 「下载资源」例外：没有实例时开那个静态窗口（标签 `mml-add_resource`），
  // 实例在窗口标题栏里选，见 AddResourceWindow 的 refreshInstance
  if (isFamily && !params?.uuid && kind !== "add_resource") {
    console.warn(`[windowManager] 打开 ${kind} 窗口缺少实例 uuid，已忽略`);
    return;
  }

  if (!multiWindow.value) {
    // 下载管理是工具型窗口：浮在当前页面之上，不把页面换掉（关掉它也不会"返回"到哪去）
    if (kind === "download") {
      openDownloadPopup();
      return;
    }
    // 已经停在这个窗口，例如就在日志页又右键了另一个实例：不压栈，只把新参数送过去，
    // 目标窗口自己按参数变化换锁定对象
    if (currentKind.value !== kind) {
      backStack.value.push(currentKind.value);
      if (backStack.value.length > BACK_STACK_MAX) backStack.value.shift();
      currentKind.value = kind;
    }
    // 目标实例 / 目标项目 / 目标存档只能靠这里送达：单窗口没有新窗口 / 新 URL，
    // 也没有壳层的 focus 事件
    windowParams.value = {
      uuid: params?.uuid ?? null,
      project: params?.project ?? null,
      drop: params?.drop ?? null,
      saveDir: params?.saveDir ?? null,
      logFile: params?.logFile ?? null,
      logBottom: params?.logBottom ?? false,
      category: params?.category ?? null,
    };
    if (!isTauri()) window.history.pushState({}, "", urlFor(kind, params));
    return;
  }

  if (isTauri()) {
    // 新窗口的目标参数只能靠 URL 送达：实例 uuid + "要打开哪个项目" + 拖入的路径 +
    // 要聚焦的存档。而 Rust 那条命令只认窗口族与 export 的 uuid。所以这里把组装好的
    // query 一起交过去，由它拼进 WebviewUrl —— 建窗仍然走 Rust，
    // 几何恢复 / 句柄表 / 窗口模型都在那边。
    const query = new URL(urlFor(kind, params)).search.replace(/^\?/, "");
    // 带拖入路径 / 目标存档 / 要打开的项目：窗口**已经开着**时必须重新加载才拿得到新参数
    // （只聚焦会把参数丢掉），所以走 reopen，Rust 侧已存在则 navigate；
    // 其余窗口维持"只聚焦"
    //
    // project 也在这里面：从收藏窗口点一个已收藏的项目，目标窗口若正开着，
    // 只聚焦的话它还停在原列表上，看不到该项目详情 —— 这正是"点了没反应"的由来
    if (
      params?.drop ||
      params?.saveDir ||
      params?.logFile ||
      params?.logBottom ||
      params?.category ||
      params?.project ||
      (kind === "server_pack" && !!params?.uuid)
    ) {
      commands.windows
        .reopenWindow(kind, instanceArgOf(kind, params), query)
        .catch((e) => {
          if (kind === "lobby") { releaseLobby(); showToast(tErr(e)); return; }
          console.error("[windowManager] Rust 重开窗口失败，回退 JS API", kind, e);
          createViaJs(kind, params);
        });
      return;
    }
    // 统一走 Rust 窗口管理器：创建 / 聚焦在 src-tauri 的 windows/lifecycle 处理。
    // open_window 是 async 命令，不在 Windows 主线程创建窗口，避免冻结。
    // 命令失败（例如窗口创建被拒）时回退到官方 JS API。
    commands.windows
      .openWindow(kind, instanceArgOf(kind, params), query)
      .catch((e) => {
        if (kind === "lobby") { releaseLobby(); showToast(tErr(e)); return; }
        console.error("[windowManager] Rust 打开窗口失败，回退 JS API", kind, e);
        createViaJs(kind, params);
      });
    return;
  }

  // 浏览器多窗口：新标签页模拟
  window.open(urlFor(kind, params), kind, "noopener");
}

/** 回退路径：用官方 JS API 创建 / 聚焦真实 WebviewWindow */
function createViaJs(kind: WindowKind, params?: WindowOpenParams) {
  // 窗口族的标签带实例 uuid，与后端 family_window_id 同一口径：
  // 少了这一段就会开出一个身份不对的窗口，跟 Rust 那条路开的也不是同一个
  const prefix = FAMILY_LABEL_PREFIX[kind];
  const label = prefix && params?.uuid ? `${prefix}${params.uuid}` : `mml-${kind}`;
  WebviewWindow.getByLabel(label).then(async (existing) => {
    if (existing) {
      // 窗口已存在则聚焦；导出窗口的目标实例靠 export-focus 事件链路自行同步
      existing.setFocus();
      return;
    }
    // 尺寸从后端注册表取（与 Rust 建窗同源）；后端不可用时给个兜底值
    const sizes = await commands.windows.getWindowSizes().catch(() => []);
    const size = sizes.find((s) => s.kind === kind);
    const info = WINDOW_REGISTRY.find((w) => w.kind === kind);
    const win = new WebviewWindow(label, {
      url: urlFor(kind, params).replace(window.location.origin, ""),
      title: info ? t(info.titleKey) : kind,
      width: size?.width ?? 900,
      height: size?.height ?? 620,
      resizable: true,
    });
    win.once("tauri://error", (e) => {
      console.error("[windowManager] 创建窗口失败，回退到应用内切换", label, e);
      // 失败时回退：应用内切换，保证功能可用
      currentKind.value = kind;
      window.history.pushState({}, "", urlFor(kind, params));
    });
  });
}

/**
 * 关闭窗口时随 kind 一起报给后端的实例参数
 *
 * 只有窗口族需要：它的窗口身份由实例派生，光一个 kind 定不下是哪一个窗口，
 * 见后端 `window_close_window`。
 *
 * - `kind`: 当前窗口类型
 *
 * # 返回值
 *
 * 实例 uuid；其余窗口为 null
 */
function closeInstanceArg(kind: WindowKind): string | null {
  return FAMILY_LABEL_PREFIX[kind] ? familyInstanceFromLabel(kind) : null;
}

/**
 * 关掉当前窗口：标题栏 ✕ 的动作，真的关窗
 *
 * - 多窗口模式：关掉当前这个功能窗口
 * - 单窗口模式：整个应用只有主窗口一个真实窗口，等于退出启动器
 *
 * "回上一层"是另一回事，走 closeWindow —— 标题栏的 ‹ 箭头与各窗口的"取消"都走它。
 */
export function quitWindow() {
  if (!isTauri()) {
    // 浏览器预览：非脚本打开的标签页关不掉，失败就作罢
    window.close();
    return;
  }
  // 单窗口模式只有主窗口是真的：那里 kind 是"当前页"而不是窗口标签，拿它去找窗口会 not found
  const target = multiWindow.value ? currentKind.value : "main";
  const instance = multiWindow.value ? closeInstanceArg(target) : null;
  commands.windows.closeWindow(target, instance).catch((e) => {
    console.error("[windowManager] 关闭窗口失败，回退 JS API", target, e);
    getCurrentWindow().close();
  });
}

/**
 * 同一次事件派发内的去重标志，见 [`closeWindow`]
 *
 * 这是踩过的坑：窗口组件若没声明 `defineEmits`，Vue 会把父级的 `@close` 当 attrs
 * 透传到根组件上，与模板里的 `@close="$emit('close')"` 合并成**两个**处理器 ——
 * 一次"返回"会同步调两遍 closeWindow()：第一遍切回主页，currentKind 变成 main，
 * 第二遍就撞上"在主页 = 关窗口 = 退出应用"，于是点返回直接把程序关了。
 *
 * 用微任务窗口去重：同一次事件派发里的重复调用被丢掉；用户连点两次返回是两个独立任务，
 * 仍然照常逐层后退。
 */
let closing = false;

/**
 * 收起当前页面 / 窗口，由各窗口的「取消」按钮、标题栏的 ‹ 箭头触发
 *
 * - 多窗口模式：每个功能就是一个独立窗口，收起 = 关掉它
 * - 单窗口模式：应用内页面切换，弹返回栈回上一层（没有上一层就回主页面）
 *
 * ✕ 不走这里，走 quitWindow：单窗口模式下 ✕ 是真的退出应用，不是退一层
 */
export function closeWindow() {
  // 同一次事件里的重复调用直接丢掉，原因见上面 closing 的说明
  if (closing) return;
  closing = true;
  queueMicrotask(() => {
    closing = false;
  });

  const kind = currentKind.value;

  // 单窗口模式：先回上一层（谁打开的），没有上一层再回主页面
  if (!multiWindow.value) {
    const prev = backStack.value.pop();
    if (prev && prev !== kind) {
      goTo(prev);
      return;
    }
    if (kind !== "main") {
      goTo("main");
      return;
    }
    // 已经在主页面：主页面就是应用本身，关它 = 退出应用。
    // 不能走上面"切回主页面"的分支——那样标题栏的 ✕ 会变成空操作（启动画面为此单独绕过过）
    if (isTauri()) {
      commands.windows.closeWindow("main", null).catch((e) => {
        console.error("[windowManager] 关闭主窗口失败，回退 JS API", e);
        getCurrentWindow().close();
      });
      return;
    }
    // 浏览器预览：非脚本打开的标签页关不掉，失败就作罢
    window.close();
    return;
  }

  if (isTauri()) {
    // 多窗口：走 Rust 窗口管理器关闭当前真实窗口，kind → 标签的映射在 windows/registry。
    // 日志窗口族的标签带实例 uuid，所以要把实例一起报上去。
    // 命令失败时回退到官方 JS API 关闭当前窗口。
    commands.windows.closeWindow(kind, closeInstanceArg(kind)).catch((e) => {
      console.error("[windowManager] Rust 关闭窗口失败，回退 JS API", kind, e);
      getCurrentWindow().close();
    });
  } else {
    // 浏览器多窗口：由脚本打开的标签页允许 window.close()
    window.close();
  }
}

/** 单窗口模式的应用内跳转；只管显示状态与 URL，不动返回栈 */
function goTo(kind: WindowKind) {
  currentKind.value = kind;
  // 返回后不再带着上一次的目标实例 / 目标项目 / 拖入路径 / 存档参数，
  // 避免窗口"记得"不该记的东西
  windowParams.value = emptyWindowParams();
  if (!isTauri()) window.history.pushState({}, "", urlFor(kind));
}

// 浏览器前进 / 后退同步（单窗口模式）
window.addEventListener("popstate", () => {
  // 浏览器历史是另一套栈，和应用内返回栈没法对齐：一后退就清掉，免得两边打架
  backStack.value = [];
  currentKind.value = kindFromUrl();
});
