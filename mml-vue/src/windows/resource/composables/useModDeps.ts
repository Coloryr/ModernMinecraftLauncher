// 模组依赖关系：把列表里每个包声明的 dependencies 建成索引，
// 供依赖树展示与"禁用前置时顺带禁用依赖方"用。
//
// 数据来自 `resource_list_mods` 的 DTO —— 每个条目自带 `dependencies`
// （只含与实例加载器一致的元数据声明，与依赖检测同一口径），列表读过一次就够了。
//
// **虚拟前置**（minecraft / forge 这类）与内核 `provided_version` 一个口径：
// 它们不由模组包提供，树里标成"游戏 / 加载器"，不能算缺失。
import { computed, type Ref } from "vue";
import type { ModDependencyDto, ModItemDto } from "../../../lib/bindings";
import type { ResourceInstance } from "./useResourceData";

/** 提供某个 modid 的条目 */
export interface ModProvider {
  /** 提供它的模组条目 */
  item: ModItemDto;
  /** 由内置 jar（jar-in-jar）提供 */
  nested: boolean;
}

/** 依赖树的一层 */
export interface ModDepNode {
  /** 前置的 modid，原样 */
  modId: string;
  /** 声明里的版本要求，如 `[1.21,)`；不限版本是 `*` */
  required: string;
  /** 推荐安装（非强制） */
  optional: boolean;
  /** 提供它的模组条目；没有条目提供时为 `null`（虚拟前置与缺失都是 null） */
  provider: ModItemDto | null;
  /** 由内置 jar 提供 */
  nested: boolean;
  /** 由游戏本体 / 加载器 / 运行环境提供，以下简称"虚拟前置" */
  virtual: boolean;
  /** 装着的版本（含虚拟前置的版本），多份用 `, ` 连接；没装为空串 */
  installed: string;
  /** 缺失：强制前置，既没有模组提供也不是虚拟前置 */
  missing: boolean;
  /** 递归的子节点；路上已经出现过的 modid 不再往下展开，防环 */
  children: ModDepNode[];
}

/**
 * 模组依赖关系
 *
 * - `mods`: 顶层模组列表，内置的在自己的 `jarInJar` 里
 * - `instance`: 当前实例，用来认虚拟前置（游戏版本 / 加载器 / 加载器版本）
 *
 * # 返回值
 *
 * - `treeOf`: 某个条目的依赖树（不含自己）
 * - `dependentsOf`: 哪些**启用**的模组依赖给定的 modid 集合，可选前置不算
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
    const loader = inst.loader;
    const mine =
      (loader === "forge" && (id === "forge" || id === "minecraftforge")) ||
      (loader === "neoforge" && (id === "neoforge" || id === "forge")) ||
      (loader === "fabric" && id === "fabricloader") ||
      (loader === "quilt" &&
        (id === "quilt_loader" || id === "quilt_base" || id === "fabricloader")) ||
      (loader === "optifine" && id === "optifine") ||
      (loader === "liteloader" && id === "liteloader");
    return mine ? (inst.loaderVersion ?? "") : null;
  }

  /** 把一层声明展开成节点；`seen` 里出现过的 modid 不再往下走 */
  function childrenOf(deps: ModDependencyDto[], seen: Set<string>): ModDepNode[] {
    return deps.map((dep) => {
      const id = dep.modId.toLowerCase();
      const providers = providersOf.value.get(id) ?? [];
      // 启用中的优先：只有禁用包提供它时，依赖检测会单独报"前置被禁用"
      const chosen = providers.find((each) => !each.item.disable) ?? providers[0];
      const provider = chosen?.item ?? null;
      const virtual = providers.length ? null : virtualVersion(id);
      const node: ModDepNode = {
        modId: dep.modId,
        required: dep.required,
        optional: dep.optional,
        provider,
        nested: chosen?.nested ?? false,
        virtual: virtual !== null,
        installed:
          providers
            .map((each) => each.item.version)
            .filter((version) => !!version)
            .join(", ") || (virtual ?? ""),
        missing: !dep.optional && !providers.length && virtual === null,
        children: [],
      };
      if (provider && !seen.has(id)) {
        seen.add(id);
        node.children = childrenOf(provider.dependencies, seen);
      }
      return node;
    });
  }

  /**
   * 某个条目的依赖树
   *
   * - `item`: 树根
   *
   * # 返回值
   *
   * 一层节点数组；强制与推荐前置都在里面
   */
  function treeOf(item: ModItemDto): ModDepNode[] {
    // 根自己先记进"见过的"：A → B → A 这种环就不会无限展开
    const seen = new Set<string>([item.modId.toLowerCase()]);
    return childrenOf(item.dependencies, seen);
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

  return { treeOf, dependentsOf };
}
