// 崩溃报告文件名工具：从日志文件列表里挑最近一份崩溃报告
//
// Minecraft 的崩溃报告名是 `crash-YYYY-MM-DD_HH.MM.SS-<side>.txt`，文件名里的本地时间
// 就是崩溃发生的时刻，比文件 mtime 更适合判断"是不是这次退出刚写出来的"。

/**
 * 从日志文件名里取崩溃报告时间
 *
 * - `path`: 日志文件绝对路径
 *
 * # 返回值
 *
 * epoch 毫秒；不是崩溃报告或名字认不出时返回 null
 */
export function crashTimeOf(path: string): number | null {
  const name = path.replace(/\\/g, "/").split("/").pop() ?? "";
  const m = /^crash-(\d{4})-(\d{2})-(\d{2})_(\d{2})\.(\d{2})\.(\d{2})/.exec(name);
  if (!m) return null;
  const time = new Date(+m[1], +m[2] - 1, +m[3], +m[4], +m[5], +m[6]).getTime();
  return Number.isFinite(time) ? time : null;
}

/**
 * 最近一份崩溃报告
 *
 * 只认 `maxAgeMs` 内写出的，避免把上一次崩溃的旧报告当成这次退出。
 *
 * - `files`: 实例日志文件列表
 * - `maxAgeMs`: 允许的最大文件年龄，单位毫秒
 *
 * # 返回值
 *
 * 最新的崩溃报告路径；没有时返回 null
 */
export function latestCrash(files: string[], maxAgeMs = 2 * 60 * 1000): string | null {
  const now = Date.now();
  let best: { path: string; time: number } | null = null;
  for (const path of files) {
    const time = crashTimeOf(path);
    if (time === null || now - time > maxAgeMs) continue;
    if (!best || time > best.time) best = { path, time };
  }
  return best?.path ?? null;
}
