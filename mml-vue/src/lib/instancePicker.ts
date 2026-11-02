// 实例下拉的纯数据部分：选项结构、分组与搜索匹配
//
// 各窗口拿到的实例对象并不一样：主窗口 / 导出 / 联机是 InstanceInfoDto（带 group），
// 统计窗口是 StatsInstanceDto（没有分组信息）。这里用一个结构类型同时容纳两者，
// 缺的字段按"没有"处理，下拉组件因此不必为每个窗口写一套。

/** 实例下拉的一项 */
export interface InstanceOption {
  /** 实例 uuid，同时是下拉项的唯一键 */
  uuid: string;
  /** 显示名，调用方已按界面语言处理好（如统计窗口会给已删除的实例加标记） */
  name: string;
  /** 所属分组 uuid；缺省、或指向已不存在的分组时归到默认分组 */
  group?: string | null;
  /** 游戏版本；没有就不画副标题 */
  version?: string;
  /** 加载器标识，`normal` 表示原版 */
  loader?: string;
  /** 是否正在运行，右侧画一个绿点 */
  running?: boolean;
}

/** 下拉里的一个分组（分节） */
export interface InstanceGroupSection {
  /** 分组 uuid；默认分组与"拿不到分组表"时是空串 */
  id: string;
  /** 分组显示名，默认分组已按界面语言翻好 */
  name: string;
  /** 组内实例，保持传入顺序 */
  items: InstanceOption[];
}

/**
 * 这一项是否有副标题（版本 / 加载器）
 *
 * - `inst`: 实例项
 *
 * # 返回值
 *
 * 有版本或非原版加载器时为 true
 */
export function hasInstanceMeta(inst: InstanceOption): boolean {
  return !!inst.version || (!!inst.loader && inst.loader !== "normal");
}

/**
 * 按分组表把实例分节
 *
 * 分组表是唯一的身份来源：`group` 指向表里没有的分组、或是空值，一律归默认分组——
 * 宁可归错组，也不能让实例从下拉里消失。
 *
 * - `instances`: 全部实例，顺序即组内顺序
 * - `groups`: 分组表；为空时所有实例归到一个默认分节
 * - `defaultName`: 默认分组的显示名，调用方已 `t()` 好的成品
 *
 * # 返回值
 *
 * 只含有实例的分节，顺序与分组表一致
 */
export function groupInstances(
  instances: InstanceOption[],
  groups: { uuid: string; name: string }[],
  defaultName: string,
): InstanceGroupSection[] {
  const defaultId = groups.find((group) => !group.name.trim())?.uuid ?? "";
  const sections: InstanceGroupSection[] = groups.map((group) => ({
    id: group.uuid,
    name: group.name.trim() || defaultName,
    items: [],
  }));
  const byId = new Map(sections.map((section) => [section.id, section]));
  // 分组表里认不出的归属也放默认分组；拿不到分组表时就是这唯一的一节
  const orphan: InstanceGroupSection = { id: "", name: defaultName, items: [] };
  for (const inst of instances) {
    const id = inst.group && byId.has(inst.group) ? inst.group : defaultId;
    (byId.get(id) ?? orphan).items.push(inst);
  }
  return [...sections, orphan].filter((section) => section.items.length > 0);
}
