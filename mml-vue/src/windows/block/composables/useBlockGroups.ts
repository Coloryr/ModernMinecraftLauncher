// 方块窗口的用户数据：左栏顺序、自建分组、收藏
//
// **存在 Rust 侧的本地文件 `block_groups.json`**（命令 block_get_groups / block_set_groups），
// 不走前端 localStorage —— 按仓库约定，前端只放临时数据，用户数据一律落本地文件。
//
// 口径（用户定的）：
// - 收藏**不单独成分类**：收藏过的方块在列表里排到最前；
// - 一个方块**只能属于一个**自建分组；
// - 左栏把游戏分类与自建分组混在一起，按用户拖出来的顺序排。
import { onMounted, ref } from "vue";
import { getBlockGroups, setBlockGroups } from "../../../lib/api";

/** 自建分组 */
export interface BlockGroup {
  /** 稳定标识，与名字无关 */
  uuid: string;
  /** 分组名，用户输入 */
  name: string;
}

/** 浏览器预览里也要能生成 uuid；没有 randomUUID 时手工拼一个 v4 */
function newId(): string {
  if (typeof crypto !== "undefined" && typeof crypto.randomUUID === "function") {
    return crypto.randomUUID();
  }
  return "xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx".replace(/[xy]/g, (ch) => {
    const r = (Math.random() * 16) | 0;
    return (ch === "x" ? r : (r & 0x3) | 0x8).toString(16);
  });
}

/** uuid 形态检查；旧版本的分组 id（`bg-…`）不是 uuid，读入时换掉 */
const UUID_RE = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

/**
 * 方块窗口的用户数据
 *
 * # 返回值
 *
 * - `orderedKeys` / `setOrder`: 左栏顺序，键是分类 uuid
 * - `groups` / `addGroup` / `renameGroup` / `removeGroup`: 自建分组
 * - `inGroup` / `groupOf` / `setGroup` / `countOf`: 方块的归属，一个方块只属于一个组
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
      // 旧版本的分组 id 是 bg-…、顺序键带 g: 前缀，读入时统一成 uuid
      const remap = new Map<string, string>();
      const groupsNext: BlockGroup[] = data.groups.map((group) => {
        const uuid = UUID_RE.test(group.uuid) ? group.uuid : newId();
        if (uuid !== group.uuid) remap.set(group.uuid, uuid);
        return { uuid, name: group.name };
      });
      // 旧数据允许多分组归属：按分组顺序只保留第一个，符合"一个方块一个分组"
      const seen = new Set<string>();
      const membersNext: Record<string, string[]> = {};
      for (let i = 0; i < groupsNext.length; i++) {
        const oldId = data.groups[i].uuid;
        membersNext[groupsNext[i].uuid] = (data.members[oldId] ?? []).filter((id) => {
          if (seen.has(id)) return false;
          seen.add(id);
          return true;
        });
      }
      const orderNext = data.order
        .map((key) => {
          const raw = key.startsWith("g:") ? key.slice(2) : key;
          return remap.get(raw) ?? raw;
        })
        // 旧顺序里自带分类那一截是当时的显示名，没法可靠地换回 uuid，直接丢弃
        .filter((key) => UUID_RE.test(key));
      order.value = orderNext;
      groups.value = groupsNext;
      members.value = membersNext;
      favorites.value = data.favorites;
      // 迁移过就写回，下次不用再转一遍
      if (
        JSON.stringify(orderNext) !== JSON.stringify(data.order) ||
        JSON.stringify(groupsNext) !== JSON.stringify(data.groups) ||
        JSON.stringify(membersNext) !== JSON.stringify(data.members)
      ) {
        save();
      }
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
    order.value = [...order.value, uuid];
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
    order.value = order.value.filter((key) => key !== uuid);
    save();
  }

  /** 某个键是不是自建分组 */
  function isCustomGroup(uuid: string): boolean {
    return groups.value.some((group) => group.uuid === uuid);
  }

  /** 某个方块在不在这个分组里 */
  function inGroup(uuid: string, id: string): boolean {
    return (members.value[uuid] ?? []).includes(id);
  }

  /** 方块所属的分组 uuid；不在任何分组时返回 null */
  function groupOf(id: string): string | null {
    return groups.value.find((g) => inGroup(g.uuid, id))?.uuid ?? null;
  }

  /**
   * 设置方块所属的分组，一个方块只能属于一个
   *
   * - `id`: 方块 id
   * - `uuid`: 目标分组；null 表示移出所有分组
   */
  function setGroup(id: string, uuid: string | null) {
    const next: Record<string, string[]> = {};
    for (const group of groups.value) {
      const list = (members.value[group.uuid] ?? []).filter((each) => each !== id);
      if (group.uuid === uuid) list.push(id);
      next[group.uuid] = list;
    }
    members.value = next;
    save();
  }

  /** 分组里的方块数 */
  function countOf(uuid: string): number {
    return (members.value[uuid] ?? []).length;
  }

  // ---------- 收藏 ----------

  /** 某个方块是否已收藏 */
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
    isCustomGroup,
    inGroup,
    groupOf,
    setGroup,
    countOf,
    favorites,
    isFavorite,
    toggleFavorite,
  };
}
