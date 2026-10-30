// 展示用的纯格式化：字节数 / 时间戳 / 日期 / 内存档位
//
// 只把数变成给人看的字符串，不含业务判断。单位符号与本地化时间格式不参与翻译，
// 但各处口径必须一致，比如两处体积显示的小数位，所以集中在这里一份。

/**
 * 字节数转可读大小
 *
 * - `bytes`: 字节数
 *
 * # 返回值
 *
 * 1024 以下按 B，之后按 KB / MB 留一位小数、GB 留两位（一位小数在 GB 上是 100MB 的粒度，太粗）
 */
export function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
  return `${(bytes / 1024 / 1024 / 1024).toFixed(2)} GB`;
}

/**
 * 时间戳转本地时间文本
 *
 * - `ms`: Unix 毫秒时间戳；0 与缺失视为没有时间
 *
 * # 返回值
 *
 * 没有时间时是 `--`
 */
export function formatTime(ms: number): string {
  if (!ms) return "--";
  return new Date(ms).toLocaleString();
}

/**
 * 时间戳转"年月日 时:分"，不含秒
 *
 * 卡片里那类一行放不下的地方用它，口径与 `formatTime` 一致，0 与缺失同样是 `--`。
 *
 * - `ms`: Unix 毫秒时间戳
 *
 * # 返回值
 *
 * 形如 `2026/10/22 22:48`；没有时间时是 `--`
 */
export function formatMinute(ms: number): string {
  if (!ms) return "--";
  return new Date(ms).toLocaleString(undefined, {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  });
}

/**
 * 日期文本取年月日
 *
 * - `date`: 形如 `2024-05-01T12:00:00Z` 的日期文本
 *
 * # 返回值
 *
 * 前 10 个字符，即 `2024-05-01`；空串原样返回
 */
export function formatDate(date: string): string {
  return date ? date.slice(0, 10) : "";
}

/**
 * 内存大小转档位标签
 *
 * - `mib`: 内存大小，单位 MiB
 *
 * # 返回值
 *
 * 整数 GiB 写成 `2 GB`，否则写成 `1536 MB`
 */
export function memoryLabel(mib: number): string {
  return mib % 1024 === 0 ? `${mib / 1024} GB` : `${mib} MB`;
}

/**
 * 把压缩包内的完整路径压成一行，保留末尾文件名
 *
 * - `path`: 压缩包内路径
 * - `max`: 最多显示多少字符，超出时优先保留上级目录与扩展名
 *
 * # 返回值
 *
 * 形如 `.../父目录/文件名`；文件名本身过长时缩短中段
 */
export function shortPath(path: string, max = 64): string {
  const normalized = path.replace(/\\/g, "/");
  if (normalized.length <= max) return normalized;

  const parts = normalized.split("/").filter(Boolean);
  const file = parts.pop() ?? normalized;
  const parent = parts.pop();
  const dot = file.lastIndexOf(".");
  const ext = dot > 0 && dot < file.length - 1 ? file.slice(dot) : "";
  const base = ext ? file.slice(0, dot) : file;

  /** 把文件名缩成 head...tail */
  const shortenBase = (budget: number) => {
    if (base.length <= budget) return base;
    const head = Math.max(1, Math.ceil(budget / 2));
    return `${base.slice(0, head)}...${base.slice(-(budget - head))}`;
  };

  const prefix = parent ? `${parent}/` : "";
  const budget = max - 4 - prefix.length - ext.length - 3;
  if (budget >= 8) {
    return `.../${prefix}${shortenBase(budget)}${ext}`;
  }

  // 上级目录太长：只保留文件名，仍然缩短中段并保留扩展名
  const onlyFileBudget = Math.max(8, max - 4 - ext.length - 3);
  return `.../${shortenBase(onlyFileBudget)}${ext}`;
}
