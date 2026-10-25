// 启动前发现服务器包有新版本时的询问
//
// 服务器包启动时本来会按发布地址自动同步；这里在启动前先查一次，有新版本就让用户选：
// 更新后启动 / 直接启动 / 取消。选「直接启动」时带 skip 标志，后端这次不比对清单，
// 免得"提醒"变成"点了取消还是被升级"。
import { ref } from "vue";
import type { ModPackUpdateDto } from "./bindings";

/** 弹窗是否打开 */
export const serverPackOpen = ref(false);
/** 这次要展示的新版本信息；关闭时清空 */
export const serverPackResult = ref<ModPackUpdateDto | null>(null);

/** 用户的选择 */
export type ServerPackAnswer = {
  /** 是否继续启动；false = 取消 */
  ok: boolean;
  /** 是否先同步再启动；false = 直接启动 */
  update: boolean;
};

/** 等待回答的 promise 回调 */
let pending: ((answer: ServerPackAnswer) => void) | null = null;

/**
 * 打开服务器包更新弹窗并等待用户选择
 *
 * - \`result\`: 检查结果，含本地与发布地址上的包版本
 *
 * # 返回值
 *
 * 用户选了继续还是取消，以及要不要先更新
 */
export function askServerPackUpdate(result: ModPackUpdateDto): Promise<ServerPackAnswer> {
  return new Promise((resolve) => {
    pending = resolve;
    serverPackResult.value = result;
    serverPackOpen.value = true;
  });
}

/**
 * 关闭弹窗并回答等待方
 *
 * - \`ok\`: 是否继续启动
 * - \`update\`: 是否先同步再启动
 */
export function answerServerPackUpdate(ok: boolean, update: boolean) {
  serverPackOpen.value = false;
  serverPackResult.value = null;
  const resolve = pending;
  pending = null;
  resolve?.({ ok, update });
}
