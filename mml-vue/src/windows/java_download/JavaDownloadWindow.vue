<script setup lang="ts">
// Java 下载窗口：搜索源 Adoptium / Zulu / OpenJ9 / Foojay +
// 发行类型 / 主版本 / 系统 / 架构 四个下拉筛选拟下载的 Java
//
// 链路：java_download_get_options(source) 取四个下拉的候选 →
//       java_download_get_list(...) 按筛选条件拉匹配的包列表 →
//       java_download_start(uuid) 下载压缩包，后端解包识别并注册进 Java 列表
import { onMounted, ref } from "vue";
import WindowFrame from "../../components/ui/WindowFrame.vue";
import BaseButton from "../../components/ui/BaseButton.vue";
import BaseModal from "../../components/ui/BaseModal.vue";
import { useWindowRefresh } from "../../composables/useWindowRefresh";
import { t, tErr } from "../../lib/i18n";
import { showToast } from "../../lib/toast";
import {
  commands,
  type JavaDownloadItemDto,
  type JavaDownloadOptionsDto,
  type JavaTypes,
} from "../../lib/bindings";
import { isTauri } from "../windowManager";
import { detectOs } from "../../lib/titlebar";

/** 请求关闭本窗口 */
defineEmits<{ (e: "close"): void }>();

const inTauri = isTauri();

const sources = ref<JavaTypes[]>([]);
const source = ref<JavaTypes | null>(null);
const options = ref<JavaDownloadOptionsDto | null>(null);
const optionsLoading = ref(false);

const javaType = ref("");
const major = ref<number | null>(null);
const system = ref("");
const arch = ref("");

/** 按当前筛选条件匹配出的包列表（窗口下方 grid 显示） */
const items = ref<JavaDownloadItemDto[]>([]);
const listLoading = ref(false);

/** 本机架构：探不出来为 null，回落到选第一项 */
type PlatformArch = "x86_64" | "x86" | "aarch64" | "arm" | null;

/**
 * 探测本机 CPU 架构
 *
 * # 返回值
 *
 * 架构名；两项探测都认不出时为 null
 */
async function detectArch(): Promise<PlatformArch> {
  // Chromium 的 userAgentData 能给出真实架构：Windows ARM 的 UA 仍伪装成 x64，
  // 所以不能只靠 UA；WebView2 支持，浏览器预览也大多支持
  const uad = (
    navigator as unknown as {
      userAgentData?: { getHighEntropyValues?: (keys: string[]) => Promise<{ architecture?: string; bitness?: string }> };
    }
  ).userAgentData;
  if (uad?.getHighEntropyValues) {
    try {
      const info = await uad.getHighEntropyValues(["architecture", "bitness"]);
      const name = String(info.architecture ?? "");
      const bits = String(info.bitness ?? "");
      if (name === "arm") return bits === "64" ? "aarch64" : "arm";
      if (name === "x86") return bits === "64" ? "x86_64" : "x86";
    } catch {
      /* 落到 UA 判定 */
    }
  }
  const ua = navigator.userAgent;
  if (/aarch64|ARM64/.test(ua)) return "aarch64";
  if (/arm/i.test(ua)) return "arm";
  if (/Win64|x86_64|x86-64|WOW64/.test(ua)) return "x86_64";
  if (/Win32/i.test(ua)) return "x86";
  return null;
}

/** 各架构在各搜索源里的候选名（按优先级），匹配时去掉 - / _ 归一化 */
const ARCH_CANDIDATES: Record<Exclude<PlatformArch, null>, string[]> = {
  x86_64: ["x86_64", "x64", "amd64"],
  x86: ["x86", "x32", "i386"],
  aarch64: ["aarch64", "arm64"],
  arm: ["arm", "aarch32", "arm32"],
};
/** 各系统在候选列表里的名字：键是 detectOs() 的结果，值是各源写法 */
const OS_CANDIDATES: Record<string, string[]> = {
  windows: ["windows"],
  macos: ["macos", "mac"],
  linux: ["linux"],
};

/** 从候选列表里按优先级找本机平台对应的项（归一化后精确比对） */
function pickMatch(list: string[], candidates: string[]): string | null {
  const norm = (s: string) => s.toLowerCase().replace(/[-_]/g, "");
  for (const cand of candidates) {
    const hit = list.find((item) => norm(item) === norm(cand));
    if (hit !== undefined) return hit;
  }
  return null;
}

/** 拉下载源并加载其选项：首次挂载与切回本窗口都走这里，保留用户已选的源 */
async function loadSources() {
  if (!inTauri) return;
  try {
    const list = await commands.javaDownload.getTypes();
    sources.value = list;
    if (!source.value || !list.includes(source.value)) {
      source.value = list[0] ?? null;
    }
    if (source.value) await loadOptions();
  } catch (e) {
    showToast(tErr(e));
  }
}

onMounted(loadSources);

// 单窗口模式：窗口被 KeepAlive 缓存，切回不会重新挂载 → 自己补一次
useWindowRefresh(loadSources);

/** 拉当前源的四个下拉候选，默认选本机对应的系统与架构，再按默认筛选拉一次列表 */
async function loadOptions() {
  if (!source.value) return;
  optionsLoading.value = true;
  try {
    options.value = await commands.javaDownload.getOptions(source.value);
    // 类型 / 主版本选第一项；系统 / 架构自动选本机对应的项
    javaType.value = options.value.types[0] ?? "";
    major.value = options.value.majors[0] ?? null;
    const osCands = OS_CANDIDATES[detectOs()] ?? [];
    system.value =
      pickMatch(options.value.systems, osCands) ?? options.value.systems[0] ?? "";
    const archDetected = await detectArch();
    arch.value =
      pickMatch(options.value.archs, archDetected ? ARCH_CANDIDATES[archDetected] : []) ??
      options.value.archs[0] ??
      "";
  } catch (e) {
    showToast(tErr(e));
    // 候选没拉到就别按旧条件拉列表，否则列表与下拉显示的源对不上
    return;
  } finally {
    optionsLoading.value = false;
  }
  await loadList();
}

/** 正在下载的项（uuid），同一时间只允许一个 */
const starting = ref<string | null>(null);

/** 下载选中的包：后端按 uuid 取缓存里的下载信息 */
async function startDownload(uuid: string) {
  if (starting.value) return;
  starting.value = uuid;
  try {
    await commands.javaDownload.start(uuid);
  } catch (e) {
    showToast(tErr(e));
  } finally {
    starting.value = null;
  }
}

/** 字节数转可读大小 */
function formatSize(bytes: number): string {
  if (bytes >= 1024 ** 3) return `${(bytes / 1024 ** 3).toFixed(1)} GB`;
  if (bytes >= 1024 ** 2) return `${(bytes / 1024 ** 2).toFixed(1)} MB`;
  if (bytes >= 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${bytes} B`;
}

/** 按当前筛选条件拉取匹配的包列表 */
async function loadList() {
  if (!source.value || !javaType.value || major.value === null || !system.value || !arch.value) {
    // 筛选不全时清空，别留着上一个源的列表
    items.value = [];
    return;
  }
  listLoading.value = true;
  try {
    items.value = await commands.javaDownload.getList(
      source.value,
      javaType.value,
      major.value,
      system.value,
      arch.value,
    );
  } catch (e) {
    showToast(tErr(e));
  } finally {
    listLoading.value = false;
  }
}
</script>

<template>
  <WindowFrame :title="t('winJavaDownload.title')" @close="$emit('close')">
    <div class="java-download-body">
      <!-- 切换搜索源 / 加载选项或列表时弹窗提示，加载完自动消失 -->
      <BaseModal v-if="optionsLoading || listLoading" :width="300" :closable="false">
        <div class="loading-modal">
          <span class="loading-spin" />
          <span>{{ t("winJavaDownload.loadingTitle") }}</span>
        </div>
      </BaseModal>

      <div class="field-row">
        <div class="field">
          <label class="field-label">{{ t("winJavaDownload.source") }}</label>
          <select v-model="source" class="field-select" :disabled="optionsLoading" @change="loadOptions">
            <option v-for="s in sources" :key="s" :value="s">{{ s }}</option>
          </select>
        </div>

        <div class="field">
          <label class="field-label">{{ t("winJavaDownload.type") }}</label>
          <select v-model="javaType" class="field-select" :disabled="optionsLoading" @change="loadList">
            <option v-for="tp in options?.types ?? []" :key="tp" :value="tp">{{ tp }}</option>
          </select>
        </div>

        <div class="field">
          <label class="field-label">{{ t("winJavaDownload.major") }}</label>
          <select v-model="major" class="field-select" :disabled="optionsLoading" @change="loadList">
            <option v-for="m in options?.majors ?? []" :key="m" :value="m">{{ m }}</option>
          </select>
        </div>

        <div class="field">
          <label class="field-label">{{ t("winJavaDownload.system") }}</label>
          <select v-model="system" class="field-select" :disabled="optionsLoading" @change="loadList">
            <option v-for="s in options?.systems ?? []" :key="s" :value="s">{{ s }}</option>
          </select>
        </div>

        <div class="field">
          <label class="field-label">{{ t("winJavaDownload.arch") }}</label>
          <select v-model="arch" class="field-select" :disabled="optionsLoading" @change="loadList">
            <option v-for="a in options?.archs ?? []" :key="a" :value="a">{{ a }}</option>
          </select>
        </div>
      </div>

      <!-- 匹配的包列表：grid 文本卡片 -->
      <div v-if="!items.length && !listLoading" class="list-empty">
        {{ t("winJavaDownload.listEmpty") }}
      </div>
      <div class="item-grid">
        <div v-for="item in items" :key="item.uuid" class="item-card">
          <div class="item-head">
            <div class="item-title">{{ item.javaVersion }}</div>
            <BaseButton variant="accent" :disabled="starting !== null" @click="startDownload(item.uuid)">
              {{ t("winJavaDownload.download") }}
            </BaseButton>
          </div>
          <div class="item-sub">{{ item.name }}</div>
          <div class="item-sub">{{ item.filename }}</div>
          <div v-if="item.size > 0" class="item-sub">{{ formatSize(item.size) }}</div>
        </div>
      </div>
    </div>
  </WindowFrame>
</template>

<style scoped>
.java-download-body {
  display: flex;
  flex-direction: column;
  height: 100%;
  overflow-y: auto;
}

/* 加载弹窗内容：转圈 + 提示文字 */
.loading-modal {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 12px;
  color: var(--text-dim);
  font-size: 13px;
}

.loading-spin {
  width: 20px;
  height: 20px;
  border: 2.5px solid var(--border);
  border-top-color: var(--accent);
  border-radius: 50%;
  animation: loading-spin 0.8s linear infinite;
}

@keyframes loading-spin {
  to {
    transform: rotate(360deg);
  }
}

/* 五个下拉框一行排开，每列标签在上、选择器在下 */
.field-row {
  display: flex;
  gap: 10px;
}

.field {
  display: flex;
  flex-direction: column;
  flex: 1 1 0;
  min-width: 0;
}

.field-label {
  white-space: nowrap;
}

/* 匹配包的 grid 列表：占满剩余空间，卡片自适应列数 */
.item-grid {
  flex: 1;
  margin-top: 14px;
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(230px, 1fr));
  gap: 12px;
  align-content: start;
  overflow-y: auto;
  scrollbar-gutter: stable;
  /* 见 styles/scrollbar.css */
}

.item-card {
  border: 1px solid var(--border);
  border-radius: 10px;
  background: var(--bg-card);
  padding: 12px 14px;
  display: flex;
  flex-direction: column;
  gap: 6px;
  min-width: 0;
}

/* 卡片标题行：版本号 + 下载按钮 */
.item-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
}

.item-title {
  font-weight: 600;
  font-size: 15px;
}

.item-sub {
  color: var(--text-dim);
  font-size: 11.5px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

/* 没有匹配的 Java：紧贴筛选行下方，用告警色 */
.list-empty {
  margin-top: 4px;
  color: var(--red);
  font-size: 12.5px;
}
</style>
