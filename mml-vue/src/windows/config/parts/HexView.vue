<script setup lang="ts">
// 二进制视图：一行 16 字节，左边十六进制、右边 ASCII
//
// 数据来自 `config_get_bytes_slice`（后端一次最多 64 KiB），这里一页只拉 4 KiB：
// 256 行足够铺满，翻页时也只搬这一页的字节，不会把整个文件塞进内存。
import { computed, ref, watch } from "vue";
import { t, tErr } from "../../../lib/i18n";
import { showToast } from "../../../lib/toast";
import { readConfigBytes } from "../../../lib/api";
import BaseButton from "../../../components/ui/BaseButton.vue";

const props = defineProps<{
  /** 实例 uuid */
  uuid: string;
  /** 来源，空串为实例本身，否则是存档目录名 */
  source: string;
  /** 来源内的相对文件路径 */
  file: string;
}>();

/** 一页的字节数：256 行 × 16 字节 */
const PAGE = 4096;

/** 本页起始偏移，单位字节 */
const offset = ref(0);
/** 文件总大小，单位字节 */
const total = ref(0);
/** 本页字节 */
const bytes = ref<Uint8Array>(new Uint8Array());
const loading = ref(false);

/** 十六进制视图里的一行 */
interface HexRow {
  /** 行首偏移，8 位十六进制 */
  offset: string;
  /** 十六进制列 */
  hex: string;
  /** ASCII 列，不可打印字符写 `.` */
  text: string;
}

const rows = computed<HexRow[]>(() => {
  const list: HexRow[] = [];
  for (let i = 0; i < bytes.value.length; i += 16) {
    const chunk = bytes.value.subarray(i, i + 16);
    const chars = [...chunk];
    list.push({
      offset: (offset.value + i).toString(16).padStart(8, "0"),
      hex: chars.map((b) => b.toString(16).padStart(2, "0")).join(" "),
      text: chars.map((b) => (b >= 0x20 && b < 0x7f ? String.fromCharCode(b) : ".")).join(""),
    });
  }
  return list;
});

// Base64 解回字节：`atob` 给的是"每字符一字节"的二进制串
function decode(base64: string): Uint8Array {
  const raw = atob(base64);
  const out = new Uint8Array(raw.length);
  for (let i = 0; i < raw.length; i += 1) out[i] = raw.charCodeAt(i);
  return out;
}

/** 拉当前这一页 */
async function load() {
  loading.value = true;
  try {
    const slice = await readConfigBytes(props.uuid, props.source, props.file, offset.value, PAGE);
    offset.value = slice.offset;
    total.value = slice.total;
    bytes.value = decode(slice.data);
  } catch (e) {
    showToast(tErr(e));
  } finally {
    loading.value = false;
  }
}

/** 翻页；偏移夹在 [0, total) 里，到尾巴那一页就是不足一页的内容 */
function turn(step: number) {
  const last = Math.max(0, total.value - 1);
  offset.value = Math.max(0, Math.min(offset.value + step * PAGE, last));
  void load();
}

// 换来源或换文件时回到开头；`immediate` 让首次打开就有内容
watch(
  () => [props.uuid, props.source, props.file],
  () => {
    offset.value = 0;
    void load();
  },
  { immediate: true },
);
</script>

<template>
  <div class="hex-view flex flex-col min-h-0 h-full">
    <div class="hex-rows flex-1 min-h-0 overflow-auto px-2.5 py-2 [font-family:ui-monospace,Cascadia_Mono,Consolas,monospace] text-[12px] leading-[1.6]">
      <div v-for="row in rows" :key="row.offset" class="hex-row flex gap-3.5 whitespace-pre">
        <span class="hex-off text-dim">{{ row.offset }}</span>
        <span class="hex-bytes shrink-0 text-fg">{{ row.hex }}</span>
        <span class="hex-text min-w-0 overflow-hidden text-dim">{{ row.text }}</span>
      </div>
      <p v-if="!rows.length && !loading" class="empty-tip">{{ t("config.hexEmpty") }}</p>
    </div>
    <div class="hex-bar flex items-center gap-2 pt-2">
      <BaseButton size="sm" :disabled="loading || offset === 0" @click="turn(-1)">
        {{ t("config.hexPrev") }}
      </BaseButton>
      <BaseButton size="sm" :disabled="loading || offset + PAGE >= total" @click="turn(1)">
        {{ t("config.hexNext") }}
      </BaseButton>
      <span class="hex-info text-dim text-meta">
        {{ t("config.hexRange", { from: offset, to: offset + bytes.length, total }) }}
      </span>
    </div>
  </div>
</template>

