<script setup lang="ts">
// 把文本里命中查询词的部分包成 <mark>；大小写不敏感，查询词为空或没命中就原样输出
// 用于方块名等搜索结果，让用户看出「为什么这个条目被搜出来」
import { computed } from "vue";

const props = defineProps<{
  /** 待高亮的完整文本 */
  text: string;
  /** 查询词；空串时整段原样输出 */
  query: string;
}>();

interface Part {
  text: string;
  hit: boolean;
}

const parts = computed<Part[]>(() => {
  const q = props.query.trim();
  if (!q) return [{ text: props.text, hit: false }];
  // 转义正则元字符：用户可能输入 . * ( ) [ 等
  const re = new RegExp(`(${q.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")})`, "gi");
  const out: Part[] = [];
  let last = 0;
  for (const m of props.text.matchAll(re)) {
    const at = m.index ?? 0;
    if (at > last) out.push({ text: props.text.slice(last, at), hit: false });
    out.push({ text: m[0], hit: true });
    last = at + m[0].length;
  }
  if (last < props.text.length) out.push({ text: props.text.slice(last), hit: false });
  return out;
});
</script>

<template>
  <span class="hl">
    <template v-for="(p, i) in parts" :key="i">
      <mark v-if="p.hit" class="hl-hit">{{ p.text }}</mark>
      <template v-else>{{ p.text }}</template>
    </template>
  </span>
</template>

<style scoped>
.hl {
  display: inline;
}

.hl-hit {
  /* 兜底先给一层半透明强调色，再叠 color-mix；不支持 color-mix 的 webview 也有底色 */
  background: var(--accent-soft, rgba(63, 140, 255, 0.28));
  background: color-mix(in srgb, var(--accent) 30%, transparent);
  color: inherit;
  border-radius: 3px;
  padding: 0 1px;
}
</style>
