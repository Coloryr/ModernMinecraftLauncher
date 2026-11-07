import { reactive, watch } from "vue";

/**
 * 可编辑「一行一条」列表的稳定 key
 *
 * 行内容会随输入变化，拿内容当 key 每敲一个字就会重建输入框、丢焦点；
 * 拿行下标当 key，删中间一行时后面的输入框会复用错位。这里按行数维护一组自增 id，
 * 删行时调用方用 `drop` 同步删掉对应 id。
 *
 * - `count`: 当前行数的取值函数
 *
 * # 返回值
 *
 * - `keys`: 与行对齐的 id，模板里直接 `:key="keys[i]"`
 * - `drop`: 删掉第 index 行的 id，调用方删完行后调一次
 */
export function useLineKeys(count: () => number) {
  const keys = reactive<number[]>([]);
  let next = 1;

  watch(
    () => count(),
    (n) => {
      if (keys.length > n) keys.length = n;
      while (keys.length < n) keys.push(next++);
    },
    { immediate: true },
  );

  return {
    keys,
    drop(index: number) {
      keys.splice(index, 1);
    },
  };
}
