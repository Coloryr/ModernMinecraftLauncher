// 模组依赖图：把"选中的模组需要哪些前置、那些前置又需要什么"整理成节点与边，
// 再用 dagre 算好分层布局（从左到右）。
//
// 为什么不是树：同一个前置常被多条路径共用（A 需要 C、B 也需要 C），
// 甚至绕一圈回到上游（环）。按树展开时同一个包会在多处重复出现，而且**见过的就不再展开**，
// 第二处看着"没有前置"，其实是没画出来。这里按 modid 去重成节点、同一对节点只留一条边。
//
// 分工：本文件只**建图**（去重、标状态、认环）并调 dagre 布局；
// 画与交互（拖节点 / 滚轮缩放 / 拖画布平移 / 点连线）交给 @vue-flow/core，
// 见 parts/mod/ModDepsModal.vue。

import { graphlib, layout } from "@dagrejs/dagre";
import type { ModDependencyDto, ModItemDto } from "../../../lib/bindings";

/** 提供某个 modid 的条目 */
export interface ModProvider {
  /** 提供它的模组条目 */
  item: ModItemDto;
  /** 由内置 jar（jar-in-jar）提供 */
  nested: boolean;
}

/** 反过来：某个 modid 被谁声明为前置 */
export interface ModDependent {
  /** 声明它的那个模组条目 */
  item: ModItemDto;
  /** 那份声明本体，取可选与版本要求 */
  dep: ModDependencyDto;
}

/** 依赖图里的一个节点，同一个 modid 只会有一个 */
export interface ModDepGraphNode {
  /** 唯一键：modid 小写；模组没有 modid 的极少数情况退回文件名 */
  key: string;
  /** 显示名：有条目用条目名，否则用 modid */
  label: string;
  /** 原始大小写的 modid */
  modId: string;
  /** 已装版本，多份提供者用 `, ` 连接；虚拟前置是它的版本；都没有时为空串 */
  version: string;
  /** 图标 data URL，没有时为空串 */
  icon: string;
  /** 是不是被选中的那个模组，即图的根 */
  root: boolean;
  /** 由内置 jar（jar-in-jar）提供 */
  nested: boolean;
  /** 由游戏本体 / 加载器 / 运行环境提供，以下简称"虚拟前置" */
  virtual: boolean;
  /** 装了：有模组提供，或由游戏 / 加载器提供 */
  installed: boolean;
  /** 缺失：有强制前置指向它，但既没有模组提供、也不是虚拟前置 */
  missing: boolean;
  /** 只有禁用状态的包提供它 */
  disabled: boolean;
  /** 必装：至少有一条**强制**边指向它；只有可选边指向它时为假 */
  mandatory: boolean;
  /** 指向它的边声明的版本要求，去重后按出现顺序；不限版本的不收 */
  required: string[];
  /** 布局坐标，单位 px，节点左上角 */
  x: number;
  y: number;
}

/** 依赖图里的一条边，同一对节点只会有一条 */
export interface ModDepGraphEdge {
  /** 唯一 id，vue-flow 靠它认边 */
  id: string;
  /** 起点节点键，即"谁需要" */
  from: string;
  /** 终点节点键，即"需要谁" */
  to: string;
  /** 推荐安装（非强制） */
  optional: boolean;
  /** 声明里的版本要求原文，不限版本是 `*` */
  required: string;
  /** 环上的回边：终点是这次遍历里的上游祖先 */
  backward: boolean;
  /**
   * 由**加载器指向 minecraft** 的那条
   *
   * 它不是模组声明的，是运行环境的常识（加载器本来就跑在某个 Minecraft 版本上）：
   * 两者都出现在图上时补一条，图上用黄色画，与虚拟前置的边框同色。
   */
  loader: boolean;
}

/** 算好布局的依赖图 */
export interface ModDepGraph {
  /** 节点 */
  nodes: ModDepGraphNode[];
  /** 连线 */
  edges: ModDepGraphEdge[];
  /** 画布宽，单位 px */
  width: number;
  /** 画布高，单位 px */
  height: number;
}

/** 节点宽，单位 px；CSS 与布局都取它，只有这一份 */
export const NODE_W = 184;
/** 节点高，单位 px */
export const NODE_H = 52;
/** 层间距，即左右相邻两列的空隙，单位 px */
const RANK_GAP = 72;
/** 同层相邻节点的间距，单位 px */
const NODE_GAP = 12;
/** 画布内边距，单位 px */
const PAD = 16;

/** 建图过程中的节点草稿：比最终节点多几个只用于判定的字段 */
interface NodeDraft {
  /** 唯一键 */
  key: string;
  /** 原始大小写的 modid */
  modId: string;
  /** 显示名 */
  label: string;
  /** 已装版本 */
  version: string;
  /** 图标 data URL */
  icon: string;
  /** 是不是根 */
  root: boolean;
  /** 由内置 jar 提供 */
  nested: boolean;
  /** 是虚拟前置 */
  virtual: boolean;
  /** 只有禁用包提供它 */
  disabled: boolean;
  /** 有模组提供它 */
  hasProvider: boolean;
  /** 指向它的边声明的版本要求 */
  required: string[];
  /** 至少有一条强制边指向它 */
  hasRequired: boolean;
}

/**
 * 从一个模组出发建依赖图，并用 dagre 算好分层布局
 *
 * - `root`: 图根，即被选中的那个模组
 * - `providers`: modid 小写 → 提供它的条目，见 useModDeps 的 providersOf
 * - `virtualOf`: 取虚拟前置的版本，不是虚拟前置返回 `null`
 * - `dependents`: modid 小写 → 声明它为前置的模组；**只画直接那一级**，不再往上牵
 * - `loaderIds`: 这台实例的加载器在元数据里可能写成哪些 modid，见 useModDeps 的 LOADER_IDS
 *
 * # 返回值
 *
 * 见 [`ModDepGraph`]
 */
export function buildDepGraph(
  root: ModItemDto,
  providers: Map<string, ModProvider[]>,
  virtualOf: (modId: string) => string | null,
  dependents: Map<string, ModDependent[]> = new Map(),
  loaderIds: Set<string> = new Set(),
): ModDepGraph {
  const drafts = new Map<string, NodeDraft>();
  const edges: ModDepGraphEdge[] = [];
  const edgeSeen = new Set<string>();
  /** 已经铺开过前置的节点：同一个包再被依赖到只连边，不重复往下走 */
  const expanded = new Set<string>();
  /** 当前遍历路径上的节点键，用来认环 */
  const path = new Set<string>();

  const rootKey = (root.modId || root.file).toLowerCase();

  /** 按 modid 取节点，没有就按它的提供者建一个 */
  function nodeOf(id: string): NodeDraft {
    const existed = drafts.get(id);
    if (existed) return existed;

    const list = providers.get(id) ?? [];
    const chosen = list.find((each) => !each.item.disable) ?? list[0] ?? null;
    // 有模组提供时就轮不到虚拟前置：装了的那份优先
    const virtual = list.length ? null : virtualOf(id);
    const versions = list
      .map((each) => each.item.version)
      .filter((version) => !!version)
      .join(", ");

    const draft: NodeDraft = {
      key: id,
      modId: chosen?.item.modId || id,
      label: chosen?.item.name || id,
      version: versions || virtual || "",
      icon: chosen?.item.icon ?? "",
      root: false,
      nested: chosen?.nested ?? false,
      virtual: virtual !== null,
      // 全是禁用包提供时才算"被禁用"；有启用的那份就正常
      disabled: list.length > 0 && list.every((each) => each.item.disable),
      hasProvider: list.length > 0,
      required: [],
      hasRequired: false,
    };
    drafts.set(id, draft);
    return draft;
  }

  /** 按"具体的那个条目"建节点：反向上游的那一层是直接拿模组条目，不经提供者索引 */
  function nodeOfItem(item: ModItemDto): NodeDraft {
    const id = item.modId.toLowerCase();
    const existed = drafts.get(id);
    if (existed) return existed;

    const draft: NodeDraft = {
      key: id,
      modId: item.modId,
      label: item.name || item.modId || item.file,
      version: item.version,
      icon: item.icon,
      root: false,
      nested: false,
      virtual: false,
      disabled: item.disable,
      hasProvider: !item.fail,
      required: [],
      hasRequired: false,
    };
    drafts.set(id, draft);
    return draft;
  }

  drafts.set(rootKey, {
    key: rootKey,
    modId: root.modId,
    label: root.name || root.file || root.modId,
    version: root.version,
    icon: root.icon,
    root: true,
    nested: false,
    virtual: false,
    disabled: root.disable,
    hasProvider: !root.fail,
    required: [],
    hasRequired: false,
  });

  /** 深度优先铺开：同一对节点只连一条边，撞上路径上的节点就是环，不再往下走 */
  function walk(key: string, deps: ModDependencyDto[]) {
    path.add(key);
    for (const dep of deps) {
      const id = dep.modId.toLowerCase();
      if (!id) continue;
      const child = nodeOf(id);
      const edgeKey = key + ">" + id;
      if (!edgeSeen.has(edgeKey)) {
        edgeSeen.add(edgeKey);
        edges.push({
          id: edgeKey,
          from: key,
          to: id,
          optional: dep.optional,
          required: dep.required,
          backward: path.has(id),
          loader: false,
        });
        if (!dep.optional) {
          child.hasRequired = true;
          if (dep.required && dep.required !== "*" && !child.required.includes(dep.required)) {
            child.required.push(dep.required);
          }
        }
      }
      // 环上的节点不展开；展开过的也只连边，不重复铺开
      if (path.has(id) || expanded.has(id)) continue;
      const list = providers.get(id) ?? [];
      const chosen = list.find((each) => !each.item.disable) ?? list[0] ?? null;
      if (!chosen) continue;
      expanded.add(id);
      walk(id, chosen.item.dependencies);
    }
    path.delete(key);
  }

  walk(rootKey, root.dependencies);

  // 反向只铺**一层**：谁直接声明了它是前置。
  // 再往上会牵出整张网（一个公共库能被半张包依赖），图就没法看了
  for (const dependent of dependents.get(rootKey) ?? []) {
    const id = dependent.item.modId.toLowerCase();
    if (!id || id === rootKey) continue;
    const edgeKey = id + ">" + rootKey;
    if (edgeSeen.has(edgeKey)) continue;
    edgeSeen.add(edgeKey);
    nodeOfItem(dependent.item);
    edges.push({
      id: edgeKey,
      from: id,
      to: rootKey,
      optional: dependent.dep.optional,
      required: dependent.dep.required,
      backward: false,
      loader: false,
    });
  }

  const nodes: ModDepGraphNode[] = [...drafts.values()].map((draft) => ({
    key: draft.key,
    label: draft.label,
    modId: draft.modId,
    version: draft.version,
    icon: draft.icon,
    root: draft.root,
    nested: draft.nested,
    virtual: draft.virtual,
    installed: draft.hasProvider || draft.virtual,
    // 缺失只认"强制指过来、又没人提供、也不是虚拟前置"：可选前置没装不算缺失
    missing: !draft.hasProvider && !draft.virtual && draft.hasRequired,
    disabled: draft.disabled,
    // 这个包自己是不是"必装"：只有可选边指向它时，用户不装也说得过去
    mandatory: draft.hasRequired,
    required: draft.required,
    x: 0,
    y: 0,
  }));

  // 加载器 → minecraft：两边都出现在图上时才补这一条（模组声明的多半只有其中一边）。
  // 它不是谁声明的，而是运行环境的常识：加载器本来就跑在某个 Minecraft 版本上
  if (drafts.has("minecraft")) {
    for (const key of loaderIds) {
      if (!drafts.has(key)) continue;
      const edgeKey = key + ">minecraft";
      if (edgeSeen.has(edgeKey)) continue;
      edgeSeen.add(edgeKey);
      edges.push({
        id: edgeKey,
        from: key,
        to: "minecraft",
        optional: false,
        required: "*",
        backward: false,
        loader: true,
      });
    }
  }

  // 布局全交给 dagre：分层、层内排序、坐标与长边的绕行都是它的事，
  // 环它自己会反向处理（本文件只把回边标出来供画线时区分）
  const ranked = new graphlib.Graph();
  ranked.setGraph({
    rankdir: "LR",
    ranksep: RANK_GAP,
    nodesep: NODE_GAP,
    marginx: PAD,
    marginy: PAD,
  });
  ranked.setDefaultEdgeLabel(() => ({}));
  for (const node of nodes) ranked.setNode(node.key, { width: NODE_W, height: NODE_H });
  for (const edge of edges) ranked.setEdge(edge.from, edge.to);
  layout(ranked);

  for (const node of nodes) {
    const placed = ranked.node(node.key);
    // dagre 给的是节点中心，vue-flow 的节点位置是左上角
    node.x = Math.round((placed.x ?? 0) - NODE_W / 2);
    node.y = Math.round((placed.y ?? 0) - NODE_H / 2);
  }

  const size = ranked.graph();
  return {
    nodes,
    edges,
    width: Math.ceil(size.width ?? 0),
    height: Math.ceil(size.height ?? 0),
  };
}
