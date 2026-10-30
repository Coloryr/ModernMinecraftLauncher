// 收藏星标切换：状态以后端为准，失败只提示
import { api } from "./api";
import { tErr } from "./i18n";
import { showToast } from "./toast";
import type { ProjectItemDto } from "./bindings";

/**
 * 切换项目在收藏夹里的状态
 *
 * 失败时只弹提示、不动 `isStar`：本地状态跟着后端走，否则界面会留下一个没收藏上的星标
 *
 * - `item`: 目标项目，成功后原地改它的 `isStar`
 */
export async function toggleStar(item: ProjectItemDto): Promise<void> {
  const star = !item.isStar;
  try {
    await api.collectStar(
      item.source.source,
      item.source.fileType,
      item.source.pid,
      item.name,
      item.image,
      item.url,
      star,
      // 不归到任何具名分组：默认分组的成员按资源类型现算，收藏进来就在自己那一类里
      null,
    );
    item.isStar = star;
  } catch (e) {
    showToast(tErr(e));
  }
}
