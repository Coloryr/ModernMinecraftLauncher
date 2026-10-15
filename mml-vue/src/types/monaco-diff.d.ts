// Monaco 的行级 diff 实现以纯 JS 交付，深路径导入要让 TS 放行。
// 路径按包的 exports 映射写：`monaco-editor/*` → `esm/vs/*.js`，所以**不要**带 `esm/vs/` 前缀，
// 与 monaco-languages.d.ts 同一口径。
//
// 这是 Monaco 的内部模块（不在 editor.api 的公开类型里），但保存配置时的行级差异就用它：
// 上游的 Myers 实现，比自己在 TS 里重写一套稳妥。升级 monaco 时若这里对不上，
// 编译会在这里报错，见 mml-vue/src/lib/textDiff.ts 的用法。
declare module "monaco-editor/base/common/diff/diff.js" {
  /** 一段差异：两侧各自的起止下标 */
  export interface DiffChange {
    /** 原文里的起始下标，含 */
    originalStart: number;
    /** 原文里这一段有几个元素 */
    originalLength: number;
    /** 新文里的起始下标，含 */
    modifiedStart: number;
    /** 新文里这一段有几个元素 */
    modifiedLength: number;
  }

  /** 差异结果 */
  export interface DiffResult {
    /** 是否因为提前放弃而没算完；本仓库不传判定函数，恒为 false */
    quitEarly: boolean;
    /** 差异段，按原文下标升序、互不重叠 */
    changes: DiffChange[];
  }

  /** 参与比较的序列；元素是字符串时按内容严格比较 */
  export interface DiffSequence<T> {
    getElements(): T[];
  }

  /** Myers 差异算法（`An O(ND) Difference Algorithm and its Variations`） */
  export class LcsDiff<T> {
    /**
     * - `originalSequence`: 原文序列
     * - `modifiedSequence`: 新文序列
     * - `continueProcessingPredicate`: 每轮问一次"还要不要接着算"，返回 false 就带着
     *   没算完的结果收工（`quitEarly` 为真）；不传则一直算到底
     */
    constructor(
      originalSequence: DiffSequence<T>,
      modifiedSequence: DiffSequence<T>,
      continueProcessingPredicate?: ((originalIndex: number, matchLength: number) => boolean) | null,
    );
    /** 算差异；`pretty` 只是内部打印用，我们的用法传 false */
    ComputeDiff(pretty: boolean): DiffResult;
  }
}
