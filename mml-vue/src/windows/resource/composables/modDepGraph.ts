// 模组依赖图的**布局**：节点与连线由后端算好（见 AGENTS.md §12.1），
// 这里只把图交给 dagre 排成分层结构（从左到右），产出带坐标的节点。
//
// 关系（谁依赖谁、前置齐不齐、装没装、缺没缺）一个字都不在这里算 —— 前端只留画图要用的坐标。
// 建图口径与各字段含义见后端 windows/resource/dep_tree.rs。
import { graphlib, layout } from "@dagrejs/dagre";
import type { ModDepEdgeDto, ModDepNodeDto, ModDepTreeDto } from "../../../lib/bindings";

/** 布局后的一个节点：后端给的节点 + 坐标 */
export interface ModDepGraphNode extends ModDepNodeDto {
  /** 布局坐标，单位 px，节点左上角 */
  x: number;
  y: number;
}

/** 算好布局的依赖图 */
export interface ModDepGraph {
  /** 节点 */
  nodes: ModDepGraphNode[];
  /** 连线 */
  edges: ModDepEdgeDto[];
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

/**
 * 给后端给的依赖图算好分层布局
 *
 * 布局全交给 dagre：分层、层内排序、坐标与长边的绕行都是它的事；
 * 环它自己会反向处理，回边由后端标出来，画线时区分。
 *
 * - `tree`: 后端返回的依赖图，只有节点与连线
 *
 * # 返回值
 *
 * 见 [`ModDepGraph`]，节点带上布局坐标
 */
export function layoutDepGraph(tree: ModDepTreeDto): ModDepGraph {
  const ranked = new graphlib.Graph();
  ranked.setGraph({
    rankdir: "LR",
    ranksep: RANK_GAP,
    nodesep: NODE_GAP,
    marginx: PAD,
    marginy: PAD,
  });
  ranked.setDefaultEdgeLabel(() => ({}));
  for (const node of tree.nodes) ranked.setNode(node.key, { width: NODE_W, height: NODE_H });
  for (const edge of tree.edges) ranked.setEdge(edge.from, edge.to);
  layout(ranked);

  const nodes: ModDepGraphNode[] = tree.nodes.map((node) => {
    const placed = ranked.node(node.key);
    // dagre 给的是节点中心，vue-flow 的节点位置是左上角
    return {
      ...node,
      x: Math.round((placed.x ?? 0) - NODE_W / 2),
      y: Math.round((placed.y ?? 0) - NODE_H / 2),
    };
  });

  const size = ranked.graph();
  return {
    nodes,
    edges: tree.edges,
    width: Math.ceil(size.width ?? 0),
    height: Math.ceil(size.height ?? 0),
  };
}
