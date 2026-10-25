// 在线实例的地址口径：MML 服务器包认 server.json，整合包压缩包照旧收
//
// 用户可能填发布目录，也可能直接贴清单文件：目录地址补上 server.json 再发请求；
// 实例里存的仍是目录，启动同步时自己拼 server.json 与 .sha1（见 mml-game 的 server_pack_update）。

/** 压缩包类地址的后缀：这些走"下载压缩包"那条导入路径 */
const ARCHIVE_EXTS = [".zip", ".mrpack", ".7z", ".rar", ".tar", ".gz", ".tgz"];

/**
 * 这个地址是不是整合包压缩包
 *
 * - `url`: 用户填的地址
 *
 * # 返回值
 *
 * 是压缩包地址返回 `true`，其余按服务器包清单处理
 */
export function isArchiveUrl(url: string): boolean {
  // 查询串与锚点不算扩展名，`pack.zip?raw=1` 仍是压缩包
  const path = url.trim().toLowerCase().split(/[?#]/)[0];
  return ARCHIVE_EXTS.some((ext) => path.endsWith(ext));
}

/**
 * 把在线实例地址补成清单地址
 *
 * - `url`: 发布目录，或已经是 server.json 的地址
 *
 * # 返回值
 *
 * 以 server.json 结尾的地址；空串原样返回
 */
export function serverPackUrl(url: string): string {
  const value = url.trim().replace(/\/+$/, "");
  if (!value) return "";
  const lower = value.toLowerCase();
  const done = lower.endsWith("/server.json") || lower === "server.json";
  return done ? value : `${value}/server.json`;
}