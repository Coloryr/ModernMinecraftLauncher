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
import { useWindowRefresh } from "../../composables/useWindowRefresh";
import { t, tErr } from "../../lib/i18n";
import { showToast } from "../../lib/toast";
import {
  commands,
  type JavaDownloadItemDto,
  type JavaDownloadOptionsDto,
  type JavaTypes,
} from "../../lib/bindings";
import { isTauri, javaMajorFromUrl, multiWindow, windowParams } from "../windowManager";
import { detectOs } from "../../lib/titlebar";
import { isRunning, javaTasks } from "../../lib/javaTasks";

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

/** 搜索源的显示名 */
const SOURCE_KEYS: Record<JavaTypes, string> = {
  adoptium: "winJavaDownload.sourceAdoptium",
  zulu: "winJavaDownload.sourceZulu",
  openj9: "winJavaDownload.sourceOpenJ9",
  foojay: "winJavaDownload.sourceFoojay",
};

/** 各源系统写法的显示名，认不出的原样显示 */
const SYSTEM_KEYS: Record<string, string> = {
  windows: "winJavaDownload.systemWindows",
  linux: "winJavaDownload.systemLinux",
  linux_musl: "winJavaDownload.systemLinuxMusl",
  mac: "winJavaDownload.systemMac",
  macos: "winJavaDownload.systemMac",
  "alpine-linux": "winJavaDownload.systemAlpine",
  aix: "winJavaDownload.systemAix",
  solaris: "winJavaDownload.systemSolaris",
};

/**
 * 搜索源显示名
 *
 * - `source`: 后端的搜索源枚举值
 */
function sourceLabel(source: JavaTypes): string {
  return t(SOURCE_KEYS[source]);
}

/**
 * 系统显示名
 *
 * - `system`: 各源给的原始系统名，大小写不敏感
 */
function systemLabel(system: string): string {
  const key = SYSTEM_KEYS[system.toLowerCase()];
  return key ? t(key) : system;
}

/**
 * 发行类型显示名，JDK / JRE 这类缩写只统一大写
 *
 * - `javaType`: 后端的发行类型
 */
function typeLabel(javaType: string): string {
  return javaType.toUpperCase();
}

/** 从候选列表里按优先级找本机平台对应的项（归一化后精确比对） */
function pickMatch(list: string[], candidates: string[]): string | null {
  const norm = (s: string) => s.toLowerCase().replace(/[-_]/g, "");
  for (const cand of candidates) {
    const hit = list.find((item) => norm(item) === norm(cand));
    if (hit !== undefined) return hit;
  }
  return null;
}

/** 刷新代次：F5 连续触发时，旧的一发不能再改源、也不能再启动选项请求 */
let sourcesSeq = 0;

/** 换搜索源：先清掉上一源的卡片，再拉新候选 */
async function onSourceChange() {
  items.value = [];
  await loadOptions(true);
}

/** 拉下载源并加载其选项：首次挂载与切回本窗口都走这里，保留用户已选的源 */
async function loadSources() {
  const mine = ++sourcesSeq;
  if (!inTauri) return;
  try {
    const list = await commands.javaDownload.getTypes();
    if (mine !== sourcesSeq) return;
    sources.value = list;
    if (!source.value || !list.includes(source.value)) {
      source.value = list[0] ?? null;
    }
    if (source.value) await loadOptions();
  } catch (e) {
    if (mine !== sourcesSeq) return;
    showToast(tErr(e));
  }
}

// 单窗口模式：窗口被 KeepAlive 缓存，切回不会重新挂载 → 自己补一次
onMounted(loadSources);

// 单窗口模式：窗口被 KeepAlive 缓存，切回不会重新挂载 → 自己补一次
useWindowRefresh(loadSources);

/** 候选请求代次：F5 / 连续切源只让最新一发写回下拉并继续拉列表 */
let optionsSeq = 0;
/** 四个筛选是否已经铺过默认值；刷新时保留用户选择 */
let optionsReady = false;

/**
 * 本次打开要预选的 Java 主版本
 *
 * 启动缺 Java 时从提示弹窗带过来；单窗口模式下读完就清掉，避免之后刷新筛选又跳回去。
 *
 * # 返回值
 *
 * 目标主版本；不是从缺 Java 提示打开时为 null
 */
function wantedJavaMajor(): number | null {
  const wanted = multiWindow.value ? javaMajorFromUrl() : windowParams.value.javaMajor;
  if (!multiWindow.value && windowParams.value.javaMajor !== null) {
    windowParams.value = { ...windowParams.value, javaMajor: null };
  }
  return wanted;
}

/** 拉当前源的四个下拉候选，默认选本机对应的系统与架构，再按默认筛选拉一次列表 */
async function loadOptions(reset = false) {
  const mine = ++optionsSeq;
  if (!source.value) {
    optionsLoading.value = false;
    return;
  }
  optionsLoading.value = true;
  try {
    const data = await commands.javaDownload.getOptions(source.value);
    if (mine !== optionsSeq) return;
    options.value = data;
    const keep = optionsReady && !reset;
    // 类型：刷新时保留仍存在的选择，否则落第一项
    javaType.value = keep && data.types.includes(javaType.value) ? javaType.value : data.types[0] ?? "";
    const wanted = wantedJavaMajor();
    // 主版本：启动缺 Java 时带过来的优先；刷新时保留仍在候选里的值
    major.value =
      wanted !== null && data.majors.includes(wanted)
        ? wanted
        : keep && major.value !== null && data.majors.includes(major.value)
          ? major.value
          : data.majors[0] ?? null;
    if (wanted !== null && !data.majors.includes(wanted)) {
      showToast(t("winJavaDownload.majorMissing", { java: wanted }));
    }
    const osCands = OS_CANDIDATES[detectOs()] ?? [];
    system.value = keep && data.systems.includes(system.value)
      ? system.value
      : pickMatch(data.systems, osCands) ?? data.systems[0] ?? "";
    const archDetected = await detectArch();
    if (mine !== optionsSeq) return;
    arch.value = keep && data.archs.includes(arch.value)
      ? arch.value
      : pickMatch(data.archs, archDetected ? ARCH_CANDIDATES[archDetected] : []) ??
        // 探不出本机架构时优先落到常见桌面架构，别默认选到 ARM
        pickMatch(data.archs, ["x64", "x86_64", "amd64"]) ??
        data.archs[0] ??
        "";
    optionsReady = true;
  } catch (e) {
    if (mine !== optionsSeq) return;
    showToast(tErr(e));
    // 候选没拉到就别按旧条件拉列表，否则列表与下拉显示的源对不上
    items.value = [];
    return;
  } finally {
    if (mine === optionsSeq) optionsLoading.value = false;
  }
  if (mine !== optionsSeq) return;
  await loadList();
}

/**
 * 这一项是不是正在装
 *
 * id 由下载地址派生，重新拉列表也不会变，所以按 id 比对即可。
 *
 * - `id`: 列表项的稳定 id
 */
function isInstalling(id: string): boolean {
  return javaTasks.value.some((task) => task.id === id && isRunning(task));
}

/** 下载并注册选中的包：后端按 uuid 取缓存里的下载信息 */
async function startDownload(item: JavaDownloadItemDto) {
  if (isInstalling(item.uuid)) return;
  try {
    // 命令只负责登记任务，下载与解压都在后台跑，进度看标题栏的 Java 指示器
    await commands.javaDownload.start(item.uuid);
  } catch (e) {
    showToast(tErr(e));
  }
}

/** 字节数转可读大小 */
function formatSize(bytes: number): string {
  if (bytes >= 1024 ** 3) return `${(bytes / 1024 ** 3).toFixed(1)} GB`;
  if (bytes >= 1024 ** 2) return `${(bytes / 1024 ** 2).toFixed(1)} MB`;
  if (bytes >= 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${bytes} B`;
}

/** 列表请求代次：F5 / 连续改筛选时，只让最新一发写回结果 */
let listSeq = 0;

/** 按当前筛选条件拉取匹配的包列表 */
async function loadList() {
  const mine = ++listSeq;
  if (!source.value || !javaType.value || major.value === null || !system.value || !arch.value) {
    // 筛选不全时清空，别留着上一个源的列表；在途的旧请求也一并作废
    items.value = [];
    listLoading.value = false;
    return;
  }
  listLoading.value = true;
  try {
    const list = await commands.javaDownload.getList(
      source.value,
      javaType.value,
      major.value,
      system.value,
      arch.value,
    );
    if (mine !== listSeq) return;
    items.value = list;
  } catch (e) {
    if (mine !== listSeq) return;
    // 失败时清空，别让旧结果继续冒充当前筛选条件
    items.value = [];
    showToast(tErr(e));
  } finally {
    if (mine === listSeq) listLoading.value = false;
  }
}
</script>

<template>
  <WindowFrame enter=":scope > *" :title="t('winJavaDownload.title')" @close="$emit('close')">
    <div class="java-download-body flex h-full flex-col overflow-y-auto">
      <div class="field-row flex flex-wrap gap-2.5">
        <div class="field flex min-w-0 flex-[1_1_110px] flex-col">
          <label class="field-label whitespace-nowrap">{{ t("winJavaDownload.source") }}</label>
          <select v-model="source" class="field-select" :disabled="optionsLoading" @change="onSourceChange">
            <option v-for="s in sources" :key="s" :value="s">{{ sourceLabel(s) }}</option>
          </select>
        </div>

        <div class="field flex min-w-0 flex-[1_1_110px] flex-col">
          <label class="field-label whitespace-nowrap">{{ t("winJavaDownload.type") }}</label>
          <select v-model="javaType" class="field-select" :disabled="optionsLoading" @change="loadList">
            <option v-for="tp in options?.types ?? []" :key="tp" :value="tp">{{ typeLabel(tp) }}</option>
          </select>
        </div>

        <div class="field flex min-w-0 flex-[1_1_110px] flex-col">
          <label class="field-label whitespace-nowrap">{{ t("winJavaDownload.major") }}</label>
          <select v-model="major" class="field-select" :disabled="optionsLoading" @change="loadList">
            <option v-for="m in options?.majors ?? []" :key="m" :value="m">{{ m }}</option>
          </select>
        </div>

        <div class="field flex min-w-0 flex-[1_1_110px] flex-col">
          <label class="field-label whitespace-nowrap">{{ t("winJavaDownload.system") }}</label>
          <select v-model="system" class="field-select" :disabled="optionsLoading" @change="loadList">
            <option v-for="s in options?.systems ?? []" :key="s" :value="s">{{ systemLabel(s) }}</option>
          </select>
        </div>

        <div class="field flex min-w-0 flex-[1_1_110px] flex-col">
          <label class="field-label whitespace-nowrap">{{ t("winJavaDownload.arch") }}</label>
          <select v-model="arch" class="field-select" :disabled="optionsLoading" @change="loadList">
            <option v-for="a in options?.archs ?? []" :key="a" :value="a">{{ a }}</option>
          </select>
        </div>
      </div>

      <!-- 加载提示放行内：改筛选时不该弹一层面板盖住列表 -->
      <div v-if="optionsLoading || listLoading" class="list-loading mt-3 flex items-center gap-2 text-body text-dim">
        <span class="loading-spin" />
        <span>{{ t("winJavaDownload.loadingTitle") }}</span>
      </div>

      <!-- 匹配的包列表：grid 文本卡片 -->
      <div v-if="!items.length && !listLoading && !optionsLoading" class="list-empty mt-1 text-body text-red">
        {{ t("winJavaDownload.listEmpty") }}
      </div>
      <div class="item-grid mt-3.5 grid flex-1 content-start gap-3 overflow-y-scroll [scrollbar-gutter:stable] grid-cols-[repeat(auto-fill,minmax(230px,1fr))]">
        <div v-for="item in items" :key="item.uuid"
          class="item-card flex min-w-0 flex-col gap-1.5 rounded-card border border-border bg-card px-3.5 py-3">
          <div class="item-head flex items-center justify-between gap-2">
            <div class="item-title text-heading font-semibold">{{ item.javaVersion }}</div>
            <BaseButton variant="accent" :disabled="isInstalling(item.uuid)"
              @click="startDownload(item)">
              {{ isInstalling(item.uuid) ? t("winJavaDownload.downloading") : t("winJavaDownload.download") }}
            </BaseButton>
          </div>
          <div class="item-sub truncate text-meta text-dim">{{ item.name }}</div>
          <div class="item-sub truncate text-meta text-dim">{{ item.filename }}</div>
          <div v-if="item.size > 0" class="item-sub truncate text-meta text-dim">{{ formatSize(item.size) }}</div>
        </div>
      </div>
    </div>
  </WindowFrame>
</template>

<style scoped>
/* 排版都改到模板上的工具类了；这里只剩转圈本体（边框 + 动画），
   工具类表达不了 keyframes。 */
.loading-spin {
  width: 14px;
  height: 14px;
  border: 2px solid var(--border);
  border-top-color: var(--accent);
  border-radius: 50%;
  animation: loading-spin 0.8s linear infinite;
}

@keyframes loading-spin {
  to {
    transform: rotate(360deg);
  }
}

</style>
