// 整合包安装进度换算
//
// 内核把安装拆成几个阶段（读信息 / 解压 / 取文件信息 / 下载 / 完成），
// `now` 是当前阶段序号、`total` 是阶段总数；阶段内部再用 `sub_now` / `sub_total`
// 报细项进度（解压的条目、下载的文件）。只按 `now / total` 画的话，
// 整段解压期间进度条是钉住的 —— 这里把阶段内的细项进度折算进去。

/** 带"阶段 + 阶段内细项"两级进度的计数快照 */
export type ProgressCounts = {
  /** 当前阶段序号 */
  now: number;
  /** 阶段总数；0 表示还没给出 */
  total: number;
  /** 阶段内细项已完成数 */
  subNow: number;
  /** 阶段内细项总数；0 表示这一阶段没有细项进度 */
  subTotal: number;
};

/**
 * 安装进度百分比
 *
 * 阶段边界上的值仍是 `now / total`，不会因为折算往回跳
 *
 * - `progress`: 安装进度快照
 *
 * # 返回值
 *
 * 0–100；阶段总数与细项总数都没有时返回 `null`，由调用方显示为不确定进度
 */
export function progressPercent(progress: ProgressCounts): number | null {
  const sub = progress.subTotal ? Math.min(1, progress.subNow / progress.subTotal) : 0;
  if (!progress.total) {
    // 阶段总数还没给（例如还在下载整合包本身）：只有细项进度可用
    return progress.subTotal ? sub * 100 : null;
  }
  return Math.min(100, ((progress.now + sub) / progress.total) * 100);
}
