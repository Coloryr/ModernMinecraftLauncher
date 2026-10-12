// 模组的自定义分组：分组表的加载、增删改，"把模组归到某个组"，以及**分组块的显示顺序**
//
// 分组数据属于实例的 GUI 设置：`gui_setting.json` 的 `Mod.Groups`，见后端 crate::gui_setting。
// **分组用 uuid 作键**，名字只是对象里的一个字段（可以随便改，键不动）；成员按模组
// **SHA1** 记 —— 它是内容哈希，启用/禁用（改文件名）之后不变。
//
// 顺序（含三个**状态分组**）存在同一个文件的 `Mod.GroupOrder`：`Groups` 是 HashMap、
// 没有"建立顺序"，而用户要能拖动状态分组，所以顺序只能单独存一份。
// 顺序的规范化在后端 `resource.rs::group_order_of`（丢弃已删的分组、补上没记的），
// 前端只负责把拖出来的那一串原样交过去。
import { computed, ref } from "vue";
import { tErr } from "../../../lib/i18n";
import { showToast } from "../../../lib/toast";
import {
  addModGroup,
  getModGroupOrder,
  getModGroups,
  getModGroupsCollapsed,
  removeModGroup,
  renameModGroup,
  setModGroup,
  setModGroupOrder,
  setModGroupsCollapsed,
} from "../../../lib/api";
import type { ModGroupDto } from "../../../lib/bindings";
import type { useResourceData } from "./useResourceData";

/**
 * 三个**状态分组**的固定 uuid
 *
 * 与后端 `resource.rs` 的 `STATE_GROUP_FAIL` / `_ON` / `_OFF` 一一对应，改要一起改。
 * 它们不是用户数据，不进 `Mod.Groups`，但同样参与排序与折叠，所以需要稳定的键 ——
 * 现在与自建分组同一套形状（都是 uuid），前端不必再维护"两套键"的映射。
 *
 * 取值是"全 0 / 尾号 1 / 尾号 2"：调试时一眼认得出是状态分组，不用查表。
 */
export const STATE_GROUP_ID_FAIL = "00000000-0000-0000-0000-000000000000";
export const STATE_GROUP_ID_ON = "00000000-0000-0000-0000-000000000001";
export const STATE_GROUP_ID_OFF = "00000000-0000-0000-0000-000000000002";

/**
 * 模组的自定义分组
 *
 * - `data`: 数据层，只为拿 `instanceUuid`；分组存在实例的 `gui_setting.json` 里
 *
 * # 返回值
 *
 * - `groups`: 分组表，顺序由后端规范化后下发
 * - `order`: 分组块的排列顺序，含三个状态分组
 * - `collapsed`: 收起的分组块键集合
 * - `isCollapsed`: 某个块是否收起
 * - `toggleCollapsed`: 切换收起 / 展开并落盘
 * - `groupOfKey`: 模组 SHA1 → 所在分组 uuid
 * - `load`: 拉分组表、顺序与折叠状态
 * - `add`: 新建分组，成功返回 `true`
 * - `rename`: 重命名分组，成功返回 `true`
 * - `remove`: 删除分组，组内模组回到未分组
 * - `setGroup`: 把若干模组移到某个分组，`group` 传 `null` 表示移出
 * - `setOrder`: 保存拖出来的块顺序
 */
export function useModGroups(data: ReturnType<typeof useResourceData>) {
  const { instanceUuid } = data;

  /**
   * 分组表，顺序 = 后端下发的顺序 = 用户拖出来的顺序，没拖过则是默认顺序
   *
   * `gui_setting.json` 的 `Groups` 是 HashMap，没有"建立顺序"可言，所以顺序由后端
   * `resource.rs::group_order_of` 统一决定再下发；前端**不要**再排一次，
   * 两处排序规则一旦不一致，就会出现"看着没变但顺序变了"。
   */
  const groups = ref<ModGroupDto[]>([]);

  /**
   * 分组块的顺序，**含状态分组**，键都是分组 uuid
   *
   * 与 `groups` 是两份数据：这一份管排列，`groups` 管分组内容。
   *
   * **整份都从后端读**，命令是 `resource_mod_group_order`，不在这里拼：状态分组不是用户数据、
   * 不在 `resource_mod_groups` 里，自己拼的话永远拼不出"用户把「已启用」拖到了某个
   * 自建分组前面"—— 表现就是拖完松手又弹回原位，用户报的"模组分组无法移动顺序"。
   */
  const order = ref<string[]>([]);

  /** 模组 SHA1 → 所在分组 uuid；未归组没有条目 */
  const groupOfKey = computed(() => {
    const map = new Map<string, string>();
    for (const group of groups.value) {
      for (const sha1 of group.mods) {
        map.set(sha1, group.uuid);
      }
    }
    return map;
  });

  /**
   * **收起**的分组块键集合，键是分组 uuid，与 `order` 同一套口径
   *
   * 存"收起的那些"：默认全展开，空集合即初始状态。落盘在实例的
   * `gui_setting.json` 的 `Mod.GroupCollapsed`，所以切换实例各记各的。
   */
  const collapsed = ref<Set<string>>(new Set());

  /** 拉分组，进入模组分类时调用 */
  async function load() {
    if (!instanceUuid.value) {
      groups.value = [];
      order.value = [];
      collapsed.value = new Set();
      return;
    }
    try {
      // 三份一起拉：同一份 gui_setting.json，三次 IPC 不如一次并发拿到
      const [list, folded, savedOrder] = await Promise.all([
        getModGroups(instanceUuid.value),
        getModGroupsCollapsed(instanceUuid.value),
        getModGroupOrder(instanceUuid.value),
      ]);
      groups.value = list;
      // 后端给的就是**完整且已规范化**的顺序（含状态分组），直接用
      order.value = savedOrder;
      collapsed.value = new Set(folded);
    } catch {
      groups.value = [];
      order.value = [];
      collapsed.value = new Set();
    }
  }

  /**
   * 保存块顺序，拖完调用
   *
   * 只把新顺序写回后端，**不重读**：落盘是异步排队的，立刻读会读到**旧顺序**，
   * 于是刚拖好的顺序又被拼回去，看着就是"松手弹回原位"。
   * 失败只提示，不回滚；下次进模组页 load 会拿到后端的真实顺序。
   */
  async function setOrder(next: string[]) {
    order.value = next;
    try {
      await setModGroupOrder(instanceUuid.value, next);
    } catch (e) {
      showToast(tErr(e));
    }
  }

  /**
   * 切换某个块的收起 / 展开
   *
   * 先改内存再落盘：折叠要立刻有反馈，等 IPC 回来才动会让点击看着"没反应"。
   * 失败只提示，不回滚；下次 load 会拿到后端的真实状态。
   */
  function toggleCollapsed(key: string) {
    const next = new Set(collapsed.value);
    if (next.has(key)) {
      next.delete(key);
    } else {
      next.add(key);
    }
    collapsed.value = next;
    if (!instanceUuid.value) return;
    void setModGroupsCollapsed(instanceUuid.value, [...next]).catch((e) => showToast(tErr(e)));
  }

  /** 这个块是不是收起的 */
  function isCollapsed(key: string): boolean {
    return collapsed.value.has(key);
  }

  /**
   * 新建分组
   *
   * **写完之后不重读**：落盘是**异步排队**的，跑在 `config_save` 后台线程，
   * 立刻 `load()` 读到的还是旧文件 —— 新分组不出现，用户看到的就是
   * "新建完了却选不到分组"，踩过。后端会把新 uuid 回给我们，
   * 本地按同样的位置（末尾）补一条即可。
   *
   * - `name`: 分组名，前后空白会去掉；去掉后为空则不建
   *
   * # 返回值
   *
   * 建成功为 `true`
   */
  async function add(name: string): Promise<boolean> {
    const trimmed = name.trim();
    if (!trimmed) return false;
    try {
      const uuid = await addModGroup(instanceUuid.value, trimmed);
      // 后端 `group_order_of` 会把没记过的分组补在末尾，本地照做
      groups.value = [...groups.value, { uuid, name: trimmed, mods: [] }];
      order.value = [...order.value, uuid];
      return true;
    } catch (e) {
      showToast(tErr(e));
      return false;
    }
  }

  /**
   * 重命名分组，**传 uuid**，只改名字，键与成员都不动
   *
   * 同样不重读，本地改名。
   *
   * - `group`: 分组 uuid
   * - `newName`: 新名字，前后空白会去掉；去掉后为空则不重命名
   *
   * # 返回值
   *
   * 改成功为 `true`
   */
  async function rename(group: string, newName: string): Promise<boolean> {
    const trimmed = newName.trim();
    if (!trimmed) return false;
    try {
      await renameModGroup(instanceUuid.value, group, trimmed);
      groups.value = groups.value.map((g) => (g.uuid === group ? { ...g, name: trimmed } : g));
      return true;
    } catch (e) {
      showToast(tErr(e));
      return false;
    }
  }

  /**
   * 删除分组，组内模组回到"未分组"，磁盘文件不动
   *
   * 同样不重读：本地摘掉这条即可，分组数据不在模组列表里，也不用重拉列表。
   */
  async function remove(group: string) {
    try {
      await removeModGroup(instanceUuid.value, group);
      groups.value = groups.value.filter((g) => g.uuid !== group);
      order.value = order.value.filter((key) => key !== group);
      const next = new Set(collapsed.value);
      next.delete(group);
      collapsed.value = next;
    } catch (e) {
      showToast(tErr(e));
    }
  }

  /**
   * 把若干模组移到某个分组
   *
   * **不重读**：落盘是异步排队的，立刻 `load()` 读到的还是旧成员表 ——
   * 移进分组的模组会看着"没进去"。本地按同一套规则改成员即可：
   * 先从所有分组里摘掉，再进目标组（一个模组同时只属于一个组）。
   *
   * - `group`: 目标分组 uuid；`null` 表示移出所有分组，回到"未分组"
   * - `keys`: 要移动的模组键，即 SHA1
   */
  async function setGroup(group: string | null, keys: string[]) {
    if (!keys.length) return;
    // 已经在目标组里的不用动（移回原组是一次空操作）
    const target = group ?? "";
    const moved = keys.filter((key) => (groupOfKey.value.get(key) ?? "") !== target);
    if (!moved.length) return;
    try {
      await setModGroup(instanceUuid.value, group, moved);
      const moving = new Set(moved);
      groups.value = groups.value.map((g) => {
        const kept = g.mods.filter((sha1) => !moving.has(sha1));
        return g.uuid === group ? { ...g, mods: [...kept, ...moved] } : { ...g, mods: kept };
      });
    } catch (e) {
      showToast(tErr(e));
    }
  }

  return {
    groups,
    order,
    collapsed,
    isCollapsed,
    toggleCollapsed,
    groupOfKey,
    load,
    add,
    rename,
    remove,
    setGroup,
    setOrder,
  };
}
