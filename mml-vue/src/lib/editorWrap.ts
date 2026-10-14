// 编辑器自动换行：**全局一份**偏好，日志视图与配置编辑器共用
//
// 与 VS Code 的 `editor.wordWrap` 一个口径：一份设置管所有编辑器，Alt+Z 在哪个视图按都在改它，
// 不是每个窗口、更不是每个实例各存一份（那正是"两边显示不一致"的来源）。
//
// 落在 [`KEYS.editorWordWrap`] 里：多窗口模式下日志与配置是两个窗口，
// 改一处由 [`onStorageChange`] 同步过去，界面上的开关因此跟着动。
import { ref, watch } from "vue";
import { KEYS, emitChange, onStorageChange, readFlag, writeFlag } from "./storage";

/** 当前是否自动换行；默认开，与日志视图一直以来的行为一致 */
export const wordWrap = ref(readFlag(KEYS.editorWordWrap, true));

watch(wordWrap, (on) => {
  writeFlag(KEYS.editorWordWrap, on);
  emitChange(KEYS.editorWordWrap, on ? "1" : "0");
});

// 别的窗口翻转了：跟上。值没变就不动，免得两边来回弹
onStorageChange(KEYS.editorWordWrap, (v) => {
  const on = v !== "0";
  if (on !== wordWrap.value) wordWrap.value = on;
});

/** 翻转换行：Alt+Z 与界面上那个开关都走这一条 */
export function toggleWordWrap(): void {
  wordWrap.value = !wordWrap.value;
}
