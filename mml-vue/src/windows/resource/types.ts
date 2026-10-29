// 资源管理窗口的共享类型与分类清单

import type { ModItemDto } from "../../lib/bindings";
import type { GlyphName } from "../../components/ui/GlyphIcon.vue";

/** 资源分类，即左侧导航的一项 */
export type CategoryId =
  | "saves"
  | "mods"
  | "resourcepacks"
  | "screenshots"
  | "servers"
  | "shaders"
  | "schematics";

/** 存档分类下的子页：存档本体 / 数据包 */
export type SaveTab = "saves" | "datapacks";

/** 左侧导航的一类：类别 id + 显示名文案键 + 图标 */
export interface ResourceCategory {
  /** 类别 id，与 `category` 当前值同一套取值 */
  id: CategoryId;
  /**
   * 显示名文案键
   *
   * 存**键**而不是 `t()` 出来的字符串：后者在 setup 阶段就把当时的语言定死了，
   * 切语言时侧栏文字不跟着变（本窗口原先就是这么冻住的）。渲染时再 `t()` 才跟着走。
   */
  labelKey: string;
  /**
   * 图标，取 `GlyphIcon` 的字形名
   *
   * 放在分类定义里而不是各处硬写：左侧导航与内容区标题都要它，
   * 两处各写一份迟早会写成两个图标。
   */
  icon: GlyphName;
}

/**
 * 分类清单：**这里的顺序就是默认顺序**，见 useResourceView 的 normalizeOrder
 *
 * 用户拖过之后以实例里存的那份为准，这里只决定"没拖过时长什么样"。
 * 模组排在存档前面（用户要求）：模组是最常翻的一类，放第一个。
 */
export const RESOURCE_CATEGORIES: ResourceCategory[] = [
  { id: "mods", labelKey: "resource.mods", icon: "package" },
  { id: "saves", labelKey: "resource.saves", icon: "archive" },
  { id: "resourcepacks", labelKey: "resource.resourcepacks", icon: "palette" },
  { id: "screenshots", labelKey: "resource.screenshots", icon: "image" },
  { id: "servers", labelKey: "resource.servers", icon: "server" },
  { id: "shaders", labelKey: "resource.shaders", icon: "sun" },
  { id: "schematics", labelKey: "resource.schematics", icon: "cube" },
];

/**
 * 分类 → 后端的 kind 串
 *
 * 后端认的是**目录名**，光影包那类是 `shaderpacks`，而左侧导航的 id 是 `shaders`。
 * 拖拽导入要按当前面板定落点，得有一份"分类 → 目录"的映射，收在这里；
 * 各面板调 `openFolder` 时仍各自写同一串，值必须与这张表一致。
 */
export const CATEGORY_KIND: Record<CategoryId, string> = {
  mods: "mods",
  saves: "saves",
  resourcepacks: "resourcepacks",
  screenshots: "screenshots",
  servers: "servers",
  shaders: "shaderpacks",
  schematics: "schematics",
};

/**
 * 当前面板对应的后端 kind
 *
 * 存档下的**数据包子页**落点是 `saves/<存档目录名>/datapacks`，所以这一页返回 `datapacks`，
 * 并且必须把当前选中的存档目录名当 `parent` 一起传给后端，见 ResourceWindow 的 dropParent。
 *
 * - `category`: 左侧导航当前选中的分类
 * - `saveTab`: 存档分类下当前选中的子页
 *
 * # 返回值
 *
 * 拖拽导入用的后端目录名
 */
export function dropKindOf(category: CategoryId, saveTab: SaveTab): string {
  if (category === "saves" && saveTab === "datapacks") return "datapacks";
  return CATEGORY_KIND[category];
}

/** 服务器添加 / 编辑的表单草稿；edit=true 时另带原 name/ip 作为定位键 */
export interface ServerFormDraft {
  /** 编辑已有条目，否则是新增 */
  edit: boolean;
  /** 服务器显示名 */
  name: string;
  /** 服务器地址，host 或 host:port */
  ip: string;
  /** 是否接受服务器资源包 */
  acceptTextures: boolean;
  /** 编辑前的显示名，仅 edit=true 时有意义，作为定位键 */
  origName: string;
  /** 编辑前的地址，仅 edit=true 时有意义，作为定位键 */
  origIp: string;
}

/**
 * 模组的展示方式，两种视图见 parts/mod/
 *
 * 列表视图里同时展开 jar-in-jar 的层级，所以层级与图标 / 副标题不必二选一。
 */
export type ModView = "list" | "table";

/**
 * 状态分组的三种类型，与 useModGroups 的 `STATE_GROUP_ID_*` 一一对应
 *
 * `on` / `off` / `fail` 分别是「已启用」「已禁用」「识别失败」三块。
 */
export type ModStateGroup = "on" | "off" | "fail";

/** 两种模组视图共用的 props */
export interface ModViewProps {
  /** 这一份要显示的模组；分组模式下是某个分组里的那一批 */
  items: ModItemDto[];
  /** 有操作在跑：按钮统一置灰 */
  busy: boolean;
  /**
   * 已选中的模组键集合（多选，供顶栏批量操作）
   *
   * 与分组同一套键（SHA1 = 内容哈希）：启用 / 禁用只改文件名，
   * uuid 会跟着变，SHA1 不变，所以刷新列表后选中状态还在。取键见 `modSelectKey`。
   */
  selectedKeys: Set<string>;
  /**
   * Shift 整段选择的**锚点**：上次点或勾选的那一项的键，见 `modSelectKey`
   *
   * 视图在那一行上加个标记（边框换成实心强调色），让人看得出"整段从哪儿开始"。
   * 可空：还没点过任何一行时不标记。
   */
  anchorKey?: string | null;
  /**
   * 有更新的那些行的多选键，见 `modSelectKey`
   *
   * 命中的行上多一枚「有更新」标签；键与多选同一套，启用 / 禁用改文件名之后仍然对得上。
   * 不传 = 没有可更新的。
   */
  updateKeys?: Set<string>;
}

/** 两种模组视图共用的行操作事件 */
export interface ModViewEmits {
  (e: "toggle", item: ModItemDto): void;
  (e: "remove", item: ModItemDto): void;
  (e: "open-folder", item: ModItemDto): void;
  /** 编辑备注：弹窗在 ModPane，三种视图只管把要编辑的那一条抛上来 */
  (e: "note", item: ModItemDto): void;
  /**
   * 表格里改完在线信息：下载源 / 项目编号 / 文件编号
   *
   * 三格一起抛上来（后端一次改三样），由 ModPane 落盘并就地更新那一行。
   */
  (
    e: "online-save",
    payload: { item: ModItemDto; source: string; projectId: string; fileId: string },
  ): void;
  /**
   * 折叠 / 展开所在分组（表格视图的第一列有折叠箭头）
   *
   * 列表视图的折叠在分组头上，所以只有表格会发这个事件。
   */
  (e: "toggle-group"): void;
  /**
   * 分组名那一格按下：交给 ModPane 的**分组排序**拖拽
   *
   * 列表视图的分组头本来就能按住拖动排序，表格视图只有"点一下折叠"——
   * 于是表格里分组顺序改不了，用户报过"模组分组无法移动顺序"。
   * 这里把按下抛上去，与列表视图走同一个 `onSectionPointerDown`。
   */
  (e: "drag-group", event: PointerEvent): void;
  /** 勾选框：把这一行加进 / 移出多选；选择逻辑见 composables/useRowSelect.ts */
  (e: "select", item: ModItemDto): void;
  /**
   * 右键模组行：交给 ModPane 弹**行菜单**（RowMenu）
   *
   * 没有选中项时是单行菜单：启用/禁用、备注、打开文件夹、移出分组、删除；
   * 有选中项时换成批量菜单，动作作用在选中的那一批上。
   * 多选的入口是行左侧的勾选框（悬停即出现），不再是右键。
   */
  (e: "item-menu", payload: { event: MouseEvent; item: ModItemDto }): void;
  /**
   * 左键点整行：**已经在多选模式时**用来加选 / 反选；按住 Shift 则是**整段选中**
   * 从上次点的那一项到这一项，按显示顺序，与资源管理器同一口径
   *
   * 不在多选模式、也没按 Shift 时什么都不做，只记下整段选择的锚点。
   */
  (e: "row-click", payload: { event: MouseEvent; item: ModItemDto }): void;
}

/**
 * 拼模组行的副标题
 *
 * - `item`: 模组条目
 *
 * # 返回值
 *
 * 版本 · 作者 · 文件名，缺的项自动跳过
 */
export function modSub(item: ModItemDto): string {
  return [item.version, item.author, item.file].filter(Boolean).join(" · ");
}

/**
 * 模组行的渲染标识，给 v-for 当 key
 *
 * 内置模组 jar-in-jar 没有 sha1 也没有 uuid，退回 modid / 名字。
 * **分组用的是 `sha1`**，与 gui_setting.json 的 Groups 同一口径，这个只是行身份，两件事别混。
 *
 * - `item`: 模组条目
 *
 * # 返回值
 *
 * sha1、uuid、modid、名字里第一个非空的
 */
export function modRowKey(item: ModItemDto): string {
  return item.sha1 || item.uuid || item.modId || item.name;
}

/**
 * 多选的键：**优先 SHA1，没有就退回 uuid**
 *
 * SHA1 是内容哈希，启用 / 禁用只改文件名，之后不变，所以正常情况下选中状态能扛住刷新；
 * 但**读取失败的 jar、手动放进 mods 的坏文件没有 SHA1**，内核哈希失败时 `sha1_of` 返回空串，
 * 一律用 uuid 兜底，否则这些行连勾选框都没有、整段选择也会把它们跳过，用户报过。
 *
 * 注意：分组的键**只能是 SHA1**，内核 `Mod.Groups` 按内容哈希记，
 * 所以往分组里放之前要换成 SHA1 并丢掉换不出来的，见 ModPane 的 `batchMove`。
 *
 * - `item`: 模组条目
 *
 * # 返回值
 *
 * 该行的多选键
 */
export function modSelectKey(item: ModItemDto): string {
  return item.sha1 || item.uuid;
}

/**
 * 平台返回的 ISO 时间只取日期那一段
 *
 * 接口给的形如 `2024-03-01T12:00:00Z`，前 10 位就是日期，不做日期解析与时区换算。
 *
 * - `iso`: ISO 8601 字符串
 *
 * # 返回值
 *
 * `2024-03-01`；不是这个形态时原样返回
 */
export function isoDate(iso: string): string {
  return /^\d{4}-\d{2}-\d{2}/.test(iso) ? iso.slice(0, 10) : iso;
}

/**
 * 一个模组条目的搜索命中：**按展示方式选字段**
 *
 * - `item`: 模组条目
 * - `q`: 已转小写的搜索词
 * - `view`: 列表模式只认名字与文件名，一行里只显示了这两样；
 *   表格模式还有 modid / 作者两列
 *
 * # 返回值
 *
 * 任一字段包含搜索词即为 `true`
 */
export function modHit(item: ModItemDto, q: string, view: ModView): boolean {
  const fields =
    view === "table" ? [item.name, item.modId, item.file, item.author] : [item.name, item.file];
  return fields.some((value) => value.toLowerCase().includes(q));
}

/**
 * 模组的显示排序：**按 modid 正序**
 *
 * 列表与表格两个视图共用这一份 —— 各自写一遍的话迟早会漂移成两种顺序，
 * 用户在两个视图之间切换就会看到"同一个分组、变了顺序"。
 *
 * 规则：
 * 1. modid 字典序，`localeCompare` 固定 `en`，避免跟随系统区域导致顺序不稳；
 * 2. **没有 modid 的沉到末尾**，用 `\uffff` 当哨兵，它比任何常规字符都大。
 *    不能拿空串比 —— 空串会排到最前面，让"识别失败的包"挤在正常模组之前；
 * 3. modid 相同时按显示名（缺失时用文件名）兜底，保证顺序是全序、不抖动。
 *
 * - `a`: 参与比较的第一个模组
 * - `b`: 参与比较的第二个模组
 *
 * # 返回值
 *
 * 负数 / 0 / 正数，可直接交给 `Array.sort`
 */
export function compareMod(a: ModItemDto, b: ModItemDto): number {
  const ka = a.modId || "\uffff";
  const kb = b.modId || "\uffff";
  const byId = ka.localeCompare(kb, "en");
  if (byId !== 0) return byId;
  return (a.name || a.file).localeCompare(b.name || b.file, "en");
}

/**
 * 排好序的模组副本，**不改原数组**
 *
 * 调用方拿到的 `items` 是分组计算出来的切片，直接 `sort` 会就地把上游数据改掉。
 *
 * - `items`: 待排序的模组
 *
 * # 返回值
 *
 * 按 `compareMod` 排好的新数组
 */
export function sortedMods(items: ModItemDto[]): ModItemDto[] {
  return [...items].sort(compareMod);
}
