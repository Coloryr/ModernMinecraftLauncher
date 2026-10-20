// 启动后自动进入存档的确认弹窗
//
// 启动流程（主窗口 / 日志窗口 / 分组与批量启动）共用：启动前向后端要一次实例配的
// 自动进入存档，有就挂起询问，由 App 里常驻的 AutoSaveModal 回答。
import { ref } from "vue";
import type { AutoSaveTargetDto } from "./bindings";

/** 弹窗是否打开 */
export const autoSaveOpen = ref(false);

/** 这次问的是哪个存档；关闭后清空 */
export const autoSavePick = ref<AutoSaveTargetDto | null>(null);

/** 用户的选择 */
export type AutoSaveAnswer = {
  /** 这次是否进入该存档 */
  enter: boolean;
  /** 是否保留启动提醒；false = 不再提醒 */
  notice: boolean;
  /** 是否清掉该实例的自动进入配置 */
  disable: boolean;
};

/** 等待回答的 promise 回调 */
let pending: ((answer: AutoSaveAnswer) => void) | null = null;

/**
 * 打开自动进入确认弹窗并等待用户选择
 *
 * - `target`: 要进入的存档，目录名与显示名
 *
 * # 返回值
 *
 * 用户是否进入、是否继续提醒、是否清掉该实例的配置
 */
export function askAutoSave(target: AutoSaveTargetDto): Promise<AutoSaveAnswer> {
  return new Promise((resolve) => {
    pending = resolve;
    autoSavePick.value = target;
    autoSaveOpen.value = true;
  });
}

/**
 * 关闭弹窗并回答等待方
 *
 * - `answer`: 用户的选择
 */
export function answerAutoSave(answer: AutoSaveAnswer) {
  autoSaveOpen.value = false;
  autoSavePick.value = null;
  const resolve = pending;
  pending = null;
  resolve?.(answer);
}
