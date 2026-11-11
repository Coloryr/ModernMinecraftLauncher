// 安装 / 下载进度弹窗的行数据：Java 安装、资源下载、整合包安装三处共用
//
// 三个功能的任务 DTO 字段不同，弹窗只认这一份归一化结果，行布局与状态样式不再各写一套。

/** 任务状态；running 之外显示成标签 */
export type TaskStatusKind = "running" | "done" | "failed" | "cancelled";

/** 进度弹窗里的一行 */
export interface TaskProgressRow {
  /** 行唯一键，取消时回传 */
  key: string;
  /** 显示名 */
  name: string;
  /** 项目图标；空串 / null 时用首字母占位 */
  icon?: string | null;
  /** 主进度，0–100 */
  percent: number;
  /** 总数未知，进度条走无限滚动 */
  unknown?: boolean;
  /** 右侧状态文案：进行中写阶段 / 进度，终态写完成 / 失败 / 已取消 */
  statusText: string;
  /** 状态类别；running 显示普通文字，其余显示成标签 */
  statusKind: TaskStatusKind;
  /** 失败原因 */
  error?: string | null;
  /** 子进度文案，如当前文件名 */
  subText?: string | null;
  /** 子进度，0–100；null 表示没有子进度 */
  subPercent?: number | null;
  /** 子进度总数未知 */
  subUnknown?: boolean;
  /** 是否可以取消；只有进行中的任务给按钮 */
  cancelable?: boolean;
}
