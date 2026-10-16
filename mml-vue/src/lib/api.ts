// M²L 前端 API（真实 IPC 实现）
//
// 数据全部从 Rust 侧获取，命令见 mml-gui/src-tauri/src/windows/；
// 按钮执行的操作也通过 IPC 调用。仅在 Tauri 环境可用（纯浏览器会报错）。
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { commands } from "./bindings";
import type {
  BlockGroupsDto,
  BlockItemDto,
  BlockStatusDto,
  BytesSliceDto,
  CollectDataDto,
  ColorMcInfoDto,
  ColorMcProgressDto,
  ColorMcReportDto,
  ConfigHunkDto,
  ConfigNbtDto,
  ConfigTextDto,
  DataPackItemDto,
  DepProblemDto,
  DetectedPackDto,
  LogLine,
  DownloadItemEvent,
  DownloadStatusDto,
  DownloadTaskEvent,
  ErrorEvent,
  ExitEvent,
  FileListDto,
  FileItemDto,
  FolderInstanceDto,
  ClientConfigDto,
  ExportConfigDto,
  ExportEntryDto,
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
  LogSettingDto,
  McaChunkDto,
  ModGroupDto,
  ModDuplicateDto,
  ResourceViewDto,
  ModItemDto,
  ModRenameDto,
  ModScanProgressDto,
  MotdDto,
  NbtEditDto,
  ModPackStatusDto,
  NewsItem,
  PackItemDto,
  PackProgressDto,
  ProjectDetailDto,
  ProjectDto,
  ResourceDependencyDto,
  ResourceSaveDto,
  ResourceStatusDto,
  SaveBackupDto,
  SaveEditDto,
  SaveItemDto,
  ScreenshotItemDto,
  ServerItemDto,
  ShaderItemDto,
  SchematicDetailDto,
  SchematicItemDto,
  StatsDataDto,
  StateEvent,
  SystemMemoryDto,
  VersionInfoDto,
  ViewMode,
} from "./bindings";
import { AddLoaderProgress, AddModpackStatus, AddNameConflict, AddPackProgress, AddResourceStatus, BlockRender, ClientConfigChange, CloseBlocked, CollectChange, ColormcProgress, CustomHomeChange, DownloadItem, DownloadTask, ExportFocus, ExportProgress, GameExit, GameLog, GuiViewModeChange, InstanceChange, JavaChange, LaunchError, LaunchState, ResourceListModsProgress, ResourceModsDuplicate } from "./listens";

export interface CreateInstanceOpts {
  loader?: string;
  loaderVersion?: string | null;
  /** 目标分组 uuid；后端原样存下不做解析，null / 缺省表示不归组 */
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

  /**
   * 获取分组列表
   *
   * uuid 是身份，name 只是显示名；含空分组，默认分组排在最前且名字为空白
   *
   * # 返回值
   *
   * 全部分组
   */
  async getGroups(): Promise<GroupDto[]> {
    return commands.main.getGroups();
  },

  /**
   * 把"用户输入的分组名"解析成分组 uuid
   *
   * 添加实例 / 安装整合包两处都是手输或选组名，而 IPC 一律按 uuid 传，
   * 所以在这个边界上转一次
   *
   * - `name`: 手输或选中的分组名，空名表示默认分组
   *
   * # 返回值
   *
   * 已有同名分组就用它，没有就现建一个；空名返回 null
   */
  async resolveGroupId(name: string): Promise<string | null> {
    const trimmed = name.trim();
    if (!trimmed) return null;
    const found = (await commands.main.getGroups()).find((g) => g.name === trimmed);
    if (found) return found.uuid;
    return await commands.main.addGroup(trimmed);
  },

  /**
   * 获取实例的游戏内语言列表
   *
   * - `uuid`: 实例 uuid
   *
   * # 返回值
   *
   * 从资源索引里的 minecraft/lang/*.json 查出，资源未下载时为空
   *
   * 每项带显示名 `name`，取自语言文件里的 `language.name`，如 zh_cn → 简体中文；
   * 读不到时它就是 `code`，所以直接显示 `name` 即可
   */
  async getInstanceLangs(uuid: string): Promise<InstanceLangDto[]> {
    return commands.main.getInstanceLangs(uuid);
  },

  /**
   * 获取 Java 列表
   *
   * 配置加载与扫描异步进行，未完成时为空
   *
   * # 返回值
   *
   * 当前已知的全部 Java
   */
  async getJavaList(): Promise<JavaInfoDto[]> {
    return commands.main.getJavaList();
  },

  /**
   * 获取本机内存，单位 MiB，用于内存设置显示的参考值
   *
   * # 返回值
   *
   * 总内存与可用内存；查询失败时后端返回 0，调用方按不可用处理，不显示
   */
  async getSystemMemory(): Promise<SystemMemoryDto> {
    return commands.main.getSystemMemory();
  },

  /**
   * 获取游戏版本列表
   *
   * 结果缓存在后端进程里
   *
   * # 返回值
   *
   * 全部游戏版本
   */
  async getVersions(): Promise<VersionInfoDto[]> {
    return commands.main.getVersions();
  },

  /**
   * 强制刷新版本列表
   *
   * 清空后端缓存后重新拉取
   *
   * # 返回值
   *
   * 重新拉取到的全部游戏版本
   */
  async refreshVersions(): Promise<VersionInfoDto[]> {
    return commands.main.refreshVersions();
  },

  /**
   * 获取 Minecraft 官方新闻
   *
   * - `page`: 页码，从 1 起，缺省 1
   *
   * # 返回值
   *
   * 该页的新闻条目
   */
  async getNews(page = 1): Promise<NewsItem[]> {
    return commands.main.getNews(page);
  },

  /**
   * 用系统浏览器打开网址
   *
   * - `url`: 要打开的完整网址
   */
  async openUrl(url: string): Promise<void> {
    return commands.main.openUrl(url);
  },

  /**
   * 从头新建实例，版本 + 加载器
   *
   * - `name`: 实例名，同时作为实例目录名
   * - `version`: 游戏版本号
   * - `loader`: 加载器 ID，取值见 `addGetLoaders`
   * - `loaderVersion`: 加载器版本，没有就传 null
   * - `group`: 分组 uuid，null 表示默认分组
   *
   * # 返回值
   *
   * 新实例的 uuid
   */
  async addCreateNew(
    name: string,
    version: string,
    loader: string,
    loaderVersion: string | null,
    group: string | null,
  ): Promise<string> {
    return commands.add.createNew(name, version, loader, loaderVersion, group);
  },

  /**
   * 导入文件夹为实例
   *
   * - `path`: 待导入的文件夹路径
   * - `name`: 实例名
   * - `group`: 分组 uuid，null = 默认分组
   *
   * # 返回值
   *
   * 新实例的 uuid
   */
  async addImportFolder(path: string, name: string, group: string | null): Promise<string> {
    return commands.add.importFolder(path, name, group);
  },

  /**
   * 导入整合包压缩包
   *
   * - `path`: 压缩包路径
   * - `packType`: 压缩包类型 ID，取值见 `addGetPackTypes`
   * - `name`: 实例名
   * - `group`: 分组 uuid，null = 默认分组
   * - `unselect`: 按包内完整条目名排除的文件，取自前端文件树未勾选项
   *
   * # 返回值
   *
   * 新实例的 uuid
   */
  async addImportArchive(
    path: string,
    packType: string,
    name: string,
    group: string | null,
    unselect: string[] | null,
  ): Promise<string> {
    return commands.add.importArchive(path, packType, name, group, unselect);
  },

  /**
   * 从网址安装实例
   *
   * - `url`: 压缩包下载地址
   * - `name`: 实例名
   * - `group`: 分组 uuid，null = 默认分组
   *
   * # 返回值
   *
   * 新实例的 uuid
   */
  async addImportUrl(url: string, name: string, group: string | null): Promise<string> {
    return commands.add.importUrl(url, name, group);
  },

  /**
   * 检测压缩包的整合包类型与推荐实例名
   *
   * - `path`: 压缩包路径
   *
   * # 返回值
   *
   * 识别出的类型与推荐实例名；识别失败抛错
   */
  async addDetectArchive(path: string): Promise<DetectedPackDto> {
    return commands.add.detectArchive(path);
  },

  // ---------------- 在线整合包 ----------------

  /**
   * 获取整合包下载源 ID 列表
   *
   * # 返回值
   *
   * 取值是 curseforge / modrinth
   */
  async getModpackSources(): Promise<string[]> {
    return commands.addResource.sourceType();
  },

  /**
   * 获取某下载源的排序方式列表
   *
   * - `source`: 下载源 ID
   *
   * # 返回值
   *
   * 取值即后端枚举线串，可原样回传
   */
  async getModpackSorts(source: string): Promise<string[]> {
    return commands.addResource.sortType(source);
  },

  /**
   * 获取某下载源整合包的分类
   *
   * - `source`: 下载源 ID
   *
   * # 返回值
   *
   * 键是传给后端的分类值，值是显示名
   */
  async getModpackCategories(source: string): Promise<Record<string, string>> {
    return commands.addResource.categories(source, "modpack");
  },

  /**
   * 获取某下载源支持的游戏版本列表
   *
   * - `source`: 下载源 ID
   *
   * # 返回值
   *
   * 该下载源支持的全部游戏版本
   */
  async getModpackVersions(source: string): Promise<string[]> {
    return commands.addResource.gameVersions(source);
  },

  /**
   * 搜索在线整合包
   *
   * - `source`: 下载源 ID
   * - `page`: 页码，从 0 开始
   * - `sort`: 排序方式 ID，取值来自 `getModpackSorts`
   * - `category`: 分类 ID，null 表示不限
   * - `filter`: 搜索词，null 表示不限
   * - `version`: 游戏版本过滤，null 表示不限
   *
   * # 返回值
   *
   * 该页的项目列表与命中总数
   */
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

  /**
   * 获取某整合包的可安装版本列表
   *
   * - `source`: 下载源 ID
   * - `projectId`: 整合包项目 ID
   * - `page`: 页码，从 0 开始
   * - `version`: 游戏版本过滤，null 表示不限
   *
   * # 返回值
   *
   * 该页文件列表、总数与最大页数
   */
  async getModpackFiles(
    source: string,
    projectId: string,
    page: number,
    version: string | null,
  ): Promise<FileListDto> {
    return commands.addModpack.file(source, projectId, page, version);
  },

  /**
   * 获取某整合包的项目详情
   *
   * - `source`: 下载源 ID
   * - `projectId`: 整合包项目 ID
   *
   * # 返回值
   *
   * 项目详情；Modrinth 含 markdown 正文，CurseForge 只有简介
   */
  async getModpackDetail(source: string, projectId: string): Promise<ProjectDetailDto> {
    return commands.addModpack.detail(source, projectId);
  },

  /**
   * 安装在线整合包
   *
   * - `source`: 下载源 ID
   * - `projectId`: 整合包项目 ID
   * - `fileId`: 文件 ID
   * - `group`: 分组 uuid，null = 默认分组
   *
   * 多任务：命令立即返回，任务后台安装，进度走 add-modpack-status 事件
   */
  async installModpack(
    source: string,
    projectId: string,
    fileId: string,
    group: string | null,
  ): Promise<void> {
    return commands.addModpack.install(source, projectId, fileId, group);
  },

  /**
   * 查询整合包安装任务总览
   *
   * 挂载时同步一次，之后靠事件
   *
   * # 返回值
   *
   * 全部安装任务
   */
  async getModpackStatus(): Promise<ModPackStatusDto> {
    return commands.addModpack.status();
  },

  // ---------------- 在线资源（实例设置 → 添加资源） ----------------

  /**
   * 获取某下载源某资源类型的分类
   *
   * - `source`: 下载源 ID
   * - `fileType`: 资源类型，取值 `mod` / `resourcepack` / `shaderpack` / `save` / `datapacks`
   *
   * # 返回值
   *
   * 键是传给后端的分类值，值是显示名；CurseForge 的数据包不支持按分类过滤，返回空表
   */
  async getResourceCategories(source: string, fileType: string): Promise<Record<string, string>> {
    return commands.addResource.categories(source, fileType);
  },

  /**
   * 获取某下载源的排序方式列表
   *
   * - `source`: 下载源 ID
   *
   * # 返回值
   *
   * 取值即后端枚举线串，可原样回传
   */
  async getResourceSorts(source: string): Promise<string[]> {
    return commands.addResource.sortType(source);
  },

  /**
   * 获取某下载源支持的游戏版本列表
   *
   * - `source`: 下载源 ID
   *
   * # 返回值
   *
   * 该下载源支持的全部游戏版本
   */
  async getResourceVersions(source: string): Promise<string[]> {
    return commands.addResource.gameVersions(source);
  },

  /**
   * 搜索在线资源
   *
   * - `game`: 实例 uuid，用来标记已下载 / 正在下载 / 已收藏
   * - `source`: 下载源 ID
   * - `fileType`: 资源类型
   * - `page`: 页码，从 0 开始
   * - `sort`: 排序方式 ID，取值来自 `getResourceSorts`
   * - `category`: 分类 ID，null 表示不限
   * - `filter`: 搜索关键字，null 表示不限
   * - `version`: 游戏版本过滤，null 表示不限
   * - `loader`: 模组加载器过滤，只有模组类型有意义，null 表示不限
   *
   * # 返回值
   *
   * 该页项目列表与命中总数
   */
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

  /**
   * 获取某资源的可下载版本列表
   *
   * - `game`: 实例 uuid，用来标记已下载 / 正在下载
   * - `source`: 下载源 ID
   * - `projectId`: 项目 ID
   * - `fileType`: 资源类型
   * - `page`: 页码，从 0 开始
   * - `version`: 游戏版本过滤，null 表示不限
   * - `loader`: 模组加载器过滤，只有模组类型有意义，null 表示不限
   *
   * # 返回值
   *
   * 该页文件列表、总数与最大页数
   */
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

  /**
   * 获取某资源的项目详情
   *
   * - `source`: 下载源 ID
   * - `projectId`: 项目 ID
   *
   * # 返回值
   *
   * 项目详情；Modrinth 含 markdown 正文，CurseForge 只有简介
   */
  async getResourceDetail(source: string, projectId: string): Promise<ProjectDetailDto> {
    return commands.addResource.detail(source, projectId);
  },

  /**
   * 拉取某个版本需要的前置，含前置的前置
   *
   * - `game`: 实例 uuid，用来按实例的版本 / 加载器挑文件，并跳过实例里已经装了的
   * - `source`: 下载源 ID
   * - `projectId`: 项目 ID
   * - `fileId`: 文件 ID
   * - `fileType`: 资源类型；只有模组有前置，其余类型返回空表
   *
   * # 返回值
   *
   * 待勾选的前置；某一项 `fid` 为空串表示该前置在本实例的版本 / 加载器下没有可用文件
   */
  async getResourceDependencies(
    game: string,
    source: string,
    projectId: string,
    fileId: string,
    fileType: string,
  ): Promise<ResourceDependencyDto[]> {
    return commands.addResource.dependencies(game, source, projectId, fileId, fileType);
  },

  /**
   * 获取实例的存档列表
   *
   * - `game`: 实例 uuid
   *
   * # 返回值
   *
   * 未损坏的存档，下载数据包时用来选目标
   */
  async getResourceSaves(game: string): Promise<ResourceSaveDto[]> {
    return commands.addResource.saves(game);
  },

  /**
   * 下载资源到实例
   *
   * - `game`: 实例 uuid
   * - `source`: 下载源 ID
   * - `projectId`: 项目 ID
   * - `fileId`: 文件 ID
   * - `fileType`: 资源类型，取值 `mod` / `resourcepack` / `shaderpack` / `save` / `datapacks`
   * - `world`: 目标存档的目录名，只有数据包必填，其余类型忽略
   *
   * 命令立即返回，下载走全局下载器
   */
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

  /**
   * 查询资源下载任务总览
   *
   * 挂载时同步一次，之后靠事件
   *
   * # 返回值
   *
   * 全部下载任务
   */
  async getResourceStatus(): Promise<ResourceStatusDto> {
    return commands.addResource.status();
  },

  /**
   * 取消一个进行中的整合包安装任务
   *
   * - `pid`: 项目 ID
   * - `fid`: 文件 ID，与 pid 一起定位这一条
   */
  async cancelModpackInstall(pid: string, fid: string): Promise<void> {
    return commands.addModpack.cancel(pid, fid);
  },

  /** 清除已结束（完成 / 失败）的整合包安装任务 */
  async clearModpackDone(): Promise<void> {
    return commands.addModpack.clearDone();
  },

  /** 清除已结束（完成 / 失败）的资源下载条目 */
  async clearResourceDone(): Promise<void> {
    return commands.addResource.clearDone();
  },

  /**
   * 取消进行中的安装任务
   *
   * # 返回值
   *
   * 确实取消了返回 true；没有进行中的任务返回 false
   */
  async addCancel(): Promise<boolean> {
    return commands.add.cancel();
  },

  /**
   * 获取加载器的可用版本列表
   *
   * - `loader`: 加载器 ID，取值见 `addGetLoaders`
   * - `mc`: 游戏版本号
   *
   * # 返回值
   *
   * 该加载器在 `mc` 下的版本号列表
   */
  async addLoaderVersions(loader: string, mc: string): Promise<string[]> {
    return commands.add.getLoaderVersions(loader, mc);
  },

  /**
   * 获取加载器 ID 列表
   *
   * # 返回值
   *
   * 全部加载器 ID，顺序由内核定
   */
  async addGetLoaders(): Promise<string[]> {
    return commands.add.getLoaders();
  },

  /**
   * 查询指定游戏版本支持的加载器 ID 列表
   *
   * - `mc`: 游戏版本号
   *
   * # 返回值
   *
   * 该游戏版本支持的加载器 ID
   */
  async addGetSupportLoaders(mc: string): Promise<string[]> {
    return commands.add.getSupportLoaders(mc);
  },

  /**
   * 设置添加实例窗口关闭保护
   *
   * - `enabled`: 为 true 期间拒绝关闭请求
   */
  async setCloseGuard(enabled: boolean): Promise<void> {
    await commands.add.setCloseGuard(enabled);
  },

  /**
   * 确保某个窗口 kind 的模型存在
   *
   * - `kind`: 窗口类型
   *
   * 单窗口模式下页面挂载时调用；多窗口模式下模型跟着真实窗口走，这里重复调用无副作用
   */
  async ensureWindowModel(kind: string): Promise<void> {
    await commands.windows.ensureModel(kind);
  },

  /**
   * 释放某个窗口 kind 的模型
   *
   * - `kind`: 窗口类型
   *
   * 单窗口模式下页面切走或关闭时调用，别让模型常驻
   */
  async dropWindowModel(kind: string): Promise<void> {
    await commands.windows.dropModel(kind);
  },

  /**
   * 扫描文件夹里可导入的实例
   *
   * - `path`: 待扫描的目录
   *
   * # 返回值
   *
   * 每个可导入的实例目录各占一项，由前端列出来让用户勾选
   *
   * 选到 `.minecraft` 这类装着若干实例的目录时用：官方启动器看 `versions/*`，
   * MMC 看 `instances/*`，而不是把整个 `.minecraft` 当成一个实例
   */
  async addScanFolder(path: string): Promise<FolderInstanceDto[]> {
    return commands.add.scanFolder(path);
  },

  /**
   * 获取压缩包类型 ID 列表
   *
   * # 返回值
   *
   * 全部压缩包类型 ID
   */
  async addGetPackTypes(): Promise<string[]> {
    return commands.add.getPackTypes();
  },

  /**
   * 获取游戏版本类型 ID 列表
   *
   * # 返回值
   *
   * 取值是 release / snapshot / old_beta / old_alpha
   */
  async addGetVersionTypes(): Promise<string[]> {
    return commands.add.getVersionTypes();
  },

  /**
   * 新建分组
   *
   * - `name`: 分组名
   *
   * # 返回值
   *
   * 新分组的 uuid；名字为空或重名返回 null
   */
  async addGroup(name: string): Promise<string | null> {
    return commands.main.addGroup(name);
  },

  /**
   * 删除分组
   *
   * 组内实例移入默认分组。
   *
   * - `uuid`: 分组 uuid；为空或非法即默认分组，不可删
   *
   * # 返回值
   *
   * 真删掉了返回 true
   */
  async removeGroup(uuid: string): Promise<boolean> {
    return commands.main.removeGroup(uuid);
  },

  /**
   * 调整分组显示顺序
   *
   * - `uuid`: 分组 uuid
   * - `index`: 目标位置，从 0 起，超出表长按表尾
   *
   * # 返回值
   *
   * 挪成功返回 true
   */
  async moveGroup(uuid: string, index: number): Promise<boolean> {
    return commands.main.moveGroup(uuid, index);
  },

  /**
   * 创建实例
   *
   * - `name`: 实例名，同时作为实例目录名
   * - `version`: 游戏版本号
   * - `opts`: 可选的加载器、分组、整合包类型，见 `CreateInstanceOpts`
   *
   * # 返回值
   *
   * 新实例的信息；实例数据是占位的
   */
  async createInstance(
    name: string,
    version: string,
    opts?: CreateInstanceOpts,
  ): Promise<InstanceInfoDto> {
    return commands.main.createInstance(name, version, opts?.loader ?? null, opts?.loaderVersion ?? null, opts?.group ?? null, opts?.modpackType ?? null, opts?.source ?? null);
  },

  /**
   * 重命名实例
   *
   * - `uuid`: 实例 uuid
   * - `name`: 新实例名
   *
   * # 返回值
   *
   * 改名成功返回 true；核心实例目录跟随改名，重名抛错
   */
  async renameInstance(uuid: string, name: string): Promise<boolean> {
    return commands.main.renameInstance(uuid, name);
  },

  /**
   * 更新实例元信息
   *
   * - `uuid`: 实例 uuid
   * - `patch`: 只带要改的字段，补丁式
   *
   * # 返回值
   *
   * 更新成功返回 true
   */
  async updateInstance(uuid: string, patch: Partial<InstanceInfoDto>): Promise<boolean> {
    return commands.main.updateInstance(uuid, patch);
  },

  /**
   * 获取实例启动参数
   *
   * - `uuid`: 实例 uuid
   *
   * # 返回值
   *
   * 启动参数；核心实例读配置，遗留数据读内存缓存，两处都取不到时是默认值
   */
  async getInstanceArgs(uuid: string): Promise<InstanceArgsDto> {
    return commands.main.getInstanceArgs(uuid);
  },

  /**
   * 更新实例启动参数
   *
   * - `uuid`: 实例 uuid
   * - `args`: 前端提交的启动参数，整份覆盖
   *
   * # 返回值
   *
   * 写入成功返回 true；核心实例写配置并保存
   */
  async updateInstanceArgs(uuid: string, args: InstanceArgsDto): Promise<boolean> {
    return commands.main.updateInstanceArgs(uuid, args);
  },

  /**
   * 删除实例
   *
   * - `uuid`: 实例 uuid
   *
   * # 返回值
   *
   * 删除成功返回 true
   *
   * 连实例文件一起进回收站，后台执行避免卡 UI
   */
  async deleteInstance(uuid: string): Promise<boolean> {
    return commands.main.deleteInstance(uuid);
  },

  /**
   * 移动实例到指定分组与组内位置
   *
   * - `uuid`: 实例 uuid
   * - `group`: 目标分组 uuid，null = 默认分组
   * - `index`: 组内目标位置
   *
   * # 返回值
   *
   * 移动成功返回 true
   *
   * 同组排序与跨组移动都走这一条
   */
  async moveInstance(uuid: string, group: string | null, index: number): Promise<boolean> {
    return commands.main.moveInstance(uuid, group, index);
  },

  /**
   * 启动游戏
   *
   * - `uuid`: 实例 uuid
   *
   * 目前是占位实现：只标记运行并发事件；启动用户名由后端从当前账户解析
   */
  async launchGame(uuid: string): Promise<void> {
    return commands.main.launchGame(uuid);
  },

  /**
   * 停止游戏
   *
   * - `uuid`: 实例 uuid
   */
  async stopGame(uuid: string): Promise<void> {
    return commands.main.stopGame(uuid);
  },

  /**
   * 获取实例的实时运行日志快照
   *
   * - `uuid`: 实例 uuid
   *
   * # 返回值
   *
   * 当前已产出的日志行
   */
  async getRuntimeLog(uuid: string): Promise<LogLine[]> {
    return commands.log.getRuntime(uuid);
  },

  /**
   * 获取运行中实例 uuid 列表
   *
   * # 返回值
   *
   * 运行中实例的 uuid，顺序不定
   */
  async getRunning(): Promise<string[]> {
    return commands.main.getRunning();
  },

  /**
   * 列出实例日志文件
   *
   * - `uuid`: 实例 uuid
   *
   * # 返回值
   *
   * logs 与 crash-reports 目录下的日志文件绝对路径
   */
  async getLogFiles(uuid: string): Promise<string[]> {
    return commands.log.getFiles(uuid);
  },

  /**
   * 读取单个日志文件
   *
   * - `uuid`: 实例 uuid
   * - `path`: 文件绝对路径，取自 `getLogFiles`
   *
   * # 返回值
   *
   * 该文件的全部日志行
   */
  async readLogFile(uuid: string, path: string): Promise<LogLine[]> {
    return commands.log.readFile(uuid, path);
  },

  /**
   * 读取实例的日志窗口设置
   *
   * - `uuid`: 实例 uuid
   *
   * # 返回值
   *
   * 自动换行 / 自动下拉 / 五个等级开关；存在实例的 `gui_setting.json` 里，没写过是默认值
   */
  async getLogSetting(uuid: string): Promise<LogSettingDto> {
    return commands.log.getSetting(uuid);
  },

  /**
   * 保存实例的日志窗口设置
   *
   * - `uuid`: 实例 uuid
   * - `setting`: 新的日志设置，整份回写
   */
  async setLogSetting(uuid: string, setting: LogSettingDto): Promise<void> {
    return commands.log.setSetting(uuid, setting);
  },

  /**
   * 把日志上传到 mclo.gs
   *
   * - `uuid`: 实例 uuid
   * - `file`: 日志文件绝对路径，取自 `getLogFiles`；`null` 表示当前运行日志
   *
   * # 返回值
   *
   * 可分享的网址；没有内容或上传失败时抛错
   */
  async uploadLog(uuid: string, file: string | null): Promise<string> {
    return commands.log.upload(uuid, file);
  },

  /**
   * 获取实例导出信息
   *
   * - `uuid`: 实例 uuid
   *
   * # 返回值
   *
   * 实例元数据与 `mods` 目录下的模组，有在线来源的带上项目号与下载地址
   */
  async getExportInfo(uuid: string): Promise<ExportInfoDto> {
    return commands.export.getInfo(uuid);
  },

  /**
   * 浏览游戏目录下的一个目录，给导出窗口的文件树当懒加载用
   *
   * - `uuid`: 实例 uuid
   * - `dir`: 相对游戏目录的路径；空串表示游戏目录本身
   *
   * # 返回值
   *
   * 该目录的直接子项，目录在前、同类按路径排序
   */
  async listExportDir(uuid: string, dir: string): Promise<ExportEntryDto[]> {
    return commands.export.listDir(uuid, dir);
  },

  /**
   * 发起导出
   *
   * - `uuid`: 实例 uuid
   * - `config`: 导出配置，见 `ExportConfigDto`
   *
   * 后台任务，进度经 onExportProgress 事件上报
   */
  async runExport(uuid: string, config: ExportConfigDto): Promise<void> {
    return commands.export.run(uuid, config);
  },

  /**
   * 获取下载状态快照
   *
   * # 返回值
   *
   * 任务列表、各线程状态与总体速度；下载管理窗口轮询它
   */
  async getDownloadStatus(): Promise<DownloadStatusDto> {
    return commands.download.getStatus();
  },

  /**
   * 全局暂停所有下载
   *
   * # 返回值
   *
   * 被暂停的任务数；暂停期间新增的任务同样暂停
   */
  async pauseAllDownloads(): Promise<number> {
    return commands.download.pauseAll();
  },

  /**
   * 全局恢复所有下载
   *
   * # 返回值
   *
   * 被恢复的任务数
   */
  async resumeAllDownloads(): Promise<number> {
    return commands.download.resumeAll();
  },

  /**
   * 全局停止所有下载
   *
   * # 返回值
   *
   * 被停止的任务数
   */
  async stopAllDownloads(): Promise<number> {
    return commands.download.cancelAll();
  },

  // ---------------- 收藏 ----------------

  /**
   * 获取收藏数据
   *
   * # 返回值
   *
   * 全部收藏项与分组
   */
  async collectGetData(): Promise<CollectDataDto> {
    return commands.collect.getData();
  },

  /**
   * 取项目图标地址
   *
   * - `source`: 下载源平台 ID
   * - `pid`: 项目 ID
   *
   * # 返回值
   *
   * 图标地址；取不到返回 null
   *
   * 收藏时没存到图标的老条目靠它回源
   */
  async collectProjectIcon(source: string, pid: string): Promise<string | null> {
    return commands.collect.projectIcon(source, pid);
  },

  /**
   * 登记一个原始图片网址
   *
   * - `url`: 原始图片网址
   *
   * # 返回值
   *
   * 可直接放到 src 上的地址；收藏里存的是原始网址
   */
  async collectImageUrl(url: string): Promise<string> {
    return commands.collect.imageUrl(url);
  },

  /**
   * 取项目条目，收藏窗口跳转到下载窗口时用
   *
   * - `source`: 下载源平台 ID
   * - `pid`: 项目 ID
   * - `fileType`: 资源类型
   *
   * # 返回值
   *
   * 项目条目
   *
   * 收藏条目只有名字 / 图标 / 网址 / 源 / 项目 ID，下载次数、更新时间、收藏状态
   * 得现取一次，详情页要显示它们
   */
  async collectProjectItem(source: string, pid: string, fileType: string) {
    return commands.collect.projectItem(source, pid, fileType);
  },

  /**
   * 添加分组
   *
   * - `name`: 分组名；为空或已存在会抛错
   */
  async collectAddGroup(name: string): Promise<void> {
    return commands.collect.addGroup(name);
  },

  /**
   * 删除分组
   *
   * - `name`: 分组名
   *
   * 组内的收藏条目保留
   */
  async collectRemoveGroup(name: string): Promise<void> {
    return commands.collect.removeGroup(name);
  },

  /**
   * 清空收藏
   *
   * - `group`: 分组名；为空表示清空全部收藏但保留分组，否则只清空该分组的成员
   */
  async collectClear(group: string | null): Promise<void> {
    return commands.collect.clear(group);
  },

  /**
   * 移除收藏
   *
   * - `uuids`: 目标收藏项 uuid 列表
   * - `group`: 分组名；为空表示从收藏中删除并从所有分组移除，否则只从该分组移除
   */
  async collectRemoveItems(uuids: string[], group: string | null): Promise<void> {
    return commands.collect.removeItems(uuids, group);
  },

  /**
   * 把收藏加入分组
   *
   * - `group`: 目标分组名
   * - `uuids`: 要加入该分组的收藏项 uuid 列表
   */
  async collectSetGroupItems(group: string, uuids: string[]): Promise<void> {
    return commands.collect.setGroupItems(group, uuids);
  },

  /**
   * 收藏或取消收藏在线项目
   *
   * - `source`: 下载源平台 ID
   * - `fileType`: 文件类型 ID
   * - `pid`: 项目 ID
   * - `name`: 项目名
   * - `icon`: 图标网址，可缺省
   * - `url`: 项目网页地址
   * - `star`: 为真加入收藏，为假按下载源加项目 ID 移除
   *
   * 整合包与资源窗口的列表和详情星标都走它
   */
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

  // ---------------- 从 ColorMC 迁移：首次启动，见 windows/colormc.rs ----------------

  /**
   * 探测本机的 ColorMC 工作目录
   *
   * # 返回值
   *
   * 探测到的目录信息；已经问过、没探测到、或探测结果就是运行目录时返回 null
   */
  async checkColorMc(): Promise<ColorMcInfoDto | null> {
    return commands.colormc.checkColormc();
  },

  /**
   * 执行迁移并返回兼容性报告
   *
   * - `source`: ColorMC 工作目录，把 `checkColorMc` 给的那份传回来
   * - `mode`: `copy` 复制并保留源目录，`move` 按条目重命名搬走并删掉空源目录
   *
   * # 返回值
   *
   * 迁移与兼容性检测的报告
   */
  async migrateColorMc(source: string, mode: "copy" | "move"): Promise<ColorMcReportDto> {
    return commands.colormc.migrateColormc(source, mode);
  },

  /** 选择「不迁移」：记下标记，以后不再询问 */
  async skipColorMc(): Promise<void> {
    return commands.colormc.skipColormc();
  },
};

// ---------------- 事件订阅：Rust emit → 前端 listen ----------------

/**
 * 游戏日志事件
 *
 * - `cb`: 收到日志行时回调，含实例 uuid 与日志内容
 *
 * # 返回值
 *
 * 取消监听的函数
 *
 * `clear` 为 true 时前端清屏
 */
export function onGameLog(cb: (e: LogEvent) => void): Promise<UnlistenFn> {
  return listen<LogEvent>(GameLog, (e) => cb(e.payload));
}
/**
 * ColorMC 迁移进度事件
 *
 * - `cb`: 进度回调，`stage` 取值 scan / copy / move / check
 *
 * # 返回值
 *
 * 取消监听的函数
 */
export function onColormcProgress(
  cb: (e: ColorMcProgressDto) => void,
): Promise<UnlistenFn> {
  return listen<ColorMcProgressDto>(ColormcProgress, (e) => cb(e.payload));
}
/**
 * 客户端设置变更事件
 *
 * - `cb`: 收到新配置时回调
 *
 * # 返回值
 *
 * 取消监听的函数
 *
 * 设置窗口保存后广播，payload 为新 client 配置
 */
export function onClientConfigChange(cb: (e: ClientConfigDto) => void): Promise<UnlistenFn> {
  return listen<ClientConfigDto>(ClientConfigChange, (e) => cb(e.payload));
}
/**
 * 实例列表显示模式变更事件
 *
 * - `cb`: 收到新模式时回调，取值 list / group / grid
 *
 * # 返回值
 *
 * 取消监听的函数
 *
 * 设置窗口改了显示模式后广播，payload 为变更后的模式
 */
export function onGuiViewModeChange(cb: (mode: ViewMode) => void): Promise<UnlistenFn> {
  return listen<ViewMode>(GuiViewModeChange, (e) => cb(e.payload));
}
/**
 * 实例导出进度事件
 *
 * - `cb`: 进度回调，`state` 取值 running / done / failed
 *
 * # 返回值
 *
 * 取消监听的函数
 */
export function onExportProgress(
  cb: (e: ExportProgressDto) => void,
): Promise<UnlistenFn> {
  return listen<ExportProgressDto>(ExportProgress, (e) => cb(e.payload));
}
/**
 * 导出窗口切换目标实例事件
 *
 * - `cb`: 收到新目标实例 uuid 时回调
 *
 * # 返回值
 *
 * 取消监听的函数
 *
 * 窗口已存在时再次打开，壳层推送新目标
 */
export function onExportFocus(cb: (uuid: string) => void): Promise<UnlistenFn> {
  return listen<LogFocusDto>(ExportFocus, (e) => cb(e.payload.uuid));
}
/**
 * 启动状态事件
 *
 * - `cb`: 状态变化时回调，`state` 取值 launching 等
 *
 * # 返回值
 *
 * 取消监听的函数
 */
export function onLaunchState(cb: (e: StateEvent) => void): Promise<UnlistenFn> {
  return listen<StateEvent>(LaunchState, (e) => cb(e.payload));
}
/**
 * 游戏退出事件
 *
 * - `cb`: 退出时回调，`code` 是进程退出码
 *
 * # 返回值
 *
 * 取消监听的函数
 */
export function onGameExit(cb: (e: ExitEvent) => void): Promise<UnlistenFn> {
  return listen<ExitEvent>(GameExit, (e) => cb(e.payload));
}
/**
 * 启动失败事件
 *
 * - `cb`: 失败时回调
 *
 * # 返回值
 *
 * 取消监听的函数
 */
export function onLaunchError(cb: (e: ErrorEvent) => void): Promise<UnlistenFn> {
  return listen<ErrorEvent>(LaunchError, (e) => cb(e.payload));
}
/**
 * 实例数据变更
 *
 * - `cb`: 变更时回调，`type` 取值 add / edit / remove / group
 *
 * # 返回值
 *
 * 取消监听的函数
 */
export function onInstanceChange(cb: (type: string) => void): Promise<UnlistenFn> {
  return listen<{ type: string }>(InstanceChange, (e) => cb(e.payload.type));
}
/**
 * Java 列表变更事件
 *
 * - `cb`: 变更时回调
 *
 * # 返回值
 *
 * 取消监听的函数
 */
export function onJavaChange(cb: () => void): Promise<UnlistenFn> {
  return listen(JavaChange, () => cb());
}

/**
 * 自定义主页面内容变更
 *
 * - `cb`: 变更时回调
 *
 * # 返回值
 *
 * 取消监听的函数
 *
 * 导入或删除后广播，主窗口据此重拉状态
 */
export function onCustomHomeChange(cb: () => void): Promise<UnlistenFn> {
  return listen(CustomHomeChange, () => cb());
}

/**
 * 下载任务状态事件
 *
 * - `cb`: 任务变化时回调，`type` 取值 add / remove / update
 *
 * # 返回值
 *
 * 取消监听的函数
 */
export function onDownloadTask(cb: (e: DownloadTaskEvent) => void): Promise<UnlistenFn> {
  return listen<DownloadTaskEvent>(DownloadTask, (e) => cb(e.payload));
}

/**
 * 下载线程当前文件状态事件
 *
 * - `cb`: 状态变化时回调
 *
 * # 返回值
 *
 * 取消监听的函数
 */
export function onDownloadItem(cb: (e: DownloadItemEvent) => void): Promise<UnlistenFn> {
  return listen<DownloadItemEvent>(DownloadItem, (e) => cb(e.payload));
}

/**
 * 关闭被拒绝
 *
 * - `cb`: 被拒绝时回调
 *
 * # 返回值
 *
 * 取消监听的函数
 *
 * 窗口处于关闭保护时触发，前端弹提示说明原因
 */
export function onCloseBlocked(cb: () => void): Promise<UnlistenFn> {
  return listen(CloseBlocked, () => cb());
}

/**
 * 加载器支持列表查询进度
 *
 * - `cb`: 进度回调，含已完成的 `step` 与总数 `total`
 *
 * # 返回值
 *
 * 取消监听的函数
 */
export function onAddLoaderProgress(cb: (e: { step: number; total: number }) => void): Promise<UnlistenFn> {
  return listen<{ step: number; total: number }>(AddLoaderProgress, (e) => cb(e.payload));
}

/**
 * 实例重名确认
 *
 * - `cb`: 需要用户确认时回调，`kind` 取值 overwrite 覆盖 / rename 自动改名
 *
 * # 返回值
 *
 * 取消监听的函数
 */
export function onAddNameConflict(
  cb: (e: { id: number; kind: string; name: string }) => void,
): Promise<UnlistenFn> {
  return listen<{ id: number; kind: string; name: string }>(AddNameConflict, (e) => cb(e.payload));
}

/**
 * 整合包安装进度
 *
 * - `cb`: 进度回调
 *
 * # 返回值
 *
 * 取消监听的函数
 *
 * 本地压缩包与在线整合包安装共用
 */
export function onAddPackProgress(cb: (e: PackProgressDto) => void): Promise<UnlistenFn> {
  return listen<PackProgressDto>(AddPackProgress, (e) => cb(e.payload));
}

/**
 * 整合包安装任务总览
 *
 * - `cb`: 任务变化时回调
 *
 * # 返回值
 *
 * 取消监听的函数
 *
 * 给多任务进度条用，下载整合包窗口与主窗口共用
 */
export function onAddModpackStatus(cb: (e: ModPackStatusDto) => void): Promise<UnlistenFn> {
  return listen<ModPackStatusDto>(AddModpackStatus, (e) => cb(e.payload));
}

/**
 * 资源下载任务总览事件
 *
 * - `cb`: 总览变化时回调
 *
 * # 返回值
 *
 * 取消监听的函数
 *
 * 任务增删、进度变化与移除时广播
 */
export function onAddResourceStatus(cb: (e: ResourceStatusDto) => void): Promise<UnlistenFn> {
  return listen<ResourceStatusDto>(AddResourceStatus, (e) => cb(e.payload));
}

/**
 * 回答实例重名确认对话框
 *
 * - `id`: 确认请求 id，取自 onAddNameConflict
 * - `answer`: 用户是否同意
 */
export function answerNameConflict(id: number, answer: boolean): Promise<void> {
  return commands.add.answerNameConflict(id, answer);
}

/**
 * 收藏变更
 *
 * - `cb`: 变更时回调
 *
 * # 返回值
 *
 * 取消监听的函数
 *
 * 跨窗口同步用
 */
export function onCollectChange(cb: () => void): Promise<UnlistenFn> {
  return listen(CollectChange, () => cb());
}

// ==================== 方块列表 ====================

/**
 * 获取方块列表
 *
 * - `lang`: 界面语言，`en_us` 取英文，其余一律按中文
 *
 * # 返回值
 *
 * 方块与物品条目，顺序即游戏创造栏顺序
 */
export function getBlockList(lang: string): Promise<BlockItemDto[]> {
  return commands.block.list(lang);
}

/**
 * 获取方块贴图渲染状态
 *
 * # 返回值
 *
 * 当前渲染状态快照
 */
export function getBlockStatus(): Promise<BlockStatusDto> {
  return commands.block.status();
}

/**
 * 取方块窗口的用户数据（左栏顺序 / 自建分组 / 收藏）
 *
 * 这些是**用户数据**：按仓库约定由 Rust 存本地文件（`block_groups.json`），
 * 不走前端 localStorage。
 *
 * # 返回值
 *
 * 见 `BlockGroupsDto`
 */
export function getBlockGroups(): Promise<BlockGroupsDto> {
  return commands.block.getGroups();
}

/**
 * 整份写回方块窗口的用户数据；落盘是后端异步做的
 *
 * - `data`: 新的完整数据
 */
export function setBlockGroups(data: BlockGroupsDto): Promise<void> {
  return commands.block.setGroups(data);
}

/**
 * 开始渲染方块贴图
 *
 * - `force`: 是否忽略版本短路、强制全量重渲染
 *
 * # 返回值
 *
 * 成功启动返回 true；已在渲染中返回 false
 */
export function blockRenderStart(force: boolean): Promise<boolean> {
  return commands.block.renderStart(force);
}

/**
 * 取消正在进行的方块贴图渲染
 *
 * # 返回值
 *
 * 确实有渲染在进行并已发出取消返回 true；没有可取消的返回 false
 */
export function blockRenderCancel(): Promise<boolean> {
  return commands.block.renderCancel();
}

/**
 * 把方块贴图设为实例图标
 *
 * - `uuid`: 实例 uuid
 * - `id`: 方块或物品 id，形如 `minecraft:stone`
 *
 * # 返回值
 *
 * 成功写入返回 true；该 id 没有可用贴图时抛错
 */
export function blockSetIcon(uuid: string, id: string): Promise<boolean> {
  return commands.block.setIcon(uuid, id);
}

/**
 * 按用户名或 UUID 添加皮肤方块
 *
 * - `input`: 玩家名或 uuid
 *
 * # 返回值
 *
 * 新皮肤方块的 id；同名玩家重复添加即覆盖，等于刷新图标
 */
export function blockSkinAdd(input: string): Promise<string> {
  return commands.block.skinAdd(input);
}

/**
 * 删除皮肤方块
 *
 * - `name`: 皮肤方块的显示名，即添加时用的玩家名
 */
export function blockSkinRemove(name: string): Promise<void> {
  return commands.block.skinRemove(name);
}

/**
 * 方块渲染状态事件
 *
 * - `cb`: 进度或结束时回调
 *
 * # 返回值
 *
 * 取消监听的函数
 */
export function onBlockRender(cb: (e: BlockStatusDto) => void): Promise<UnlistenFn> {
  return listen<BlockStatusDto>(BlockRender, (e) => cb(e.payload));
}

/**
 * mml-image 协议访问前缀
 *
 * # 返回值
 *
 * 自定义图片协议的基础地址，拼实例图标等本地图片地址用
 */
export function getImageBaseUrl(): Promise<string> {
  return commands.main.imageBaseUrl();
}

/**
 * 核心加载状态
 *
 * # 返回值
 *
 * 加载状态；`ok` 表示核心是否已就绪，失败详情走事件
 *
 * 启动兜底用：load-done 事件可能在页面监听前就发出
 */
export function getLoadState(): Promise<LoadState> {
  return commands.main.loadState();
}

// ==================== 实例资源管理 ====================

/**
 * 模组列表
 *
 * - `uuid`: 实例 uuid
 *
 * # 返回值
 *
 * 全部模组；要解析 jar 元数据，条目多时耗时数秒
 */
export function listMods(uuid: string): Promise<ModItemDto[]> {
  return commands.resource.listMods(uuid);
}
/**
 * 模组依赖检测：检查各包声明的强制前置是否齐全、版本是否满足
 *
 * - `uuid`: 实例 uuid
 *
 * # 返回值
 *
 * 逐条问题（缺前置 / 版本不符 / 前置被禁用）；没有问题时空数组
 *
 * 要重新扫一遍 mods 目录，与打开模组分类同一代价，界面要给忙碌态
 */
export function modsDepTest(uuid: string): Promise<DepProblemDto[]> {
  return commands.resource.modsDepTest(uuid);
}
/**
 * 模组扫描进度事件
 *
 * - `cb`: 扫描期间回调，形如 `x/x`
 *
 * # 返回值
 *
 * 取消监听的函数
 *
 * 只在扫描期间来，用于加载占位上显示进度
 */
export function onModScanProgress(cb: (e: ModScanProgressDto) => void): Promise<UnlistenFn> {
  return listen<ModScanProgressDto>(ResourceListModsProgress, (e) => cb(e.payload));
}
/**
 * 模组重复安装事件
 *
 * - `cb`: 模组列表加载完成时回调，载荷是重复的 modid 与涉及的文件；只有检测到重复才发
 *
 * # 返回值
 *
 * 取消监听的函数
 *
 * 重复不是依赖问题，列表加载完就报，前端据此自动弹窗
 */
export function onModsDuplicate(cb: (e: ModDuplicateDto[]) => void): Promise<UnlistenFn> {
  return listen<ModDuplicateDto[]>(ResourceModsDuplicate, (e) => cb(e.payload));
}
/**
 * 启用模组，去掉 .disable / .disabled 后缀
 *
 * - `uuid`: 实例 uuid
 * - `modUuid`: 模组当前的 uuid
 *
 * # 返回值
 *
 * 改名后的新身份，含 uuid、文件名与相对路径。文件名变了，uuid 是路径的 v5 也跟着变，
 * 调用方必须拿它更新本地那一条，否则再点一次会按旧 uuid 找不到文件；
 * 有了它就不必重扫整个 mods 目录
 */
export function enableMod(uuid: string, modUuid: string): Promise<ModRenameDto> {
  return commands.resource.modEnable(uuid, modUuid);
}
/**
 * 禁用模组，追加 .disable 后缀
 *
 * - `uuid`: 实例 uuid
 * - `modUuid`: 模组当前的 uuid
 *
 * # 返回值
 *
 * 同 [`enableMod`]
 */
export function disableMod(uuid: string, modUuid: string): Promise<ModRenameDto> {
  return commands.resource.modDisable(uuid, modUuid);
}
/**
 * 删除模组
 *
 * - `uuid`: 实例 uuid
 * - `modUuid`: 模组 uuid
 *
 * 文件进回收站
 */
export function deleteMod(uuid: string, modUuid: string): Promise<void> {
  return commands.resource.deleteMod(uuid, modUuid);
}

/**
 * 取该实例的模组自定义分组
 *
 * - `uuid`: 实例 uuid
 *
 * # 返回值
 *
 * 自建分组，按用户拖出来的顺序；分组用 uuid 作键，成员已排序
 */
export function getModGroups(uuid: string): Promise<ModGroupDto[]> {
  return commands.resource.modGroups(uuid);
}
/**
 * 新建模组分组
 *
 * - `uuid`: 实例 uuid
 * - `name`: 分组名；为空或重名会抛错
 *
 * # 返回值
 *
 * 新分组的 uuid，顺序表与折叠集合要用它
 */
export function addModGroup(uuid: string, name: string): Promise<string> {
  return commands.resource.modGroupAdd(uuid, name);
}
/**
 * 删除模组分组
 *
 * - `uuid`: 实例 uuid
 * - `group`: 分组 uuid
 *
 * 组内模组回到未分组，不动磁盘文件
 */
export function removeModGroup(uuid: string, group: string): Promise<void> {
  return commands.resource.modGroupRemove(uuid, group);
}
/**
 * 取分组块的完整顺序，含三个状态分组的固定 uuid
 *
 * - `uuid`: 实例 uuid
 *
 * # 返回值
 *
 * 与真实存在的分组对齐后的分组键序列
 *
 * 必须从后端读：状态分组不在分组表里，前端自己拼不出用户把它们排到了哪儿
 */
export function getModGroupOrder(uuid: string): Promise<string[]> {
  return commands.resource.modGroupOrder(uuid);
}
/**
 * 取收起的分组块键集合
 *
 * - `uuid`: 实例 uuid
 *
 * # 返回值
 *
 * 收起状态的分组 uuid，与分组表同一套键口径；已经删掉的分组不会出现
 */
export function getModGroupsCollapsed(uuid: string): Promise<string[]> {
  return commands.resource.modGroupsCollapsed(uuid);
}
/**
 * 保存收起的分组块键集合
 *
 * - `uuid`: 实例 uuid
 * - `collapsed`: 收起的分组 uuid；空数组 = 全展开
 */
export function setModGroupsCollapsed(uuid: string, collapsed: string[]): Promise<void> {
  return commands.resource.modGroupCollapsedSet(uuid, collapsed);
}
/**
 * 重命名模组分组
 *
 * - `uuid`: 实例 uuid
 * - `group`: 分组 uuid
 * - `newName`: 新分组名
 *
 * 只改名字，键与成员都不动
 */
export function renameModGroup(uuid: string, group: string, newName: string): Promise<void> {
  return commands.resource.modGroupRename(uuid, group, newName);
}
/**
 * 把若干模组移到某个分组
 *
 * - `uuid`: 实例 uuid
 * - `group`: 目标分组 uuid，null = 移出所有分组
 * - `keys`: 模组的 SHA1 列表
 *
 * 一个模组同时只属于一个组
 */
export function setModGroup(uuid: string, group: string | null, keys: string[]): Promise<void> {
  return commands.resource.modGroupSet(uuid, group, keys);
}
/**
 * 某实例的资源窗口视图偏好，含左侧分类顺序、上次类别、模组展示方式与表格列宽
 *
 * - `uuid`: 实例 uuid
 *
 * # 返回值
 *
 * 该实例的视图偏好
 *
 * 存在实例的 `gui_setting.json` 里，跟着实例走，不是全局界面状态
 */
export function getResourceView(uuid: string): Promise<ResourceViewDto> {
  return commands.resource.viewGet(uuid);
}
/**
 * 保存资源窗口视图偏好
 *
 * - `uuid`: 实例 uuid
 * - `order`: 左侧分类顺序
 * - `category`: 上次选中的类别
 * - `modView`: 模组展示方式
 * - `modColWidths`: 模组表格列宽
 *
 * 整份覆盖这一块；排序与默认值的口径在前端 useResourceView
 */
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
 * - `uuid`: 实例 uuid
 * - `order`: 分组 uuid 序列；状态分组用 `STATE_GROUP_ID_*` 那几个固定 uuid，
 *   与后端 `resource.rs::STATE_GROUP_*` 同源，自建分组就是它自己的 uuid
 */
export function setModGroupOrder(uuid: string, order: string[]): Promise<void> {
  return commands.resource.modGroupOrderSet(uuid, order);
}
/**
 * 写模组备注
 *
 * - `uuid`: 实例 uuid
 * - `file`: 列表里的原始文件名，可能带 `.disabled`
 * - `note`: 备注文本，空串表示删除
 */
export function setModNote(uuid: string, file: string, note: string): Promise<void> {
  return commands.resource.modNoteSet(uuid, file, note);
}

/**
 * 材质包列表
 *
 * - `uuid`: 实例 uuid
 * - `lang`: 当前界面语言，一般传 `locale.value`
 *
 * # 返回值
 *
 * 材质包列表；简介写成 `translate` 组件时，后端按 `lang` 查资源包自带的语言表
 */
export function listResourcepacks(uuid: string, lang: string): Promise<PackItemDto[]> {
  return commands.resource.listResourcepacks(uuid, lang);
}
/**
 * 删除材质包
 *
 * - `uuid`: 实例 uuid
 * - `file`: 材质包文件名
 *
 * 文件进回收站
 */
export function deleteResourcepack(uuid: string, file: string): Promise<void> {
  return commands.resource.deleteResourcepack(uuid, file);
}

/**
 * 启用材质包，把这一条加进 options.txt 的 `resourcePacks`
 *
 * - `uuid`: 实例 uuid
 * - `file`: 材质包文件名，取自列表里的 `PackItemDto.file`
 *
 * 只动 options.txt，列表里那一行的 `enable` 要调用方自己就地改：重拉一次列表要把
 * 每个包重新算 SHA1 / SHA256 并解一遍 `pack.mcmeta`，而启用状态只跟 options.txt 有关，
 * 包本身一个字都没变
 */
export function enableResourcepack(uuid: string, file: string): Promise<void> {
  return commands.resource.resourcepackEnable(uuid, file);
}

/**
 * 禁用材质包，上面那条的反向操作，口径见 [`enableResourcepack`]
 *
 * - `uuid`: 实例 uuid
 * - `file`: 材质包文件名
 */
export function disableResourcepack(uuid: string, file: string): Promise<void> {
  return commands.resource.resourcepackDisable(uuid, file);
}

/**
 * 存档列表
 *
 * - `uuid`: 实例 uuid
 *
 * # 返回值
 *
 * 全部存档
 */
export function listSaves(uuid: string): Promise<SaveItemDto[]> {
  return commands.resource.listSaves(uuid);
}
/**
 * 删除存档
 *
 * - `uuid`: 实例 uuid
 * - `dir`: 存档目录名
 *
 * 文件进回收站
 */
export function deleteSave(uuid: string, dir: string): Promise<void> {
  return commands.resource.deleteSave(uuid, dir);
}
/**
 * 编辑存档：游戏模式 / 难度 / 极限模式 / 名字 / 地图种子
 *
 * - `uuid`: 实例 uuid
 * - `dir`: 存档目录名
 * - `info`: 新的信息，字段口径见 `SaveEditDto`
 *
 * 写回存档的 level.dat；实例正在运行时报 err.instanceRunning。
 * 种子走字符串：它在 level.dat 里是 i64，当数字传会被前端丢精度。
 */
export function editSave(uuid: string, dir: string, info: SaveEditDto): Promise<void> {
  return commands.resource.saveEdit(uuid, dir, info);
}
/**
 * 备份存档
 *
 * - `uuid`: 实例 uuid
 * - `dir`: 存档目录名
 *
 * # 返回值
 *
 * 备份文件名；备份是 zip，放在实例备份目录
 */
export function backupSave(uuid: string, dir: string): Promise<string> {
  return commands.resource.backupSave(uuid, dir);
}
/**
 * 某个存档的备份列表
 *
 * - `uuid`: 实例 uuid
 * - `dir`: 存档目录名
 *
 * # 返回值
 *
 * 全部备份，按时间倒序，最近的在最上面
 */
export function listSaveBackups(uuid: string, dir: string): Promise<SaveBackupDto[]> {
  return commands.resource.listSaveBackups(uuid, dir);
}
/**
 * 还原某个备份
 *
 * - `uuid`: 实例 uuid
 * - `dir`: 存档目录名
 * - `file`: 备份文件名
 *
 * 破坏性：当前存档整目录会先移入回收站，再把这个备份解开覆盖上去
 */
export function restoreSaveBackup(uuid: string, dir: string, file: string): Promise<void> {
  return commands.resource.restoreSaveBackup(uuid, dir, file);
}

/**
 * 截图列表
 *
 * - `uuid`: 实例 uuid
 *
 * # 返回值
 *
 * 全部截图
 */
export function listScreenshots(uuid: string): Promise<ScreenshotItemDto[]> {
  return commands.resource.listScreenshots(uuid);
}
/**
 * 删除截图
 *
 * - `uuid`: 实例 uuid
 * - `name`: 截图文件名
 *
 * 文件进回收站
 */
export function deleteScreenshot(uuid: string, name: string): Promise<void> {
  return commands.resource.deleteScreenshot(uuid, name);
}
/**
 * 清空全部截图
 *
 * - `uuid`: 实例 uuid
 *
 * 文件进回收站
 */
export function clearScreenshots(uuid: string): Promise<void> {
  return commands.resource.clearScreenshots(uuid);
}

/**
 * 打开资源目录
 *
 * - `uuid`: 实例 uuid
 * - `kind`: 资源类别，取值 `mods` / `resourcepacks` / `saves` / `screenshots` /
 *   `shaderpacks` / `schematics` / `servers` / `datapacks`
 * - `name`: 要定位的文件名；为空则打开目录本身，否则在资源管理器里选中该文件
 * - `parent`: 只有 `datapacks` 类别要传，即存档目录名
 */
export function openResourceFolder(
  uuid: string,
  kind: string,
  name: string | null,
  parent: string | null = null,
): Promise<void> {
  return commands.resource.openFolder(uuid, kind, name, parent);
}

// ==================== 服务器 ====================

/**
 * 服务器列表
 *
 * - `uuid`: 实例 uuid
 *
 * # 返回值
 *
 * servers.dat 里的全部服务器
 */
export function listServers(uuid: string): Promise<ServerItemDto[]> {
  return commands.resource.listServers(uuid);
}
/**
 * 查询服务器 MOTD
 *
 * - `address`: 服务器地址，可写 host 或 host:port，端口缺省 25565
 *
 * # 返回值
 *
 * MOTD 查询结果；连不上或无数据时 `state` 为对应失败态
 *
 * 与 `api.getMotd` 是同一个命令，资源窗口的服务器列表用的是这个具名版本
 */
export function getMotd(address: string): Promise<MotdDto> {
  return commands.main.getMotd(address);
}

/**
 * 添加服务器
 *
 * - `uuid`: 实例 uuid
 * - `name`: 服务器名
 * - `ip`: 服务器地址
 */
export function addServer(uuid: string, name: string, ip: string): Promise<void> {
  return commands.resource.serverAdd(uuid, name, ip);
}
/**
 * 编辑服务器
 *
 * - `uuid`: 实例 uuid
 * - `name`: 原服务器名
 * - `ip`: 原服务器地址，与原 name 一起定位这一条
 * - `newName`: 新服务器名
 * - `newIp`: 新服务器地址
 * - `acceptTextures`: 是否接受服务器资源包
 */
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
/**
 * 删除服务器
 *
 * - `uuid`: 实例 uuid
 * - `name`: 服务器名
 * - `ip`: 服务器地址
 */
export function deleteServer(uuid: string, name: string, ip: string): Promise<void> {
  return commands.resource.serverDelete(uuid, name, ip);
}

// ==================== 光影包 ====================

/**
 * 光影包列表
 *
 * - `uuid`: 实例 uuid
 *
 * # 返回值
 *
 * 全部光影包，`selected` 标出当前启用的那个
 */
export function listShaderpacks(uuid: string): Promise<ShaderItemDto[]> {
  return commands.resource.listShaderpacks(uuid);
}
/**
 * 启用或停用光影包
 *
 * - `uuid`: 实例 uuid
 * - `file`: 光影包文件名，null = 停用，此时 options.txt 写 OFF
 */
export function setShader(uuid: string, file: string | null): Promise<void> {
  return commands.resource.shaderSet(uuid, file);
}
/**
 * 删除光影包
 *
 * - `uuid`: 实例 uuid
 * - `file`: 光影包文件名
 *
 * 文件进回收站
 */
export function deleteShaderpack(uuid: string, file: string): Promise<void> {
  return commands.resource.deleteShaderpack(uuid, file);
}

// ==================== 结构文件 ====================

/**
 * 结构文件列表
 *
 * - `uuid`: 实例 uuid
 *
 * # 返回值
 *
 * 全部结构文件
 */
export function listSchematics(uuid: string): Promise<SchematicItemDto[]> {
  return commands.resource.listSchematics(uuid);
}
/**
 * 结构文件详情
 *
 * - `uuid`: 实例 uuid
 * - `file`: 结构文件名
 *
 * # 返回值
 *
 * 尺寸、方块总数与种类数，外加逐种方块的数量（按数量从多到少）
 */
export function getSchematicDetail(uuid: string, file: string): Promise<SchematicDetailDto> {
  return commands.resource.schematicDetail(uuid, file);
}
/**
 * 删除结构文件
 *
 * - `uuid`: 实例 uuid
 * - `file`: 结构文件名
 *
 * 文件进回收站
 */
export function deleteSchematic(uuid: string, file: string): Promise<void> {
  return commands.resource.deleteSchematic(uuid, file);
}
/**
 * 在文件管理器里打开实例目录
 *
 * - `uuid`: 实例 uuid
 *
 * 落点是 `instances/<实例名>/`，里面有 game.json 等实例文件；游戏目录 `.minecraft`
 * 走资源管理窗口的「打开文件夹」
 */
export function openInstanceDir(uuid: string): Promise<void> {
  return commands.main.openInstanceDir(uuid);
}
/**
 * 列出某个来源下可编辑的配置文件
 *
 * - `uuid`: 实例 uuid
 * - `source`: 来源，空串为实例本身，否则是存档目录名
 *
 * # 返回值
 *
 * 来源为空串时是 options.txt 与 `config` / `kubejs` / `local` 下的全部文件，
 * 否则是该存档目录下的全部文件；名字都是**相对该来源**的路径
 */
export function listConfigFiles(uuid: string, source = ""): Promise<FileItemDto[]> {
  return commands.config.getFiles(uuid, source);
}
/**
 * 读取一个文本配置文件
 *
 * - `uuid`: 实例 uuid
 * - `source`: 来源，空串为实例本身，否则是存档目录名，与 `listConfigFiles` 一致
 * - `file`: 来源内的相对路径，取自 `listConfigFiles`
 *
 * # 返回值
 *
 * 见 `ConfigTextDto`：全文 + 全文 SHA1；保存时要把这个 SHA1 一起交回去，
 * 后端据此确认磁盘上还是打开时的这一份。二进制文件报 err.fileBinary，
 * 过大报 err.fileTooLarge
 */
export function readConfigFile(
  uuid: string,
  source: string,
  file: string,
): Promise<ConfigTextDto> {
  return commands.config.getFile(uuid, source, file);
}
/**
 * 按行级差异保存文本配置文件
 *
 * - `uuid`: 实例 uuid
 * - `source`: 来源，空串为实例本身，否则是存档目录名
 * - `file`: 来源内的相对路径
 * - `baseSha1`: 打开时那一份的 SHA1，见 `readConfigFile`
 * - `expectedLines`: 改完后的总行数，按 `\n` 切；后端用它交叉校验差异
 * - `hunks`: 行替换列表，行号相对原文，见 `lib/textDiff`
 *
 * # 返回值
 *
 * 保存后全文的 SHA1，供下一次保存接着用；磁盘上那一份已经变了报 err.fileChanged
 */
export function writeConfigFile(
  uuid: string,
  source: string,
  file: string,
  baseSha1: string,
  expectedLines: number,
  hunks: ConfigHunkDto[],
): Promise<string> {
  return commands.config.saveFile(uuid, source, file, baseSha1, expectedLines, hunks);
}
/**
 * 打开一个 NBT 文件，转成可展示、可编辑的树
 *
 * **打开就是真的打开**：文件句柄与解析出的内容留在后端会话里，之后的保存直接在那份状态上改、
 * 由同一个句柄写回；会话还在时再打开一次给的是会话里那棵树，不碰磁盘。要重新读盘就先
 * `closeConfigNbt` 再打开。
 *
 * - `uuid`: 实例 uuid
 * - `source`: 来源，空串为实例本身，否则是存档目录名
 * - `file`: 来源内的相对路径
 *
 * # 返回值
 *
 * 见 `ConfigNbtDto`：树 + **文件字节**的 SHA1；保存时要把这个 SHA1 一起交回去。
 * 数组只展开不超过上限的那些，超长的只报个数
 */
export function openConfigNbt(
  uuid: string,
  source: string,
  file: string,
): Promise<ConfigNbtDto> {
  return commands.config.openNbt(uuid, source, file);
}
/**
 * 关掉一个 NBT 文件的后端会话，句柄跟着释放
 *
 * 切走配置窗口、或不想再保持这个文件打开时调用；下次打开就是重新读盘。
 *
 * - `uuid`: 实例 uuid
 * - `source`: 来源，空串为实例本身，否则是存档目录名
 * - `file`: 来源内的相对路径
 */
export function closeConfigNbt(uuid: string, source: string, file: string): Promise<void> {
  return commands.config.closeNbt(uuid, source, file);
}
/**
 * 按若干处标量修改改写 NBT 文件
 *
 * 只改标量的值，不动结构、不换类型；写回沿用原文件的压缩方式。
 *
 * - `uuid`: 实例 uuid
 * - `source`: 来源，空串为实例本身，否则是存档目录名
 * - `file`: 来源内的相对路径
 * - `baseSha1`: 打开时那一份文件字节的 SHA1，见 `openConfigNbt`
 * - `edits`: 要改的标量，路径相对根标签
 *
 * # 返回值
 *
 * 写回后文件字节的 SHA1，供下一次保存接着用；磁盘上那一份已经变了报 err.fileChanged
 */
export function writeConfigNbt(
  uuid: string,
  source: string,
  file: string,
  baseSha1: string,
  edits: NbtEditDto[],
): Promise<string> {
  return commands.config.setNbt(uuid, source, file, baseSha1, edits);
}
/**
 * 读取区域文件（`.mca`）的区块表
 *
 * - `uuid`: 实例 uuid
 * - `source`: 来源，空串为实例本身，否则是存档目录名
 * - `file`: 来源内的相对路径
 *
 * # 返回值
 *
 * 有数据的区块，按表项下标升序；只读 8 KiB 文件头，不解析区块内容
 */
export function readConfigMca(
  uuid: string,
  source: string,
  file: string,
): Promise<McaChunkDto[]> {
  return commands.config.getMca(uuid, source, file);
}
/**
 * 读取一个图片文件，给图片视图直接用
 *
 * - `uuid`: 实例 uuid
 * - `source`: 来源，空串为实例本身，否则是存档目录名
 * - `file`: 来源内的相对路径
 *
 * # 返回值
 *
 * `data:<mime>;base64,<...>`，可直接塞进 `<img src>`；体积上限与文本一致
 */
export function readConfigImage(uuid: string, source: string, file: string): Promise<string> {
  return commands.config.getImage(uuid, source, file);
}
/**
 * 读取任意文件的一段字节，给二进制视图翻页用
 *
 * - `uuid`: 实例 uuid
 * - `source`: 来源，空串为实例本身，否则是存档目录名
 * - `file`: 来源内的相对路径
 * - `offset`: 起始偏移，单位字节；越界会被夹到文件末尾
 * - `length`: 想要的字节数，上限 64 KiB
 *
 * # 返回值
 *
 * 见 `BytesSliceDto`；`data` 是标准 Base64
 */
export function readConfigBytes(
  uuid: string,
  source: string,
  file: string,
  offset: number,
  length: number,
): Promise<BytesSliceDto> {
  return commands.config.getBytesSlice(uuid, source, file, offset, length);
}

// ==================== 数据包（存档子页） ====================

/**
 * 存档的数据包列表
 *
 * - `uuid`: 实例 uuid
 * - `dir`: 存档目录名
 *
 * # 返回值
 *
 * 该存档的全部数据包
 */
export function listDatapacks(uuid: string, dir: string): Promise<DataPackItemDto[]> {
  return commands.resource.listDatapacks(uuid, dir);
}
/**
 * 切换数据包启用状态
 *
 * - `uuid`: 实例 uuid
 * - `dir`: 存档目录名
 * - `name`: 数据包名
 */
export function toggleDatapack(uuid: string, dir: string, name: string): Promise<void> {
  return commands.resource.datapackToggle(uuid, dir, name);
}
/**
 * 删除数据包
 *
 * - `uuid`: 实例 uuid
 * - `dir`: 存档目录名
 * - `name`: 数据包名
 *
 * 清掉 level.dat 里的引用，文件进回收站
 */
export function deleteDatapack(uuid: string, dir: string, name: string): Promise<void> {
  return commands.resource.datapackDelete(uuid, dir, name);
}
