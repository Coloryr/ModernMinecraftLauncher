// 列表星标与收藏夹同步
//
// 下载整合包 / 下载资源两个窗口的星标是搜索时从后端快照下来的，收藏在别处被改过
// （收藏窗口删掉一条、另一个窗口又收藏了一项目）它就过期了 —— 表现是"删了收藏，这边星标还亮着"。
// 收藏变更走全局广播，这里订阅一次，把看得见的条目按最新的收藏夹重算一遍。
import { onMounted, onUnmounted } from "vue";
import { api, onCollectChange } from "./api";
import type { ProjectItemDto } from "./bindings";

/**
 * 让列表里的星标跟着收藏夹走
 *
 * - `items`: 当前要同步的条目，事件到达时重新求值（列表 + 打开着的详情都在里面对得上）
 *
 * # 返回值
 *
 * - `sync`: 手动同步一次，窗口把新条目塞进列表后可以立刻调，省得等下一发事件
 */
export function useCollectStars(items: () => ProjectItemDto[]) {
  let unlisten: (() => void) | null = null;
  // 组件可能在监听建立完成前就卸载，之后到的退订函数要立刻执行，否则监听泄漏
  let disposed = false;

  /** 按最新收藏夹重算每条目的 isStar */
  async function sync(): Promise<void> {
    try {
      const data = await api.collectGetData();
      const starred = new Set(data.items.map((i) => `${i.source}:${i.pid}`));
      for (const item of items()) {
        item.isStar = starred.has(`${item.source.source}:${item.source.pid}`);
      }
    } catch {
      // 拉不到就保持原样：星标错一会儿，比清空列表好
    }
  }

  onMounted(async () => {
    // 非 Tauri 环境下 listen 会拒绝；星标保持原样即可
    const un = await onCollectChange(() => void sync()).catch(() => null);
    if (!un) return;
    if (disposed) un();
    else unlisten = un;
  });

  onUnmounted(() => {
    disposed = true;
    unlisten?.();
    unlisten = null;
  });

  return { sync };
}