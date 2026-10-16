// 模组依赖关系：**关系在后端算**（窗口模型那份扫描结果上，见 AGENTS.md §12.1），
// 这里只是两个命令的包装 + 把后端回的 uuid 对回列表项。
//
// - 依赖图的节点 / 连线：`resource_mods_dep_tree`，前端只做 dagre 布局（modDepGraph.ts）；
// - "要和哪几个模组一起改"：`resource_mods_related_mods`，禁用方向找"谁还需要它"，
//   启用方向找"它要的强制前置里现在被禁用着的"。
import type { Ref } from "vue";
import { modsDepTree, modsRelatedMods } from "../../../lib/api";
import type { ModItemDto } from "../../../lib/bindings";
import { layoutDepGraph, type ModDepGraph } from "./modDepGraph";

/**
 * 模组依赖关系
 *
 * - `mods`: 顶层模组列表，用来把后端回的 uuid 对回条目
 * - `instanceUuid`: 本窗口锁定的实例 uuid
 *
 * # 返回值
 *
 * - `depTreeOf`: 某个条目的依赖图，后端建图 + 本地 dagre 布局
 * - `relatedOf`: 要和这几个条目一起改的那些，关系来自后端
 * - `itemOf`: 按 modid 取那个模组条目，给依赖图双击换目标用
 */
export function useModDeps(mods: Ref<ModItemDto[]>, instanceUuid: Ref<string>) {
  /**
   * 在列表里按 modid 找一个条目，内置的（jar-in-jar）也算
   *
   * - `key`: modid 小写
   *
   * # 返回值
   *
   * 找到的条目；缺失的前置与虚拟前置没有条目，返回 `null`
   */
  function itemOf(key: string): ModItemDto | null {
    const find = (list: ModItemDto[]): ModItemDto | null => {
      for (const item of list) {
        if (item.modId.toLowerCase() === key) return item;
        const nested = find(item.jarInJar);
        if (nested) return nested;
      }
      return null;
    };
    return find(mods.value);
  }

  /**
   * 某个条目的依赖图
   *
   * - `item`: 图根
   *
   * # 返回值
   *
   * 见 [`ModDepGraph`]，节点与连线由后端给，坐标是本地算的
   */
  async function depTreeOf(item: ModItemDto): Promise<ModDepGraph> {
    const tree = await modsDepTree(instanceUuid.value, item.uuid);
    return layoutDepGraph(tree);
  }

  /**
   * 要和这几个条目一起改的那些
   *
   * - `picked`: 这次要改的条目
   * - `disable`: true = 这次是禁用（找"谁还需要它"），false = 启用（找"它要的禁用前置"）
   *
   * # 返回值
   *
   * 要一起改的条目，按列表顺序；后端回的是 uuid，这里对回列表项
   */
  async function relatedOf(picked: ModItemDto[], disable: boolean): Promise<ModItemDto[]> {
    if (!picked.length || !instanceUuid.value) return [];
    const uuids = await modsRelatedMods(
      instanceUuid.value,
      picked.map((item) => item.uuid),
      disable,
    );
    return uuids
      .map((uuid) => mods.value.find((item) => item.uuid === uuid))
      .filter((item): item is ModItemDto => !!item);
  }

  return { depTreeOf, relatedOf, itemOf };
}
