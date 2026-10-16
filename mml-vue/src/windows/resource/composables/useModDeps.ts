// 模组依赖关系：把列表里每个包声明的 dependencies 建成索引，
// 供依赖图展示与"禁用前置时顺带禁用依赖方"用。
//
// 数据来自 `resource_list_mods` 的 DTO —— 每个条目自带 `dependencies`
// （只含与实例加载器一致的元数据声明，与依赖检测同一口径），列表读过一次就够了。
//
// **虚拟前置**（minecraft / forge 这类）与内核 `provided_version` 一个口径：
// 它们不由模组包提供，图里标成"游戏 / 加载器"，不能算缺失。
//
// 建图与布局在 modDepGraph.ts（纯函数、不碰响应式），这里只负责备料：
// 两个索引（谁提供某个 modid、某个 modid 被谁声明）与虚拟前置的版本。
import { computed, type Ref } from "vue";
import type { ModItemDto } from "../../../lib/bindings";
import {
  buildDepGraph,
  type ModDepGraph,
  type ModDependent,
  type ModProvider,
} from "./modDepGraph";
import type { ResourceInstance } from "./useResourceData";

/**
 * 实例的加载器 → 模组元数据里可能写成哪些 modid
 *
 * 一个加载器常有好几种写法（Forge 有 forge / minecraftforge，Quilt 会认 fabricloader），
 * 虚拟前置的版本与"加载器 → minecraft"那条合成边都按这张表认。
 */
const LOADER_IDS: Record<string, string[]> = {
  forge: ["forge", "minecraftforge"],
  neoforge: ["neoforge", "forge"],
  fabric: ["fabricloader"],
  quilt: ["quilt_loader", "quilt_base", "fabricloader"],
  optifine: ["optifine"],
  liteloader: ["liteloader"],
};

/**
 * 模组依赖关系

 * - `mods`: 顶层模组列表，内置的在自己的 `jarInJar` 里
 * - `instance`: 当前实例，用来认虚拟前置（游戏版本 / 加载器 / 加载器版本）
 *
 * # 返回值
 *
 * - `graphOf`: 某个条目的依赖图（节点按 modid 去重，含布局）
 * - `dependentsOf`: 哪些**启用**的模组依赖给定的 modid 集合，可选前置不算
 * - `itemOf`: 按 modid 取那个模组条目，给依赖图双击换目标用
 */
export function useModDeps(mods: Ref<ModItemDto[]>, instance: Ref<ResourceInstance | null>) {
  /** modid（小写）→ 提供它的条目；顶层在前、内置在后 */
  const providersOf = computed(() => {
    const map = new Map<string, ModProvider[]>();
    const add = (item: ModItemDto, nested: boolean) => {
      const id = item.modId.toLowerCase();
      if (!id) return;
      const list = map.get(id);
      if (list) {
        list.push({ item, nested });
      } else {
        map.set(id, [{ item, nested }]);
      }
    };
    const addNested = (item: ModItemDto) => {
      for (const child of item.jarInJar) {
        add(child, true);
        addNested(child);
      }
    };
    for (const item of mods.value) add(item, false);
    for (const item of mods.value) addNested(item);
    return map;
  });

  /**
   * modid（小写）→ 声明它是前置的那些模组
   *
   * 依赖图反过来画这一层用，**只到直接声明它的那一级**。含禁用状态与可选声明：
   * 图上如实画出来，是否可选由那条连线表示，与"禁用依赖方"那个判断是两回事。
   */
  const dependentsIndexOf = computed(() => {
    const map = new Map<string, ModDependent[]>();
    for (const item of mods.value) {
      if (item.fail || !item.modId) continue;
      for (const dep of item.dependencies) {
        const id = dep.modId.toLowerCase();
        if (!id) continue;
        const list = map.get(id);
        if (list) {
          list.push({ item, dep });
        } else {
          map.set(id, [{ item, dep }]);
        }
      }
    }
    return map;
  });

  /**
   * 这个 modid 是不是游戏 / 加载器 / 运行环境提供的
   *
   * 与内核 `provided_version` 同一份名单：命中返回它的版本（版本未知时是空串），
   * 不是虚拟前置返回 `null`。
   */
  function virtualVersion(id: string): string | null {
    const inst = instance.value;
    if (!inst) return null;
    // java / mcp / fml：运行环境或老加载器提供，版本口径不在实例设置里
    if (id === "java" || id === "mcp" || id === "fml") return "";
    if (id === "minecraft") return inst.version;
    return (LOADER_IDS[inst.loader] ?? []).includes(id) ? (inst.loaderVersion ?? "") : null;
  }

  /** 这台实例的加载器在图上可能用的节点键，见 LOADER_IDS */
  const loaderIds = computed(() => new Set(LOADER_IDS[instance.value?.loader ?? ""] ?? []));

  /**
   * 某个条目的依赖图
   *
   * 同一个前置被多条路径共用时只出现一个节点，环也画得出来 —— 展开规则见
   * modDepGraph.ts 的 buildDepGraph。
   *
   * - `item`: 图根
   *
   * # 返回值
   *
   * 见 [`ModDepGraph`]，节点与连线都带好了布局坐标
   */
  function graphOf(item: ModItemDto): ModDepGraph {
    return buildDepGraph(
      item,
      providersOf.value,
      virtualVersion,
      dependentsIndexOf.value,
      loaderIds.value,
    );
  }

  /**
   * 按 modid 取那个模组条目
   *
   * 依赖图里双击某个节点就是换目标：图的根必须是一个**模组条目**，
   * 而图上只有 modid，所以回到这里查一次。
   *
   * - `key`: modid 小写
   *
   * # 返回值
   *
   * 装了的那份优先；缺失的前置与虚拟前置没有条目，返回 `null`
   */
  function itemOf(key: string): ModItemDto | null {
    const list = providersOf.value.get(key) ?? [];
    return (list.find((each) => !each.item.disable) ?? list[0])?.item ?? null;
  }

  /**
   * 哪些启用的模组依赖给定的 modid 集合
   *
   * - `modIds`: 小写 modid 集合
   *
   * # 返回值
   *
   * 依赖它们的条目，按列表顺序
   */
  function dependentsOf(modIds: Set<string>): ModItemDto[] {
    if (!modIds.size) return [];
    return mods.value.filter(
      (item) =>
        !item.disable &&
        !item.fail &&
        !modIds.has(item.modId.toLowerCase()) &&
        item.dependencies.some((dep) => !dep.optional && modIds.has(dep.modId.toLowerCase())),
    );
  }

  return { graphOf, dependentsOf, itemOf };
}
