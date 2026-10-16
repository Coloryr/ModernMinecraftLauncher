// 方块窗口的用户数据：左栏顺序、自建分组、收藏
//
// **存在 Rust 侧的本地文件 `block_groups.json`**（命令 block_get_groups / block_set_groups），
// 不走前端 localStorage —— 按仓库约定，前端只放临时数据，用户数据一律落本地文件。
//
// 口径（用户定的）：
// - 收藏**不单独成分类**：收藏过的方块在列表里排到最前；
// - 一个方块可以同时属于**多个**自建分组；
// - 左栏把游戏分类与自建分组混在一起，按用户拖出来的顺序排。
import { onMounted, ref } from "vue";
import { getBlockGroups, setBlockGroups } from "../../../lib/api";
import { GROUP_PREFIX } from "../types";

/** 自建分组 */
export interface BlockGroup {
  /** 稳定标识，与名字无关 */
  uuid: string;
  /** 分组名，用户输入 */
  name: string;
}

/**
 * 自建分组的分类键
 *
 * - `uuid`: 分组标识
 *
 * # 返回值
 *
 * 形如 `g:<uuid>`
 */
export function groupKey(uuid: string): string {
  return `${GROUP_PREFIX}${uuid}`;
}

/**
 * 分类键是不是自建分组
 *
 * - `key`: 左栏的分类键
 *
 * # 返回值
 *
 * 自建分组为 `true`
 */
export function isGroupKey(key: string): boolean {
  return key.startsWith(GROUP_PREFIX);
}

/** 新的分组 id；不用 crypto.randomUUID，浏览器预览里也稳 */
function newId(): string {
  return `bg-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`;
}

/**
 * 方块窗口的用户数据
 *
 * # 返回值
 *
 * - `orderedKeys` / `setOrder`: 左栏顺序
 * - `groups` / `addGroup` / `renameGroup` / `removeGroup`: 自建分组
 * - `inGroup` / `groupsOf` / `setMember` / `countOf`: 方块的归属，可属于多个组
 * - `favorites` / `isFavorite` / `toggleFavorite`: 收藏
 */
export function useBlockGroups() {
  const order = ref<string[]>([]);
  const groups = ref<BlockGroup[]>([]);
  const members = ref<Record<string, string[]>>({});
  const favorites = ref<string[]>([]);

  /** 整份落盘：Rust 侧异步写 block_groups.json，失败不打断界面 */
  function save() {
    void setBlockGroups({
      order: order.value,
      groups: groups.value.map((g) => ({ uuid: g.uuid, name: g.name })),
      members: members.value,
      favorites: favorites.value,
    }).catch(() => {});
  }

  // 打开窗口时读一次；读不到就按空的来，之后的改动照样能写回去
  onMounted(async () => {
    try {
      const data = await getBlockGroups();
      order.value = data.order;
      groups.value = data.groups;
      members.value = data.members;
      favorites.value = data.favorites;
    } catch {
      /* 读不到就空着 */
    }
  });

  // ---------- 顺序 ----------

  /**
   * 把实际存在的分类键按用户顺序排好
   *
   * - `existing`: 当前存在的分类键（游戏分类 + 自建分组）
   *
   * # 返回值
   *
   * 排好的键；顺序表里没有的（新出现的游戏分类 / 新建的分组）追加在末尾
   */
  function orderedKeys(existing: string[]): string[] {
    const have = new Set(existing);
    const out = order.value.filter((key) => have.has(key));
    for (const key of existing) {
      if (!out.includes(key)) out.push(key);
    }
    return out;
  }

  /** 写回一整份顺序 */
  function setOrder(next: string[]) {
    order.value = next;
    save();
  }

  // ---------- 分组 ----------

  /**
   * 新建分组
   *
   * - `name`: 分组名
   *
   * # 返回值
   *
   * 新分组的 uuid
   */
  function addGroup(name: string): string {
    const uuid = newId();
    groups.value = [...groups.value, { uuid, name }];
    members.value = { ...members.value, [uuid]: [] };
    order.value = [...order.value, groupKey(uuid)];
    save();
    return uuid;
  }

  /** 改名 */
  function renameGroup(uuid: string, name: string) {
    groups.value = groups.value.map((g) => (g.uuid === uuid ? { uuid, name } : g));
    save();
  }

  /** 删除分组：分组与它的成员表一起去掉，方块本身不动 */
  function removeGroup(uuid: string) {
    groups.value = groups.value.filter((g) => g.uuid !== uuid);
    const next = { ...members.value };
    delete next[uuid];
    members.value = next;
    order.value = order.value.filter((key) => key !== groupKey(uuid));
    save();
  }

  /** 某个方块在不在这个分组里 */
  function inGroup(uuid: string, id: string): boolean {
    return (members.value[uuid] ?? []).includes(id);
  }

  /** 某个方块属于哪些分组 */
  function groupsOf(id: string): string[] {
    return groups.value.filter((g) => inGroup(g.uuid, id)).map((g) => g.uuid);
  }

  /** 把方块加进 / 移出某个分组；别的分组不受影响 */
  function setMember(uuid: string, id: string, on: boolean) {
    const list = new Set(members.value[uuid] ?? []);
    if (on) {
      list.add(id);
    } else {
      list.delete(id);
    }
    members.value = { ...members.value, [uuid]: [...list] };
    save();
  }

  /** 分组里的方块数 */
  function countOf(uuid: string): number {
    return (members.value[uuid] ?? []).length;
  }

  // ---------- 收藏 ----------

  function isFavorite(id: string): boolean {
    return favorites.value.includes(id);
  }

  /** 收藏 / 取消收藏 */
  function toggleFavorite(id: string) {
    const set = new Set(favorites.value);
    if (set.has(id)) {
      set.delete(id);
    } else {
      set.add(id);
    }
    favorites.value = [...set];
    save();
  }

  return {
    orderedKeys,
    setOrder,
    groups,
    addGroup,
    renameGroup,
    removeGroup,
    inGroup,
    groupsOf,
    setMember,
    countOf,
    favorites,
    isFavorite,
    toggleFavorite,
  };
}
