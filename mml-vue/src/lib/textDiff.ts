// 文本配置文件的保存差异：只把"改了哪几行"送回后端，整份文件不再过 IPC
//
// 为什么用差异：配置文件上限 4 MiB，改一行却要把整份回传（还要后端整份重写一遍），
// 大文件上纯属白搬。差异让每次保存只走改动的那几行。
//
// 行级差异直接用 Monaco 内置的 LcsDiff（`base/common/diff/diff.js`，Myers 算法），
// 不自己造轮子；它是 Monaco 的内部模块，类型垫片见 src/types/monaco-diff.d.ts。
//
// **与后端的约定**（`config_save_file`，见 src-tauri 的 windows/config.rs）：
// - 文本按 `\n` 切行、按 `\n` 拼回，两端对同一份文本必须切出同样的行数；
// - 差异里的行号一律**相对原文**（打开时读到的那一份），各段按 `start` 升序、互不重叠；
// - 后端先按打开时的 SHA1 核对磁盘上还是不是那一份，再套差异，并核对改完的行数。
import { LcsDiff } from "monaco-editor/base/common/diff/diff.js";

/** 一次行替换：把原文 `start` 起的 `deleteCount` 行换成 `lines` */
export interface LineHunk {
  /** 原文里的起始行号，从 0 起 */
  start: number;
  /** 要替换掉的行数；0 表示纯插入 */
  deleteCount: number;
  /** 替换成的新行；空数组表示纯删除 */
  lines: string[];
}

/** 文本按 `\n` 切行；与后端同一口径 */
export function splitLines(text: string): string[] {
  return text.split("\n");
}

/** 行列表按 `\n` 拼回文本；与 [`splitLines`] 互为逆运算 */
export function joinLines(lines: string[]): string {
  return lines.join("\n");
}

/** LcsDiff 只要求一个"能给出元素列表"的序列对象 */
function sequence(lines: string[]) {
  return { getElements: () => lines };
}

/**
 * 差异最多算多少轮，超了就退回"整段替换"
 *
 * Myers 是 O(行数 × 改动数)：把一个大文件整份换掉要好几秒，界面就那么卡着。
 * 每轮问一次够不够（Monaco 的 `continueProcessingPredicate`），超过这个轮数就收工。
 * 4000 轮的代价是千万级比较，几十毫秒量级。
 */
const MAX_DIFF_STEPS = 4000;

/**
 * 用一段替换盖住改动的那一片：掐掉两侧相同的行，中间整段换掉
 *
 * 前后缀逐行相同，所以拿这一段替换回去必然得到 `edited` —— 与逐行算出来的结果等价，
 * 只是改动分散时发回去的行会多一些。
 */
function windowHunk(origin: string[], edited: string[]): LineHunk {
  const max = Math.min(origin.length, edited.length);

  let prefix = 0;
  while (prefix < max && origin[prefix] === edited[prefix]) prefix += 1;

  let suffix = 0;
  while (suffix < max - prefix && origin[origin.length - 1 - suffix] === edited[edited.length - 1 - suffix]) {
    suffix += 1;
  }

  return {
    start: prefix,
    deleteCount: origin.length - prefix - suffix,
    lines: edited.slice(prefix, edited.length - suffix),
  };
}

/**
 * 算出行级差异
 *
 * 改动大到算不完（见 [`MAX_DIFF_STEPS`]）时给一段整段替换：改一整片时结果本来就是这样，
 * 没必要让界面卡上几秒。
 *
 * - `origin`: 原文的行列表，即打开文件时读到的内容
 * - `edited`: 编辑后的行列表
 *
 * # 返回值
 *
 * 行替换列表，行号相对原文、按 `start` 升序；没有改动时是空数组
 */
export function diffLines(origin: string[], edited: string[]): LineHunk[] {
  let steps = 0;
  const diff = new LcsDiff(sequence(origin), sequence(edited), () => {
    steps += 1;
    return steps <= MAX_DIFF_STEPS;
  }).ComputeDiff(false);

  if (diff.quitEarly) return [windowHunk(origin, edited)];

  return diff.changes.map((change) => ({
    start: change.originalStart,
    deleteCount: change.originalLength,
    lines: edited.slice(change.modifiedStart, change.modifiedStart + change.modifiedLength),
  }));
}

/**
 * 把差异套回原文，得到新的行列表
 *
 * 发出去之前用它自检一次：套回来的结果必须与编辑中的内容逐字相同，否则宁可退回
 * "整段替换"，也不能让后端按错的差异改盘上的文件。
 *
 * - `origin`: 原文的行列表
 * - `hunks`: 行替换列表
 *
 * # 返回值
 *
 * 新的行列表
 */
export function applyHunks(origin: string[], hunks: LineHunk[]): string[] {
  const lines = origin.slice();
  // 行号相对原文，倒着改前面的才不会被后面影响
  for (const hunk of [...hunks].sort((a, b) => b.start - a.start)) {
    lines.splice(hunk.start, hunk.deleteCount, ...hunk.lines);
  }
  return lines;
}
