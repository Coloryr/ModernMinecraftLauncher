// 本地压缩包后缀口径
//
// 拖入窗口 / 命令行给入的路径先按后缀认：认得就直接进"导入压缩包"，
// 不必先开一次包探测 —— 1GB 的整合包开一次要好几秒，窗口会干等。
// 后缀与后端 `window_probe_drop` 认的那一档保持一致。

/** 本地整合包压缩包的后缀 */
const ARCHIVE_EXTS = [".zip", ".mrpack", ".modpack"];

/**
 * 这个本地路径是不是整合包压缩包
 *
 * - `path`: 本机绝对路径
 *
 * # 返回值
 *
 * 是压缩包返回 `true`；其余类型交给后端探测
 */
export function isArchivePath(path: string): boolean {
  const lower = path.trim().toLowerCase();
  return ARCHIVE_EXTS.some((ext) => lower.endsWith(ext));
}
