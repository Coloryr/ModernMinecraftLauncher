<script setup lang="ts">
// 帮助窗口：启动器信息（右上角挂仓库 / 问题反馈两枚图标按钮）+ FAQ 手风琴 +
// 开源许可 + 第三方服务，文案取当前语言的 locale 包
//
// 启动器信息里的 exe SHA1 是打开窗口时现读可执行文件算出来的，见 src-tauri/src/build_info.rs。
// 开源许可只给**上游地址**：许可全文不随安装包分发，点按钮由系统浏览器打开
// （见 ./thirdParty.ts 的说明）。
import { computed, onMounted, ref } from "vue";
import WindowFrame from "../../components/ui/WindowFrame.vue";
import GlyphIcon from "../../components/ui/GlyphIcon.vue";
import { t, tErr, locale } from "../../lib/i18n";
import { api } from "../../lib/api";
import { ISSUES_URL, QQ_GROUP, REPO_URL } from "../../lib/links";
import { commands } from "../../lib/bindings";
import { copyText } from "../../lib/clipboard";
import { showToast } from "../../lib/toast";
import { appVersion, buildBadgeKey, buildChannel, isTestBuild, loadBuildInfo } from "../../lib/buildInfo";
// 与 src-tauri/icons 那套是同一个源（design/icon/f-blend.svg）；换图标时两处一起换
import appIcon from "../../assets/app-icon.svg";
import { faqs as zhFaqs } from "../../lib/i18n/locales/zh-CN";
import { faqs as enFaqs } from "../../lib/i18n/locales/en-US";
import {
  PROJECT_WORKS,
  RUST_LICENSE_COUNTS,
  RUST_MPL,
  RUST_TOTAL,
  SPDX_LICENSE_LIST,
  THIRD_PARTY_GROUPS,
  THIRD_PARTY_SERVICES,
  type LicenseCount,
  type ThirdPartyGroup,
  type ThirdPartyItem,
} from "./thirdParty";

// 必须显式声明 close：不声明的话 Vue 会把父级的 @close 当 attrs 透传到根组件（WindowFrame），
// 与模板里的 @close="$emit('close')" 合并成两个处理器 —— 一次返回会调两遍 closeWindow()，
// 第二遍时 currentKind 已经回到 main，于是"返回"变成退出应用
/** 请求关闭本窗口 */
defineEmits<{ (e: "close"): void }>();

// ---------------- 启动器信息 ----------------

/** 参与测试的人员，显示在「作者」下面；名字原样展示，不进 i18n */
const TESTERS = ["huige233"];

/** 测试人员一行的文本：中文用顿号、英文用逗号分隔 */
const testersText = computed(() => TESTERS.join(locale.value === "en_us" ? ", " : "、"));

/** 当前 exe 的 SHA1，小写十六进制；还没算完时为空串 */
const exeSha1 = ref("");
/** exe 的绝对路径，挂在 SHA1 上的悬浮提示 */
const exePath = ref("");
/** 读 exe 失败时的原因 */
const exeError = ref("");
/** exe 信息读取中：哈希要读完整份 exe，先显示「读取中」 */
const exeLoading = ref(true);

/** 读启动器 exe 的路径与 SHA1；失败只把原因写在这一栏，不影响帮助内容 */
async function loadExeInfo() {
  try {
    const info = await commands.buildInfo.exeInfo();
    exeSha1.value = info.sha1;
    exePath.value = info.path;
  } catch (e) {
    exeError.value = tErr(e);
  } finally {
    exeLoading.value = false;
  }
}

onMounted(() => {
  void loadBuildInfo();
  void loadExeInfo();
});

// ---------------- 启动器日志 ----------------
// 这里说的是启动器自己的 logs.log（panic、前端错误、下载与渲染记录），
// 不是日志窗口里那份实例运行日志 —— 后者整份上传，见 log_upload

/** 用系统默认程序打开日志文件；没有日志文件时后端回 err.fileNotFound */
async function openLog() {
  try {
    await api.openLauncherLog();
  } catch (e) {
    showToast(tErr(e));
  }
}

/** 上传中：避免连点 */
const uploading = ref(false);

/** 上传最后 1000 行并复制链接：体积太大才截，尾部就是出事那段 */
async function uploadLog() {
  if (uploading.value) return;
  uploading.value = true;
  try {
    const url = await api.uploadLauncherLog();
    await copyText(url);
    showToast(t("winHelp.uploadOk"));
  } catch (e) {
    showToast(tErr(e));
  } finally {
    uploading.value = false;
  }
}

/** 当前语言的 FAQ 列表，文案在 lib/i18n/locales 里 */
const faqs = computed(() => (locale.value === "en_us" ? enFaqs : zhFaqs));

/** 展开的 FAQ 下标；null 表示全部收起，默认展开第一条 */
const openIndex = ref<number | null>(0);

/**
 * 展开 / 收起一条 FAQ
 *
 * - `i`: FAQ 下标；再点一次同一条即收起
 */
function toggle(i: number) {
  openIndex.value = openIndex.value === i ? null : i;
}

/** 开源许可默认收起：帮助页以 FAQ 为主，许可清单是查证用的 */
const licenseOpen = ref(false);

/** 第三方服务与平台同样默认收起 */
const servicesOpen = ref(false);

/** 分组标题的 i18n 键，与 ThirdPartyGroup["id"] 一一对应 */
const GROUP_TITLE: Record<ThirdPartyGroup["id"], string> = {
  embedded: "winHelp.licenseGroupEmbedded",
  plugin: "winHelp.licenseGroupPlugin",
  frontend: "winHelp.licenseGroupFrontend",
};

/** 许可清单的一节 */
interface LicenseSection {
  /** 分节键，模板 v-for 用 */
  key: string;
  /** 分节标题，已 i18n */
  title: string;
  /** 标题下的说明行；没有时为空串 */
  lead: string;
  /** 许可分布，只有 Rust 那一节有；其余为空数组 */
  counts: LicenseCount[];
  /** 该节的组件 */
  items: ThirdPartyItem[];
}

/**
 * 许可清单分节：作者作品 → 各组第三方 → Rust 依赖
 *
 * 分节的标题与说明在这里拼好，模板只需按同一套结构渲染，不必为每节各写一份行模板。
 */
const licenseSections = computed<LicenseSection[]>(() => [
  { key: "own", title: t("winHelp.licenseOwn"), lead: "", counts: [], items: PROJECT_WORKS },
  ...THIRD_PARTY_GROUPS.map((group) => ({
    key: group.id,
    title: t(GROUP_TITLE[group.id]),
    lead: "",
    counts: [],
    items: group.items,
  })),
  {
    key: "rust",
    title: t("winHelp.licenseRust"),
    lead: t("winHelp.licenseRustTotal", { total: RUST_TOTAL }),
    counts: RUST_LICENSE_COUNTS,
    items: RUST_MPL,
  },
]);

/** 用系统浏览器打开上游地址；打不开就静默，许可清单不该为一次跳转弹提示 */
function openUrl(url: string) {
  api.openUrl(url).catch(() => { });
}

/** 复制 QQ 群号 */
async function copyQq() {
  showToast((await copyText(QQ_GROUP)) ? t("winHelp.qqCopied") : t("account.copyFail"));
}
</script>

<template>
  <WindowFrame enter=":scope > *" :title="t('features.help')" body-gutter @close="$emit('close')">
    <!-- 启动器信息：图标、名称、版本、作者与这份 exe 的 SHA1，放在 FAQ 之前 -->
    <section class="about mb-3.5 flex gap-3.5 rounded-panel border border-border bg-card px-5 py-4">
      <img class="about-icon h-14 w-14 shrink-0 rounded-panel" :src="appIcon" alt="" />
      <div class="about-body flex min-w-0 flex-1 flex-col gap-1">
        <div class="about-title flex items-center gap-1.5 text-heading font-bold">
          {{ t("app.name") }}
          <span v-if="isTestBuild" class="about-badge rounded-pill border border-current px-1.5 py-0.5 text-caption font-semibold" :class="`is-${buildChannel}`">
            {{ t(buildBadgeKey) }}
          </span>
        </div>
        <p class="about-sub mb-1.5 text-body text-dim">{{ t("app.sub") }}</p>
        <div class="about-row flex items-baseline gap-2 text-body">
          <span class="about-label min-w-[84px] shrink-0 text-meta text-dim">{{ t("winHelp.version") }}</span>
          <span class="about-value text-fg">{{ appVersion || "—" }}</span>
        </div>
        <div class="about-row flex items-baseline gap-2 text-body">
          <span class="about-label min-w-[84px] shrink-0 text-meta text-dim">{{ t("winHelp.author") }}</span>
          <span class="about-value text-fg">{{ t("winHelp.authorName") }}</span>
        </div>
        <div class="about-row flex items-baseline gap-2 text-body">
          <span class="about-label min-w-[84px] shrink-0 text-meta text-dim">{{ t("winHelp.testers") }}</span>
          <span class="about-value text-fg">{{ testersText }}</span>
        </div>
        <div class="about-row flex items-baseline gap-2 text-body">
          <span class="about-label min-w-[84px] shrink-0 text-meta text-dim">{{ t("winHelp.exeSha1") }}</span>
          <code class="about-sha1 text-meta text-fg break-all [font-family:'Cascadia Code',Consolas,monospace]" :class="{ 'is-error': !!exeError }" :title="exePath">
            <template v-if="exeSha1">{{ exeSha1 }}</template>
<template v-else-if="exeLoading">{{ t("winHelp.sha1Reading") }}</template>
<template v-else>{{ exeError || "—" }}</template>
</code>
        </div>
      </div>

      <!-- 打开日志 / 上传日志 / 仓库 / 问题反馈：跟介绍信息同一张卡，贴右上角，正文行数不用再挪 -->
      <div class="about-actions -mr-2 flex gap-1 self-start">
        <!-- 反馈问题前先看日志：打开的是启动器自己的 logs.log，不是实例运行日志 -->
        <button class="ui-btn-icon" v-tip="t('winHelp.openLog')" :aria-label="t('winHelp.openLog')"
          @click="openLog">
          <GlyphIcon name="log" :size="16" />
        </button>
        <!-- 日志太大贴不进 issue：传最后 1000 行到 mclo.gs，链接直接进剪贴板 -->
        <button class="ui-btn-icon" :disabled="uploading"
          v-tip="uploading ? t('winHelp.uploading') : t('winHelp.uploadLog')"
          :aria-label="t('winHelp.uploadLog')" @click="uploadLog">
          <GlyphIcon name="export" :size="16" />
        </button>
        <button class="ui-btn-icon" v-tip="t('winHelp.repo')" :aria-label="t('winHelp.repo')"
          @click="openUrl(REPO_URL)">
          <GlyphIcon name="github" :size="16" />
        </button>
        <button class="ui-btn-icon" v-tip="t('winHelp.feedback')" :aria-label="t('winHelp.feedback')"
          @click="openUrl(ISSUES_URL)">
          <GlyphIcon name="bug" :size="16" />
        </button>
      </div>
    </section>

    <div class="help-list flex flex-col gap-2.5">
      <div v-for="(item, i) in faqs" :key="item.q" class="faq-item overflow-hidden rounded-panel border border-border bg-card">
        <button class="faq-q flex w-full cursor-pointer items-center justify-between gap-3 border-0 bg-transparent px-5 py-3.5 text-left text-title font-semibold text-fg [font-family:inherit] hover:bg-hover" @click="toggle(i)">
          <span>{{ item.q }}</span>
          <svg class="shrink-0 text-dim transition-transform duration-150" viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2"
            :style="{ transform: openIndex === i ? 'rotate(180deg)' : 'none' }">
            <path d="m6 9 6 6 6-6" />
          </svg>
        </button>
        <div v-if="openIndex === i" class="faq-a px-5 pb-4 pt-1 text-title leading-[1.7] text-dim">{{ item.a }}</div>
      </div>
    </div>

    <div class="footer-note mt-[22px] rounded-panel border border-border bg-card px-5 py-4 text-body leading-[1.8] text-dim">
      <p>{{ t("winHelp.stillStuck") }}</p>
      <p>{{ t("winHelp.report") }}</p>
      <span class="footer-qq mt-2.5 inline-flex items-center gap-1.5 text-body text-dim">
        {{ t("winHelp.qqGroup", { n: QQ_GROUP }) }}
        <button class="qq-copy h-ctl cursor-pointer rounded-ctl border border-border bg-raised px-2.5 text-meta text-accent [font-family:inherit] transition-[border-color,background] duration-[0.12s] hover:border-accent-border hover:bg-accent-soft" v-tip="t('winHelp.qqCopy')" :aria-label="t('winHelp.qqCopy')" @click="copyQq">
          {{ t("winHelp.qqCopy") }}
        </button>
      </span>
    </div>

    <!-- 开源许可：只给上游链接，许可全文不随安装包分发，见 ./thirdParty.ts -->
    <div class="lic-block mt-[22px] overflow-hidden rounded-panel border border-border bg-card">
      <button class="faq-q flex w-full cursor-pointer items-center justify-between gap-3 border-0 bg-transparent px-5 py-3.5 text-left text-title font-semibold text-fg [font-family:inherit] hover:bg-hover" @click="licenseOpen = !licenseOpen">
        <span>{{ t("winHelp.license") }}</span>
        <svg class="shrink-0 text-dim transition-transform duration-150" viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2"
          :style="{ transform: licenseOpen ? 'rotate(180deg)' : 'none' }">
          <path d="m6 9 6 6 6-6" />
        </svg>
      </button>

      <div v-if="licenseOpen" class="lic-body px-5 pb-4 pt-1 text-title leading-[1.7] text-dim">
        <p class="lic-lead mb-2.5">{{ t("winHelp.licenseIntro") }}</p>

        <section v-for="sec in licenseSections" :key="sec.key" class="lic-sec">
          <p class="lic-sub mb-1.5 text-title font-semibold text-fg">{{ sec.title }}</p>
          <p v-if="sec.lead" class="lic-text mb-2 text-body">{{ sec.lead }}</p>
          <div v-if="sec.counts.length" class="lic-counts mb-2 flex flex-wrap gap-x-3.5 gap-y-1 text-meta">
            <span v-for="row in sec.counts" :key="row.license || row.key">
              {{ row.key ? t(row.key) : row.license }} · {{ row.count }}
            </span>
          </div>
          <div v-for="item in sec.items" :key="item.name" class="lic-row flex items-center gap-2.5 py-1.5">
            <div class="lic-info flex min-w-0 flex-1 flex-wrap items-baseline gap-x-2 gap-y-0.5">
              <span class="lic-name text-body font-semibold text-fg">{{ item.name }}</span>
              <span class="lic-meta text-meta">{{ item.version }} · {{ item.license }}</span>
              <span v-if="item.note" class="lic-note basis-full text-meta">{{ t(item.note) }}</span>
            </div>
            <button v-if="item.url" class="lic-open shrink-0 cursor-pointer rounded-ctl border border-border bg-transparent px-2.5 py-[3px] text-meta text-dim [font-family:inherit] transition-[background,color] duration-150 hover:bg-hover hover:text-fg" @click="openUrl(item.url)">
              {{ t("winHelp.licenseUpstream") }}
            </button>
          </div>
        </section>

        <button class="lic-open shrink-0 cursor-pointer rounded-ctl border border-border bg-transparent px-2.5 py-[3px] text-meta text-dim [font-family:inherit] transition-[background,color] duration-150 hover:bg-hover hover:text-fg lic-spdx mx-auto mt-3.5 block" @click="openUrl(SPDX_LICENSE_LIST)">
          {{ t("winHelp.licenseSpdx") }}
        </button>
      </div>
    </div>

    <!-- 第三方服务与平台：本项目只是调用方，许可那一档放不下这类内容 -->
    <div class="lic-block mt-[22px] overflow-hidden rounded-panel border border-border bg-card">
      <button class="faq-q flex w-full cursor-pointer items-center justify-between gap-3 border-0 bg-transparent px-5 py-3.5 text-left text-title font-semibold text-fg [font-family:inherit] hover:bg-hover" @click="servicesOpen = !servicesOpen">
        <span>{{ t("winHelp.services") }}</span>
        <svg class="shrink-0 text-dim transition-transform duration-150" viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2"
          :style="{ transform: servicesOpen ? 'rotate(180deg)' : 'none' }">
          <path d="m6 9 6 6 6-6" />
        </svg>
      </button>

      <div v-if="servicesOpen" class="lic-body px-5 pb-4 pt-1 text-title leading-[1.7] text-dim">
        <p class="lic-lead mb-2.5">{{ t("winHelp.servicesIntro") }}</p>

        <div v-for="srv in THIRD_PARTY_SERVICES" :key="srv.name" class="lic-row flex items-center gap-2.5 py-1.5">
          <div class="lic-info flex min-w-0 flex-1 flex-wrap items-baseline gap-x-2 gap-y-0.5">
            <span class="lic-name text-body font-semibold text-fg">{{ srv.name }}</span>
            <span class="lic-note basis-full text-meta">{{ t(srv.purpose) }}</span>
          </div>
          <button v-if="srv.url" class="lic-open shrink-0 cursor-pointer rounded-ctl border border-border bg-transparent px-2.5 py-[3px] text-meta text-dim [font-family:inherit] transition-[background,color] duration-150 hover:bg-hover hover:text-fg" @click="openUrl(srv.url)">
            {{ t("winHelp.serviceOpen") }}
          </button>
          <button v-for="link in srv.links" :key="link.url" class="lic-open shrink-0 cursor-pointer rounded-ctl border border-border bg-transparent px-2.5 py-[3px] text-meta text-dim [font-family:inherit] transition-[background,color] duration-150 hover:bg-hover hover:text-fg" @click="openUrl(link.url)">
            {{ t(link.label) }}
          </button>
        </div>
      </div>
    </div>
  </WindowFrame>
</template>

<style scoped>
/* 排版 / 间距 / 字号 / 颜色都在模板的工具类上；这里只留状态色与兄弟组合 ——
   前者按语义类名做主题覆盖，后者用工具类表达不了相邻关系 */
.about-badge.is-debug {
  color: var(--accent);
}

.about-badge.is-prerelease {
  color: var(--yellow);
}

.about-sha1.is-error {
  color: var(--red);
}

/* 许可清单相邻行之间的分隔线 */
.lic-row + .lic-row {
  border-top: 1px solid var(--border);
}

.lic-sec + .lic-sec {
  margin-top: 16px;
}
</style>
