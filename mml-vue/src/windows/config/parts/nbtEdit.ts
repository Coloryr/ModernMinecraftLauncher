// NBT 编辑的共用口径：注入契约、值文本的校验、搜索命中的判定
//
// 树很深，用 provide / inject 而不是逐层 emit：每个 NbtNode 只关心自己那一行，
// 不必把子节点的事件再转发一遍。
//
// **值文本的解析口径必须与后端 `config_set_nbt` 一致**（见 src-tauri 的 windows/config.rs）：
// 数字按类型判格式与范围，字符串原样，数组是逗号 / 空白分隔的一串数，数组长度上限两边同值。
import type { InjectionKey } from "vue";
import type { NbtKindDto, NbtNodeDto } from "../../../lib/bindings";

/**
 * 树上带编辑痕迹的节点
 *
 * 标记不跨 IPC，直接挂在工作树的节点对象上：增删之后路径会挪位，
 * 按路径记"哪一行改过"会跟丢。
 */
export type NbtTreeNode = NbtNodeDto & {
  /** 这一处有未保存的改动：值改过、数组改过，或新加进来的 */
  touched?: boolean;
};

/** 数组最多展开多少个元素给前端编辑；必须与 config.rs 的 `MAX_ARRAY_EDIT` 一致 */
export const MAX_ARRAY_EDIT = 1024;

/** 标量类型：能就地改值 */
const SCALARS: NbtKindDto[] = ["Byte", "Short", "Int", "Long", "Float", "Double", "String"];
/** 数组类型：当一串数改 */
const ARRAYS: NbtKindDto[] = ["ByteArray", "IntArray", "LongArray"];

/** 新建标签时可选的类型：End 只作文件头的结束标记，不能新建 */
export const ADDABLE_KINDS: NbtKindDto[] = [
  "Byte",
  "Short",
  "Int",
  "Long",
  "Float",
  "Double",
  "String",
  "ByteArray",
  "IntArray",
  "LongArray",
  "List",
  "Compound",
];

/** 是不是数组 */
export function isArrayKind(kind: NbtKindDto): boolean {
  return ARRAYS.includes(kind);
}

/** 是不是标量 */
export function isScalarKind(kind: NbtKindDto): boolean {
  return SCALARS.includes(kind);
}

/** 数组的元素类型：ByteArray → Byte、IntArray → Int、LongArray → Long；不是数组给 null */
export function elementKindOf(kind: NbtKindDto): NbtKindDto | null {
  switch (kind) {
    case "ByteArray":
      return "Byte";
    case "IntArray":
      return "Int";
    case "LongArray":
      return "Long";
    default:
      return null;
  }
}

/**
 * 这一行能不能展开
 *
 * - `kind`: 标签类型
 * - `length`: 元素个数，只对数组有意义
 *
 * # 返回值
 *
 * 复合与列表都能展开；数组在 [`MAX_ARRAY_EDIT`] 以内才展开，再长只报个数
 */
export function isBranchKind(kind: NbtKindDto, length = 0): boolean {
  if (kind === "Compound" || kind === "List") return true;
  return isArrayKind(kind) && length <= MAX_ARRAY_EDIT;
}

/** 数组值文本切成元素：逗号或空白分隔，空串是空数组 */
export function splitArrayText(text: string): string[] {
  return text.split(/[\s,]+/).filter((item) => item !== "");
}

/** 十进制整数且在闭区间内 */
function intInRange(text: string, min: bigint, max: bigint): boolean {
  if (!/^-?\d+$/.test(text)) return false;
  const value = BigInt(text);
  return value >= min && value <= max;
}

/** 一个数组元素按元素类型能不能用 */
function arrayItemOk(kind: NbtKindDto, text: string): boolean {
  const raw = text.trim();
  switch (kind) {
    case "ByteArray":
      return /^\d+$/.test(raw) && Number(raw) <= 255;
    case "IntArray":
      return intInRange(raw, -(2n ** 31n), 2n ** 31n - 1n);
    case "LongArray":
      return intInRange(raw, -(2n ** 63n), 2n ** 63n - 1n);
    default:
      return false;
  }
}

/**
 * 值按类型能不能用
 *
 * - `kind`: 标签类型
 * - `text`: 值文本
 *
 * # 返回值
 *
 * 数字判格式与范围，字符串一律照收，数组判每个元素与个数；复合与列表不看值
 */
export function valueOk(kind: NbtKindDto, text: string): boolean {
  if (isArrayKind(kind)) {
    const items = splitArrayText(text);
    return items.length <= MAX_ARRAY_EDIT && items.every((item) => arrayItemOk(kind, item));
  }

  const raw = text.trim();
  switch (kind) {
    case "Byte":
      return /^\d+$/.test(raw) && Number(raw) <= 255;
    case "Short":
      return intInRange(raw, -(2n ** 15n), 2n ** 15n - 1n);
    case "Int":
      return intInRange(raw, -(2n ** 31n), 2n ** 31n - 1n);
    case "Long":
      return intInRange(raw, -(2n ** 63n), 2n ** 63n - 1n);
    case "Float":
    case "Double":
      return raw !== "" && Number.isFinite(Number(raw));
    default:
      return true;
  }
}

/** 路径的 JSON 串当键；搜索命中的集合按它记，与节点的身份无关 */
export function pathKey(path: string[]): string {
  return JSON.stringify(path);
}

/** 树上一层的子节点：复合用 `children`，列表与数组用 `items` */
export function kidsOf(node: NbtTreeNode): NbtTreeNode[] {
  return node.kind === "Compound" ? node.children : node.items;
}

/** 标量 / 列表元素 / 复合成员 / 数组的登记表，由 NbtTree 实现 */
export interface NbtEditApi {
  /**
   * 这一行手动铺开过没有
   *
   * - `path`: 该节点的路径
   *
   * # 返回值
   *
   * 用户手动定过的展开状态；没定过返回 `undefined`，由调用方按默认档处理
   */
  openedOf(path: string[]): boolean | undefined;
  /**
   * 记住这一行的展开状态
   *
   * - `path`: 该节点的路径
   * - `open`: 铺开还是收起
   */
  setOpened(path: string[], open: boolean): void;
  /**
   * 改一处值，就地改在工作树上，同时记一条待保存的改动
   *
   * - `node`: 要改的节点，工作树上的对象
   * - `path`: 该节点的路径
   * - `value`: 新值文本
   */
  set(node: NbtTreeNode, path: string[], value: string): void;
  /**
   * 往容器里加一个标签
   *
   * - `container`: 复合或列表节点
   * - `containerPath`: 容器路径
   * - `key`: 复合的键名；列表不用，传空串
   * - `kind`: 新标签的类型
   * - `value`: 新标签的值文本
   */
  add(
    container: NbtTreeNode,
    containerPath: string[],
    key: string,
    kind: NbtKindDto,
    value: string,
  ): void;
  /**
   * 改复合成员的键名，整棵子树跟着搬
   *
   * - `container`: 所在的复合
   * - `containerPath`: 该复合的路径
   * - `index`: 成员在复合里的位置
   * - `newName`: 新键名
   */
  rename(container: NbtTreeNode, containerPath: string[], index: number, newName: string): void;
  /**
   * 删掉容器里的一个子节点
   *
   * - `container`: 复合或列表节点
   * - `containerPath`: 容器路径
   * - `index`: 子节点在容器里的位置
   */
  remove(container: NbtTreeNode, containerPath: string[], index: number): void;
}

/** 注入键 */
export const NBT_EDIT: InjectionKey<NbtEditApi> = Symbol("nbt-edit");
