// 服务器包更新前选哪些可选模组要更新
//
// 更新会按清单覆盖文件；可选模组本来就是玩家的选择，先摆出来让玩家去掉这次不想动的。
// 没勾的那几个不下载新版本，本地旧文件原样留着，等下次发布方换了内容再问。
import { ref } from "vue";
import type { ServerPackModDto } from "./bindings";

/** 弹窗是否打开 */
export const serverPackModsOpen = ref(false);
/** 这次会变的可选模组；关闭时清空 */
export const serverPackModsList = ref<ServerPackModDto[]>([]);

/** 等待回答的 promise 回调 */
let pending: ((skip: string[] | null) => void) | null = null;

/**
 * 打开可选模组选择弹窗并等待用户确认
 *
 * - `mods`: 这次会变的可选模组，取更新检查结果里的 mods
 *
 * # 返回值
 *
 * 这次不更新的文件路径；用户取消返回 null
 */
export function askServerPackMods(mods: ServerPackModDto[]): Promise<string[] | null> {
  // 上一问未回答时按"取消"放掉，避免旧 Promise 永远不 settle
  pending?.(null);
  pending = null;

  return new Promise((resolve) => {
    pending = resolve;
    serverPackModsList.value = mods;
    serverPackModsOpen.value = true;
  });
}

/**
 * 关闭弹窗并回答等待方
 *
 * - `skip`: 这次不更新的文件路径；取消传 null
 */
export function answerServerPackMods(skip: string[] | null): void {
  serverPackModsOpen.value = false;
  serverPackModsList.value = [];
  const resolve = pending;
  pending = null;
  resolve?.(skip);
}
