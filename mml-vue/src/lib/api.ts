// M²L 前端 API（真实 IPC 实现）
//
// 数据全部从 Rust 侧获取（命令见 mml-gui/src-tauri/src/windows/），
// 按钮执行的操作也通过 IPC 调用。仅在 Tauri 环境可用（纯浏览器会报错）。
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { commands } from "./bindings";
import type {
  BlockItemDto,
  BlockStatusDto,
  CollectDataDto,
  ColorMcInfoDto,
  ColorMcProgressDto,
  ColorMcReportDto,
  DataPackItemDto,
  DetectedPackDto,
  LogLine,
  DownloadItemEvent,
  DownloadStatusDto,
  DownloadTaskEvent,
  ErrorEvent,
  ExitEvent,
  FileListDto,
  FolderInstanceDto,
  ClientConfigDto,
  ExportConfigDto,
  ExportInfoDto,
  ExportProgressDto,
  GroupDto,
  InstanceArgsDto,
  InstanceInfoDto,
  InstanceLangDto,
  JavaInfoDto,
  LoadState,
  LogEvent,
  LogFocusDto,
  ModGroupDto,
  ResourceViewDto,
  ModItemDto,
  ModRenameDto,
  ModScanProgressDto,
  MotdDto,
  ModPackStatusDto,
  NewsItem,
  PackItemDto,
  PackProgressDto,
  ProjectDetailDto,
  ProjectDto,
  ResourceSaveDto,
  ResourceStatusDto,
  SaveBackupDto,
  SaveItemDto,
  ScreenshotItemDto,
  ServerItemDto,
  ShaderItemDto,
  SchematicItemDto,
  StatsDataDto,
  StateEvent,
  SystemMemoryDto,
  VersionInfoDto,
} from "./bindings";
import { AddLoaderProgress, AddModpackStatus, AddNameConflict, AddPackProgress, AddResourceStatus, BlockRender, ClientConfigChange, CloseBlocked, CollectChange, ColormcProgress, CustomHomeChange, DownloadItem, DownloadTask, ExportFocus, ExportProgress, GameExit, GameLog, InstanceChange, JavaChange, LaunchError, LaunchState, LogFocus, ResourceListModsProgress } from "./listens";

export interface CreateInstanceOpts {
  loader?: string;
  loaderVersion?: string | null;
  /** 目标分组 uuid（null / 缺省 = 默认分组） */
  group?: string | null;
  modpackType?: string;
  source?: string;
}

export const api = {
  /**
   * 获取实例列表
   *
   * # 返回值
   *
   * 全部实例，含各自的运行状态
   */
  async getInstances(): Promise<InstanceInfoDto[]> {
    return commands.main.getInstances();
  },

  /**
   * 获取统计快照
   *
   * # 返回值
   *
   * 全局启动计数，以及每实例的启动次数与累计时长
   */
  async getStatsData(): Promise<StatsDataDto> {
    return commands.stats.data();
  },

  /**
   * 查询服务器 MOTD
   *
   * - `address`: 服务器地址，可写 host 或 host:port，端口缺省 25565
   *
   * # 返回值
   *
   * MOTD 查询结果；连不上或无数据时 `state` 为对应失败态
   */
  async getMotd(address: string): Promise<MotdDto> {
    return getMotd(address);
  },

  /** 获取分组列表（含空分组；uuid 为身份、name 只是显示名，默认分组排在最前且名字为空白） */
  async getGroups(): Promise<GroupDto[]> {
    return commands.main.getGroups();
  },

  /**
   * 把"用户输入的分组名"解析成分组 uuid
   *
   * 添加实例 / 安装整合包两处都是**手输或选一个组名**（还能顺手建新组），
   * 而 IPC 一律按 uuid 传，所以在这个边界上转一次：
   * 已有同名分组就用它，没有就现建一个。空名 = 默认分组（null）。
   */
  async resolveGroupId(name: string): Promise<string | null> {
    const trimmed = name.trim();
    if (!trimmed) return null;
    const found = (await commands.main.getGroups()).find((g) => g.name === trimmed);
    if (found) return found.uuid;
    return await commands.main.addGroup(trimmed);
  },

  /**
   * 获取实例的游戏内语言列表（从资源索引查 minecraft/lang/*.json，资源未下载时为空）
   *
   * 每项带显示名 `name`（取自语言文件里的 `language.name`，如 zh_cn → 简体中文），
   * 读不到时它就是 `code`，所以直接显示 `name` 即可。
   */
  async getInstanceLangs(uuid: string): Promise<InstanceLangDto[]> {
    return commands.main.getInstanceLangs(uuid);
  },

  /** 获取 Java 列表（配置加载 / 扫描异步进行，未完成时为空） */
  async getJavaList(): Promise<JavaInfoDto[]> {
    return commands.main.getJavaList();
  },

  /**
   * 获取本机内存（MiB），用于内存设置显示的参考值
   *
   * 查询失败时后端返回 0，调用方按"不可用"处理（不显示）
   */
  async getSystemMemory(): Promise<SystemMemoryDto> {
    return commands.main.getSystemMemory();
  },

  /** 获取游戏版本列表（后端进程级缓存） */
  async getVersions(): Promise<VersionInfoDto[]> {
    return commands.main.getVersions();
  },

  /** 强制刷新版本列表（清空后端缓存重新拉取） */
  async refreshVersions(): Promise<VersionInfoDto[]> {
    return commands.main.refreshVersions();
  },

  /** 获取 Minecraft 官方新闻（第 page 页，默认 1） */
  async getNews(page = 1): Promise<NewsItem[]> {
    return commands.main.getNews(page);
  },

  /** 用系统浏览器打开网址 */
  async openUrl(url: string): Promise<void> {
    return commands.main.openUrl(url);
  },

  /** 从头新建实例，版本 + 加载器；返回新实例 uuid。group 是分组 uuid，null 表示默认分组 */
  async addCreateNew(
    name: string,
    version: string,
    loader: string,
    loaderVersion: string | null,
    group: string | null,
  ): Promise<string> {
    return commands.add.createNew(name, version, loader, loaderVersion, group);
  },

  /** 导入文件夹为实例（group 是分组 uuid，null = 默认分组） */
  async addImportFolder(path: string, name: string, group: string | null): Promise<string> {
    return commands.add.importFolder(path, name, group);
  },

  /** 导入整合包压缩包（packType：CurseForge / Modrinth / McMod / 本地；group 是分组 uuid） */
  async addImportArchive(
    path: string,
    packType: string,
    name: string,
    group: string | null,
    unselect: string[] | null,
  ): Promise<string> {
    return commands.add.importArchive(path, packType, name, group, unselect);
  },

  /** 从网址安装实例（group 是分组 uuid，null = 默认分组） */
  async addImportUrl(url: string, name: string, group: string | null): Promise<string> {
    return commands.add.importUrl(url, name, group);
  },

  /** 检测压缩包的整合包类型与推荐实例名（识别失败抛错） */
  async addDetectArchive(path: string): Promise<DetectedPackDto> {
    return commands.add.detectArchive(path);
  },

  // ---------------- 在线整合包 ----------------

  /** 获取整合包下载源 ID 列表（curseforge / modrinth） */
  async getModpackSources(): Promise<string[]> {
    return commands.addResource.sourceType();
  },

  /** 获取某下载源的排序方式列表（取值即后端枚举线串，可原样回传） */
  async getModpackSorts(source: string): Promise<string[]> {
    return commands.addResource.sortType(source);
  },

  /** 获取某下载源整合包的分类（键 = 传给后端的分类值，值 = 显示名） */
  async getModpackCategories(source: string): Promise<Record<string, string>> {
    return commands.addResource.categories(source, "modpack");
  },

  /** 获取某下载源支持的游戏版本列表 */
  async getModpackVersions(source: string): Promise<string[]> {
    return commands.addResource.gameVersions(source);
  },

  /** 搜索在线整合包（page 从 0 开始；category / filter / version 传 null 表示不过滤） */
  async searchModpacks(
    source: string,
    page: number,
    sort: string,
    category: string | null,
    filter: string | null,
    version: string | null,
  ): Promise<ProjectDto> {
    return commands.addModpack.list(source, page, sort, category, filter, version);
  },

  /** 获取某整合包的可安装版本列表（version 传 null 取全部） */
  async getModpackFiles(
    source: string,
    projectId: string,
    page: number,
    version: string | null,
  ): Promise<FileListDto> {
    return commands.addModpack.file(source, projectId, page, version);
  },

  /** 获取某整合包的项目详情（Modrinth 含 markdown 正文；CurseForge 只有简介） */
  async getModpackDetail(source: string, projectId: string): Promise<ProjectDetailDto> {
    return commands.addModpack.detail(source, projectId);
  },

  /** 安装在线整合包（多任务：命令立即返回，任务后台安装，进度走 add-modpack-status 事件；group 是分组 uuid） */
  async installModpack(
    source: string,
    projectId: string,
    fileId: string,
    group: string | null,
  ): Promise<void> {
    return commands.addModpack.install(source, projectId, fileId, group);
  },

  /** 查询整合包安装任务总览（挂载时同步一次，之后靠事件） */
  async getModpackStatus(): Promise<ModPackStatusDto> {
    return commands.addModpack.status();
  },

  // ---------------- 在线资源（实例设置 → 添加资源） ----------------

  /** 获取某下载源某资源类型的分类（键 = 传给后端的分类值，值 = 显示名） */
  async getResourceCategories(source: string, fileType: string): Promise<Record<string, string>> {
    return commands.addResource.categories(source, fileType);
  },

  /** 获取某下载源的排序方式列表（取值即后端枚举线串，可原样回传） */
  async getResourceSorts(source: string): Promise<string[]> {
    return commands.addResource.sortType(source);
  },

  /** 获取某下载源支持的游戏版本列表 */
  async getResourceVersions(source: string): Promise<string[]> {
    return commands.addResource.gameVersions(source);
  },

  /** 搜索在线资源（page 从 0 开始；category / filter / version / loader 传 null 表示不过滤） */
  async searchResources(
    game: string,
    source: string,
    fileType: string,
    page: number,
    sort: string,
    category: string | null,
    filter: string | null,
    version: string | null,
    loader: string | null,
  ): Promise<ProjectDto> {
    return commands.addResource.list(
      game,
      source,
      fileType,
      page,
      sort,
      category,
      filter,
      version,
      loader,
    );
  },

  /** 获取某资源的可下载版本列表（page 从 0 开始；version / loader 传 null 表示不过滤） */
  async getResourceFiles(
    game: string,
    source: string,
    projectId: string,
    fileType: string,
    page: number,
    version: string | null,
    loader: string | null,
  ): Promise<FileListDto> {
    return commands.addResource.file(game, source, projectId, fileType, page, version, loader);
  },

  /** 获取实例的存档列表（下载数据包时选择目标存档） */
  async getResourceSaves(game: string): Promise<ResourceSaveDto[]> {
    return commands.addResource.saves(game);
  },

  /** 下载资源到实例（fileType = mod / resourcepack / shaderpack / save / dataPacks；
   *  数据包必须传 world = 存档文件夹名；命令立即返回，下载走全局下载器） */
  async downloadResource(
    game: string,
    source: string,
    projectId: string,
    fileId: string,
    fileType: string,
    world: string | null,
  ): Promise<void> {
    return commands.addResource.download(game, source, projectId, fileId, fileType, world);
  },

  /** 查询资源下载任务总览（挂载时同步一次，之后靠事件） */
  async getResourceStatus(): Promise<ResourceStatusDto> {
    return commands.addResource.status();
  },

  /** 取消一个进行中的整合包安装任务（pid + fid 定位） */
  async cancelModpackInstall(pid: string, fid: string): Promise<void> {
    return commands.addModpack.cancel(pid, fid);
  },

  /** 清除已结束（完成 / 失败 / 取消）的整合包安装任务 */
  async clearModpackDone(): Promise<void> {
    return commands.addModpack.clearDone();
  },

  /** 取消进行中的安装任务 */
  async addCancel(): Promise<boolean> {
    return commands.add.cancel();
  },

  /** 获取加载器的可用版本列表（loader：加载器 ID，mc：游戏版本号） */
  async addLoaderVersions(loader: string, mc: string): Promise<string[]> {
    return commands.add.getLoaderVersions(loader, mc);
  },

  /** 获取加载器 ID 列表 */
  async addGetLoaders(): Promise<string[]> {
    return commands.add.getLoaders();
  },

  /** 查询指定游戏版本支持的加载器 ID 列表 */
  async addGetSupportLoaders(mc: string): Promise<string[]> {
    return commands.add.getSupportLoaders(mc);
  },

  /** 设置添加实例窗口关闭保护（enabled = true 期间拒绝关闭请求） */
  async setCloseGuard(enabled: boolean): Promise<void> {
    await commands.add.setCloseGuard(enabled);
  },

  /**
   * 确保某个窗口 kind 的模型存在（单窗口模式下页面挂载时调用）
   *
   * 多窗口模式下模型跟着真实窗口走，这里重复调用无副作用。
   */
  async ensureWindowModel(kind: string): Promise<void> {
    await commands.windows.ensureModel(kind);
  },

  /** 释放某个窗口 kind 的模型（单窗口模式下页面切走 / 关闭时调用，别让模型常驻） */
  async dropWindowModel(kind: string): Promise<void> {
    await commands.windows.dropModel(kind);
  },

  /**
   * 扫描文件夹里可导入的实例（官方启动器 `versions/*` 与 MMC `instances/*`）
   *
   * 选到 `.minecraft` 这类装着若干实例的目录时用：返回的每一项都是一个可导入的实例目录，
   * 由前端列出来让用户勾选（而不是把整个 `.minecraft` 当成一个实例）
   */
  async addScanFolder(path: string): Promise<FolderInstanceDto[]> {
    return commands.add.scanFolder(path);
  },

  /** 获取压缩包类型 ID 列表 */
  async addGetPackTypes(): Promise<string[]> {
    return commands.add.getPackTypes();
  },

  /** 获取游戏版本类型 ID 列表（release / snapshot / old_beta / old_alpha） */
  async addGetVersionTypes(): Promise<string[]> {
    return commands.add.getVersionTypes();
  },

  /** 添加空分组（重名返回 false） */
  /** 新建分组，返回新分组的 uuid（名字为空 / 重名返回 null） */
  async addGroup(name: string): Promise<string | null> {
    return commands.main.addGroup(name);
  },

  /** 删除分组（组内实例移入默认分组；分组 uuid 为空 / 非法即默认分组，不可删） */
  async removeGroup(uuid: string): Promise<boolean> {
    return commands.main.removeGroup(uuid);
  },

  /** 调整分组显示顺序（uuid 是分组 uuid，index 为目标位置） */
  async moveGroup(uuid: string, index: number): Promise<boolean> {
    return commands.main.moveGroup(uuid, index);
  },

  /** 创建实例（占位数据），返回实例信息 */
  async createInstance(
    name: string,
    version: string,
    opts?: CreateInstanceOpts,
  ): Promise<InstanceInfoDto> {
    return commands.main.createInstance(name, version, opts?.loader ?? null, opts?.loaderVersion ?? null, opts?.group ?? null, opts?.modpackType ?? null, opts?.source ?? null);
  },

  /** 重命名实例（核心实例目录跟随改名，重名抛错） */
  async renameInstance(uuid: string, name: string): Promise<boolean> {
    return commands.main.renameInstance(uuid, name);
  },

  /** 更新实例元信息（补丁式，Partial<InstanceInfoDto>） */
  async updateInstance(uuid: string, patch: Partial<InstanceInfoDto>): Promise<boolean> {
    return commands.main.updateInstance(uuid, patch);
  },

  /** 获取实例启动参数（核心实例读配置，遗留数据读内存缓存） */
  async getInstanceArgs(uuid: string): Promise<InstanceArgsDto> {
    return commands.main.getInstanceArgs(uuid);
  },

  /** 更新实例启动参数（核心实例写配置并保存） */
  async updateInstanceArgs(uuid: string, args: InstanceArgsDto): Promise<boolean> {
    return commands.main.updateInstanceArgs(uuid, args);
  },

  /** 删除实例（含文件，进回收站；后台执行避免卡 UI） */
  async deleteInstance(uuid: string): Promise<boolean> {
    return commands.main.deleteInstance(uuid);
  },

  /** 移动实例到 (分组 uuid, 组内位置)，支持同组排序与跨组移动（group 为 null = 默认分组） */
  async moveInstance(uuid: string, group: string | null, index: number): Promise<boolean> {
    return commands.main.moveInstance(uuid, group, index);
  },

  /** 启动游戏（占位：标记运行 + 发事件；启动用户名由后端从当前账户解析） */
  async launchGame(uuid: string): Promise<void> {
    return commands.main.launchGame(uuid);
  },

  /** 停止游戏 */
  async stopGame(uuid: string): Promise<void> {
    return commands.main.stopGame(uuid);
  },

  /** 获取实例的实时运行日志快照（log_get_runtime） */
  async getRuntimeLog(uuid: string): Promise<LogLine[]> {
    return commands.log.getRuntime(uuid);
  },

  /** 获取运行中实例 uuid 列表 */
  async getRunning(): Promise<string[]> {
    return commands.main.getRunning();
  },

  /** 列出实例日志文件（logs 与 crash-reports 目录，绝对路径） */
  async getLogFiles(uuid: string): Promise<string[]> {
    return commands.log.getFiles(uuid);
  },

  /** 读取单个日志文件（path 为 getLogFiles 返回的路径） */
  async readLogFile(uuid: string, path: string): Promise<LogLine[]> {
    return commands.log.readFile(uuid, path);
  },

  /** 获取实例导出信息（在线 / 本地模组分组 + 内容目录有无） */
  async getExportInfo(uuid: string): Promise<ExportInfoDto> {
    return commands.export.getInfo(uuid);
  },

  /** 发起导出（后台任务，进度经 onExportProgress 事件上报） */
  async runExport(uuid: string, config: ExportConfigDto): Promise<void> {
    return commands.export.run(uuid, config);
  },

  /** 获取下载状态快照（任务 + 线程 + 总体速度，下载管理窗口轮询） */
  async getDownloadStatus(): Promise<DownloadStatusDto> {
    return commands.download.getStatus();
  },

  /** 全局暂停所有下载（期间新增任务同样暂停），返回被暂停的任务数 */
  async pauseAllDownloads(): Promise<number> {
    return commands.download.pauseAll();
  },

  /** 全局恢复所有下载，返回被恢复的任务数 */
  async resumeAllDownloads(): Promise<number> {
    return commands.download.resumeAll();
  },

  /** 全局停止（取消）所有下载，返回被停止的任务数 */
  async stopAllDownloads(): Promise<number> {
    return commands.download.cancelAll();
  },

  // ---------------- 收藏 ----------------

  /** 获取收藏数据（收藏项 + 分组） */
  async collectGetData(): Promise<CollectDataDto> {
    return commands.collect.getData();
  },

  /** 取项目图标地址（收藏时没存到图标的老条目回源用），取不到返回 null */
  async collectProjectIcon(source: string, pid: string): Promise<string | null> {
    return commands.collect.projectIcon(source, pid);
  },

  /** 登记一个原始图片网址，返回可直接放到 src 上的地址（收藏里存的是原始网址） */
  async collectImageUrl(url: string): Promise<string> {
    return commands.collect.imageUrl(url);
  },

  /**
   * 取项目条目（收藏窗口跳转到下载窗口时用）
   *
   * 收藏条目只有 名字 / 图标 / 网址 / 源 / 项目ID，下载次数、更新时间、收藏状态
   * 得现取一次 —— 详情页要显示它们。
   */
  async collectProjectItem(source: string, pid: string, fileType: string) {
    return commands.collect.projectItem(source, pid, fileType);
  },

  /** 添加分组（重名会抛错） */
  async collectAddGroup(name: string): Promise<void> {
    return commands.collect.addGroup(name);
  },

  /** 删除分组（组内的收藏条目保留） */
  async collectRemoveGroup(name: string): Promise<void> {
    return commands.collect.removeGroup(name);
  },

  /** 清空收藏：group 为空 = 清空全部（分组保留），否则只清空该分组 */
  async collectClear(group: string | null): Promise<void> {
    return commands.collect.clear(group);
  },

  /** 移除收藏：group 为空 = 从收藏删除，否则只从该分组移除 */
  async collectRemoveItems(uuids: string[], group: string | null): Promise<void> {
    return commands.collect.removeItems(uuids, group);
  },

  /** 把收藏加入分组 */
  async collectSetGroupItems(group: string, uuids: string[]): Promise<void> {
    return commands.collect.setGroupItems(group, uuids);
  },

  /** 收藏 / 取消收藏在线项目（列表与详情里的星标） */
  async collectStar(
    source: string,
    fileType: string,
    pid: string,
    name: string,
    icon: string | null,
    url: string,
    star: boolean,
  ): Promise<void> {
    return commands.collect.star(source, fileType, pid, name, icon, url, star);
  },

  // ---------------- 从 ColorMC 迁移（首次启动，见 windows/colormc.rs） ----------------

  /** 探测本机的 ColorMC 工作目录；已经问过（或本机没有）返回 null */
  async checkColorMc(): Promise<ColorMcInfoDto | null> {
    return commands.colormc.checkColormc();
  },

  /** 执行迁移（copy = 复制并保留源目录 / move = 按条目重命名搬走并删掉空源目录） */
  async migrateColorMc(source: string, mode: "copy" | "move"): Promise<ColorMcReportDto> {
    return commands.colormc.migrateColormc(source, mode);
  },

  /** 选择「不迁移」：记下标记，以后不再询问 */
  async skipColorMc(): Promise<void> {
    return commands.colormc.skipColormc();
  },
};

// ---------------- 事件订阅（Rust emit → 前端 listen） ----------------

/** 游戏日志事件（uuid + 日志行；clear = true 时前端清屏） */
export function onGameLog(cb: (e: LogEvent) => void): Promise<UnlistenFn> {
  return listen<LogEvent>(GameLog, (e) => cb(e.payload));
}
/** ColorMC 迁移进度事件（stage：scan / copy / move / check） */
export function onColormcProgress(
  cb: (e: ColorMcProgressDto) => void,
): Promise<UnlistenFn> {
  return listen<ColorMcProgressDto>(ColormcProgress, (e) => cb(e.payload));
}
/** 日志窗口切换目标实例事件（窗口已存在时再次打开，壳层推送新目标） */
export function onLogFocus(cb: (uuid: string) => void): Promise<UnlistenFn> {
  return listen<LogFocusDto>(LogFocus, (e) => cb(e.payload.uuid));
}
/** 客户端设置变更事件（设置窗口保存后广播，payload 为新 client 配置） */
export function onClientConfigChange(cb: (e: ClientConfigDto) => void): Promise<UnlistenFn> {
  return listen<ClientConfigDto>(ClientConfigChange, (e) => cb(e.payload));
}
/** 实例导出进度事件（state：running / done / failed） */
export function onExportProgress(
  cb: (e: ExportProgressDto) => void,
): Promise<UnlistenFn> {
  return listen<ExportProgressDto>(ExportProgress, (e) => cb(e.payload));
}
/** 导出窗口切换目标实例事件（窗口已存在时再次打开，壳层推送新目标） */
export function onExportFocus(cb: (uuid: string) => void): Promise<UnlistenFn> {
  return listen<LogFocusDto>(ExportFocus, (e) => cb(e.payload.uuid));
}
/** 启动状态事件（state：launching 等） */
export function onLaunchState(cb: (e: StateEvent) => void): Promise<UnlistenFn> {
  return listen<StateEvent>(LaunchState, (e) => cb(e.payload));
}
/** 游戏退出事件（code = 进程退出码） */
export function onGameExit(cb: (e: ExitEvent) => void): Promise<UnlistenFn> {
  return listen<ExitEvent>(GameExit, (e) => cb(e.payload));
}
/** 启动失败事件 */
export function onLaunchError(cb: (e: ErrorEvent) => void): Promise<UnlistenFn> {
  return listen<ErrorEvent>(LaunchError, (e) => cb(e.payload));
}
/** 实例数据变更（type：add / edit / remove / group） */
export function onInstanceChange(cb: (type: string) => void): Promise<UnlistenFn> {
  return listen<{ type: string }>(InstanceChange, (e) => cb(e.payload.type));
}
export function onJavaChange(cb: () => void): Promise<UnlistenFn> {
  return listen(JavaChange, () => cb());
}

/** 自定义主页面内容变更（导入 / 删除后广播，主窗口据此重拉状态） */
export function onCustomHomeChange(cb: () => void): Promise<UnlistenFn> {
  return listen(CustomHomeChange, () => cb());
}

/** 下载任务状态事件（type：add / remove / update） */
export function onDownloadTask(cb: (e: DownloadTaskEvent) => void): Promise<UnlistenFn> {
  return listen<DownloadTaskEvent>(DownloadTask, (e) => cb(e.payload));
}

/** 下载线程当前文件状态事件 */
export function onDownloadItem(cb: (e: DownloadItemEvent) => void): Promise<UnlistenFn> {
  return listen<DownloadItemEvent>(DownloadItem, (e) => cb(e.payload));
}

/** 关闭被拒绝（窗口处于关闭保护时，前端弹提示说明原因） */
export function onCloseBlocked(cb: () => void): Promise<UnlistenFn> {
  return listen(CloseBlocked, () => cb());
}

/** 加载器支持列表查询进度（step / total） */
export function onAddLoaderProgress(cb: (e: { step: number; total: number }) => void): Promise<UnlistenFn> {
  return listen<{ step: number; total: number }>(AddLoaderProgress, (e) => cb(e.payload));
}

/** 实例重名确认（kind：overwrite 覆盖 / rename 自动改名） */
export function onAddNameConflict(
  cb: (e: { id: number; kind: string; name: string }) => void,
): Promise<UnlistenFn> {
  return listen<{ id: number; kind: string; name: string }>(AddNameConflict, (e) => cb(e.payload));
}

/** 整合包安装进度（本地压缩包 / 在线整合包安装共用） */
export function onAddPackProgress(cb: (e: PackProgressDto) => void): Promise<UnlistenFn> {
  return listen<PackProgressDto>(AddPackProgress, (e) => cb(e.payload));
}

/** 整合包安装任务总览（多任务进度条，下载整合包窗口 / 主窗口共用） */
export function onAddModpackStatus(cb: (e: ModPackStatusDto) => void): Promise<UnlistenFn> {
  return listen<ModPackStatusDto>(AddModpackStatus, (e) => cb(e.payload));
}

/** 资源下载任务总览事件（任务增删 / 进度变化 / 移除时广播） */
export function onAddResourceStatus(cb: (e: ResourceStatusDto) => void): Promise<UnlistenFn> {
  return listen<ResourceStatusDto>(AddResourceStatus, (e) => cb(e.payload));
}

export function answerNameConflict(id: number, answer: boolean): Promise<void> {
  return commands.add.answerNameConflict(id, answer);
}

/** 收藏变更（跨窗口同步） */
export function onCollectChange(cb: () => void): Promise<UnlistenFn> {
  return listen(CollectChange, () => cb());
}

// ==================== 方块列表 ====================

/** 获取方块列表（按语言翻译显示名） */
export function getBlockList(lang: string): Promise<BlockItemDto[]> {
  return commands.block.list(lang);
}

/** 获取方块贴图渲染状态 */
export function getBlockStatus(): Promise<BlockStatusDto> {
  return commands.block.status();
}

/** 开始渲染方块贴图（force = 全量重渲染；已在进行返回 false） */
export function blockRenderStart(force: boolean): Promise<boolean> {
  return commands.block.renderStart(force);
}

/** 取消正在进行的方块贴图渲染（返回是否有渲染可取消） */
export function blockRenderCancel(): Promise<boolean> {
  return commands.block.renderCancel();
}

/** 把方块贴图设为实例图标 */
export function blockSetIcon(uuid: string, id: string): Promise<boolean> {
  return commands.block.setIcon(uuid, id);
}

/** 按用户名或UUID添加皮肤方块（同名覆盖），返回方块ID */
export function blockSkinAdd(input: string): Promise<string> {
  return commands.block.skinAdd(input);
}

/** 删除皮肤方块（名字即皮肤方块显示名） */
export function blockSkinRemove(name: string): Promise<void> {
  return commands.block.skinRemove(name);
}

/** 方块渲染状态事件（进度 / 结束） */
export function onBlockRender(cb: (e: BlockStatusDto) => void): Promise<UnlistenFn> {
  return listen<BlockStatusDto>(BlockRender, (e) => cb(e.payload));
}

/** mml-image 协议访问前缀（拼实例图标等本地图片地址用） */
export function getImageBaseUrl(): Promise<string> {
  return commands.main.imageBaseUrl();
}

/** 核心加载状态（启动兜底：load-done 事件可能在页面监听前就发出） */
export function getLoadState(): Promise<LoadState> {
  return commands.main.loadState();
}

// ==================== 实例资源管理 ====================

/** 模组列表（解析 jar 元数据，条目多时耗时数秒） */
export function listMods(uuid: string): Promise<ModItemDto[]> {
  return commands.resource.listMods(uuid);
}
/** 模组扫描进度事件（`x/x`）：只在扫描期间来，用于加载占位上显示进度 */
export function onModScanProgress(cb: (e: ModScanProgressDto) => void): Promise<UnlistenFn> {
  return listen<ModScanProgressDto>(ResourceListModsProgress, (e) => cb(e.payload));
}
/**
 * 启用模组（去掉 .disable / .disabled 后缀）
 *
 * 返回**改名后的新身份**（uuid / 文件名 / 相对路径）：文件名变了，uuid（路径的 v5）
 * 也跟着变，调用方必须拿它更新本地那一条 —— 否则再点一次会按旧 uuid 找不到文件。
 * 有了它就不必重扫整个 mods 目录。
 */
export function enableMod(uuid: string, modUuid: string): Promise<ModRenameDto> {
  return commands.resource.modEnable(uuid, modUuid);
}
/** 禁用模组（追加 .disable 后缀）；返回同上 */
export function disableMod(uuid: string, modUuid: string): Promise<ModRenameDto> {
  return commands.resource.modDisable(uuid, modUuid);
}
/** 删除模组（进回收站） */
export function deleteMod(uuid: string, modUuid: string): Promise<void> {
  return commands.resource.deleteMod(uuid, modUuid);
}

/** 取该实例的模组自定义分组（按用户拖出来的顺序；分组用 uuid 作键） */
export function getModGroups(uuid: string): Promise<ModGroupDto[]> {
  return commands.resource.modGroups(uuid);
}
/** 新建模组分组（重名会抛错）；**返回新分组的 uuid**，顺序表 / 折叠集合要用它 */
export function addModGroup(uuid: string, name: string): Promise<string> {
  return commands.resource.modGroupAdd(uuid, name);
}
/** 删除模组分组（传分组 uuid；组内模组回到"未分组"，不动磁盘文件） */
export function removeModGroup(uuid: string, group: string): Promise<void> {
  return commands.resource.modGroupRemove(uuid, group);
}
/**
 * 取分组块的**完整顺序**（含三个状态分组的固定 uuid）
 *
 * 必须从后端读：状态分组不在分组表里，前端自己拼不出用户把它们排到了哪儿。
 */
export function getModGroupOrder(uuid: string): Promise<string[]> {
  return commands.resource.modGroupOrder(uuid);
}
/** 取收起的分组块键集合（分组 uuid，与分组表同一套键口径） */
export function getModGroupsCollapsed(uuid: string): Promise<string[]> {
  return commands.resource.modGroupsCollapsed(uuid);
}
/** 保存收起的分组块键集合（分组 uuid；空数组 = 全展开） */
export function setModGroupsCollapsed(uuid: string, collapsed: string[]): Promise<void> {
  return commands.resource.modGroupCollapsedSet(uuid, collapsed);
}
/** 重命名模组分组（传分组 uuid；只改名字，键与成员都不动） */
export function renameModGroup(uuid: string, group: string, newName: string): Promise<void> {
  return commands.resource.modGroupRename(uuid, group, newName);
}
/** 把若干模组移到某个分组（传分组 uuid）；group 传 null = 移出所有分组 */
export function setModGroup(uuid: string, group: string | null, keys: string[]): Promise<void> {
  return commands.resource.modGroupSet(uuid, group, keys);
}
/**
 * 某实例的资源窗口视图偏好（左侧分类顺序 / 上次类别 / 模组展示方式 / 表格列宽）
 *
 * 存在**实例**的 `gui_setting.json` 里（跟着实例走），不是全局界面状态。
 */
export function getResourceView(uuid: string): Promise<ResourceViewDto> {
  return commands.resource.viewGet(uuid);
}
/** 保存资源窗口视图偏好（整份覆盖这一块；排序与默认值的口径在前端 useResourceView） */
export function setResourceView(
  uuid: string,
  order: string[],
  category: string,
  modView: string,
  modColWidths: Record<string, number>,
): Promise<void> {
  return commands.resource.viewSet(uuid, order, category, modView, modColWidths);
}
/**
 * 保存分组块的显示顺序
 *
 * `order` 是**分组 uuid**（状态分组用 `STATE_GROUP_ID_*` 那几个固定 uuid，
 * 与后端 `resource.rs::STATE_GROUP_*` 同源），自建分组就是它自己的 uuid。
 */
export function setModGroupOrder(uuid: string, order: string[]): Promise<void> {
  return commands.resource.modGroupOrderSet(uuid, order);
}
/** 写模组备注（传空串 = 删掉这条备注）；file 传列表里的原始文件名即可 */
export function setModNote(uuid: string, file: string, note: string): Promise<void> {
  return commands.resource.modNoteSet(uuid, file, note);
}

/**
 * 材质包列表
 *
 * `lang` 传当前界面语言（`locale.value`）：简介写成 `translate` 组件时，
 * 后端按它查资源包自带的语言表
 */
export function listResourcepacks(uuid: string, lang: string): Promise<PackItemDto[]> {
  return commands.resource.listResourcepacks(uuid, lang);
}
/** 删除材质包（进回收站） */
export function deleteResourcepack(uuid: string, file: string): Promise<void> {
  return commands.resource.deleteResourcepack(uuid, file);
}

/**
 * 启用材质包（把这一条加进 options.txt 的 `resourcePacks`）
 *
 * `file` 传列表里的文件名（`PackItemDto.file`）。
 * 只动 options.txt，**列表里那一行的 `enable` 要调用方自己就地改** —— 重拉一次列表
 * 要把每个包重新算 SHA1 / SHA256 并解一遍 `pack.mcmeta`，而启用状态只跟 options.txt
 * 有关，包本身一个字都没变。
 */
export function enableResourcepack(uuid: string, file: string): Promise<void> {
  return commands.resource.resourcepackEnable(uuid, file);
}

/** 禁用材质包（上面那条的反向操作，口径见 [`enableResourcepack`]） */
export function disableResourcepack(uuid: string, file: string): Promise<void> {
  return commands.resource.resourcepackDisable(uuid, file);
}

/** 存档列表 */
export function listSaves(uuid: string): Promise<SaveItemDto[]> {
  return commands.resource.listSaves(uuid);
}
/** 删除存档（进回收站） */
export function deleteSave(uuid: string, dir: string): Promise<void> {
  return commands.resource.deleteSave(uuid, dir);
}
/** 备份存档（zip 到实例备份目录），返回备份文件名 */
export function backupSave(uuid: string, dir: string): Promise<string> {
  return commands.resource.backupSave(uuid, dir);
}
/** 某个存档的备份列表（按时间倒序，最近的在最上面） */
export function listSaveBackups(uuid: string, dir: string): Promise<SaveBackupDto[]> {
  return commands.resource.listSaveBackups(uuid, dir);
}
/**
 * 还原某个备份
 *
 * **破坏性**：当前存档整目录会先移入回收站，再把这个备份解开覆盖上去。
 */
export function restoreSaveBackup(uuid: string, dir: string, file: string): Promise<void> {
  return commands.resource.restoreSaveBackup(uuid, dir, file);
}

/** 截图列表 */
export function listScreenshots(uuid: string): Promise<ScreenshotItemDto[]> {
  return commands.resource.listScreenshots(uuid);
}
/** 删除截图（进回收站） */
export function deleteScreenshot(uuid: string, name: string): Promise<void> {
  return commands.resource.deleteScreenshot(uuid, name);
}
/** 清空全部截图（进回收站） */
export function clearScreenshots(uuid: string): Promise<void> {
  return commands.resource.clearScreenshots(uuid);
}

/** 打开资源目录（name 为空打开目录本身，否则资源管理器选中该文件；
 * datapacks 类别需传 parent = 存档目录名） */
export function openResourceFolder(
  uuid: string,
  kind: string,
  name: string | null,
  parent: string | null = null,
): Promise<void> {
  return commands.resource.openFolder(uuid, kind, name, parent);
}

// ==================== 服务器 ====================

/** 服务器列表（servers.dat） */
export function listServers(uuid: string): Promise<ServerItemDto[]> {
  return commands.resource.listServers(uuid);
}
/** 添加服务器 */
/**
 * 查询服务器 MOTD（地址 host 或 host:port，端口缺省 25565）
 *
 * 与上面 `api.getMotd` 是同一个命令；资源窗口的服务器列表用的是这个具名版本
 * （那一层其它调用都是具名导出）。两边共用这一份实现，别再各写一遍
 */
export function getMotd(address: string): Promise<MotdDto> {
  return commands.main.getMotd(address);
}

/** 添加服务器 */
export function addServer(uuid: string, name: string, ip: string): Promise<void> {
  return commands.resource.serverAdd(uuid, name, ip);
}
/** 编辑服务器（按原 name + ip 定位） */
export function updateServer(
  uuid: string,
  name: string,
  ip: string,
  newName: string,
  newIp: string,
  acceptTextures: boolean,
): Promise<void> {
  return commands.resource.serverUpdate(uuid, name, ip, newName, newIp, acceptTextures);
}
/** 删除服务器 */
export function deleteServer(uuid: string, name: string, ip: string): Promise<void> {
  return commands.resource.serverDelete(uuid, name, ip);
}

// ==================== 光影包 ====================

/** 光影包列表（selected 标出当前启用的包） */
export function listShaderpacks(uuid: string): Promise<ShaderItemDto[]> {
  return commands.resource.listShaderpacks(uuid);
}
/** 启用 / 停用光影包（null = 停用，options.txt 写 OFF） */
export function setShader(uuid: string, file: string | null): Promise<void> {
  return commands.resource.shaderSet(uuid, file);
}
/** 删除光影包（进回收站） */
export function deleteShaderpack(uuid: string, file: string): Promise<void> {
  return commands.resource.deleteShaderpack(uuid, file);
}

// ==================== 结构文件 ====================

/** 结构文件列表 */
export function listSchematics(uuid: string): Promise<SchematicItemDto[]> {
  return commands.resource.listSchematics(uuid);
}
/** 删除结构文件（进回收站） */
export function deleteSchematic(uuid: string, file: string): Promise<void> {
  return commands.resource.deleteSchematic(uuid, file);
}

// ==================== 数据包（存档子页） ====================

/** 存档的数据包列表 */
export function listDatapacks(uuid: string, dir: string): Promise<DataPackItemDto[]> {
  return commands.resource.listDatapacks(uuid, dir);
}
/** 切换数据包启用状态 */
export function toggleDatapack(uuid: string, dir: string, name: string): Promise<void> {
  return commands.resource.datapackToggle(uuid, dir, name);
}
/** 删除数据包（清 level.dat 引用 + 文件进回收站） */
export function deleteDatapack(uuid: string, dir: string, name: string): Promise<void> {
  return commands.resource.datapackDelete(uuid, dir, name);
}
