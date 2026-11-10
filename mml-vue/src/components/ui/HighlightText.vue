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

/** 一段文本：hit 为 true 表示命中查询词 */
interface Part {
  /** 这一段原文 */
  text: string;
  /** 是否命中查询词 */
  hit: boolean;
}

/** 按查询词切段；查询词为空时整段原样输出 */
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

/** 命中段与普通段的文字可能相同（如查询词本身就是整段），key 用「类型 + 文本 + 出现次数」 */
const keyedParts = computed(() => {
  const seen = new Map<string, number>();
  return parts.value.map((p) => {
    const base = (p.hit ? "h:" : "t:") + p.text;
    const n = seen.get(base) ?? 0;
    seen.set(base, n + 1);
    return { p, key: n ? base + "#" + n : base };
  });
});
</script>

<template>
  <span class="hl">
    <template v-for="{ p, key } in keyedParts" :key="key">
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
  background: var(--accent-soft);
  background: color-mix(in srgb, var(--accent) 30%, transparent);
  color: inherit;
  border-radius: 3px;
  padding: 0 1px;
}
</style>
