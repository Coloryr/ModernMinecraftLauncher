<script setup lang="ts">
// 帮助窗口：启动器信息 + FAQ 手风琴 + 开源许可 + 第三方服务 + 问题反馈提示，
// 文案取当前语言的 locale 包
//
// 启动器信息里的 exe SHA1 是打开窗口时现读可执行文件算出来的，见 src-tauri/src/build_info.rs。
// 开源许可只给**上游地址**：许可全文不随安装包分发，点按钮由系统浏览器打开
// （见 ./thirdParty.ts 的说明）。
import { computed, onMounted, ref } from "vue";
import WindowFrame from "../../components/ui/WindowFrame.vue";
import BaseButton from "../../components/ui/BaseButton.vue";
import { t, tErr, locale } from "../../lib/i18n";
import { api } from "../../lib/api";
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
  api.openUrl(url).catch(() => {});
}

/** 项目仓库与问题反馈地址 */
const REPO_URL = "https://github.com/Coloryr/ModernMinecraftLauncher";
const ISSUES_URL = `${REPO_URL}/issues`;
/** QQ 交流群号 */
const QQ_GROUP = "571239090";

/** 复制 QQ 群号 */
async function copyQq() {
  showToast((await copyText(QQ_GROUP)) ? t("winHelp.qqCopied") : t("account.copyFail"));
}
</script>

<template>
  <WindowFrame :title="t('features.help')" body-gutter @close="$emit('close')">
    <!-- 启动器信息：图标、名称、版本、作者与这份 exe 的 SHA1，放在 FAQ 之前 -->
    <section class="about">
      <img class="about-icon" :src="appIcon" alt="" />
      <div class="about-body">
        <div class="about-title">
          {{ t("app.name") }}
          <span v-if="isTestBuild" class="about-badge" :class="`is-${buildChannel}`">
            {{ t(buildBadgeKey) }}
          </span>
        </div>
        <p class="about-sub">{{ t("app.sub") }}</p>
        <div class="about-row">
          <span class="about-label">{{ t("winHelp.version") }}</span>
          <span class="about-value">{{ appVersion || "—" }}</span>
        </div>
        <div class="about-row">
          <span class="about-label">{{ t("winHelp.author") }}</span>
          <span class="about-value">{{ t("winHelp.authorName") }}</span>
        </div>
        <div class="about-row">
          <span class="about-label">{{ t("winHelp.exeSha1") }}</span>
          <code class="about-sha1" :class="{ 'is-error': !!exeError }" :title="exePath">
            <template v-if="exeSha1">{{ exeSha1 }}</template>
            <template v-else-if="exeLoading">{{ t("winHelp.sha1Reading") }}</template>
            <template v-else>{{ exeError || "—" }}</template>
          </code>
        </div>
      </div>
    </section>

    <div class="help-list">
      <div v-for="(item, i) in faqs" :key="i" class="faq-item">
        <button class="faq-q" @click="toggle(i)">
          <span>{{ item.q }}</span>
          <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2"
            :style="{ transform: openIndex === i ? 'rotate(180deg)' : 'none' }">
            <path d="m6 9 6 6 6-6" />
          </svg>
        </button>
        <div v-if="openIndex === i" class="faq-a">{{ item.a }}</div>
      </div>
    </div>

    <div class="footer-note">
      <p>{{ t("winHelp.stillStuck") }}</p>
      <p>{{ t("winHelp.report") }}</p>
      <div class="footer-actions">
        <BaseButton size="sm" @click="openUrl(REPO_URL)">{{ t("winHelp.repo") }}</BaseButton>
        <BaseButton size="sm" @click="openUrl(ISSUES_URL)">{{ t("winHelp.feedback") }}</BaseButton>
        <span class="footer-qq">
          {{ t("winHelp.qqGroup", { n: QQ_GROUP }) }}
          <button class="qq-copy" v-tip="t('winHelp.qqCopy')" :aria-label="t('winHelp.qqCopy')"
            @click="copyQq">
            {{ t("winHelp.qqCopy") }}
          </button>
        </span>
      </div>
    </div>

    <!-- 开源许可：只给上游链接，许可全文不随安装包分发，见 ./thirdParty.ts -->
    <div class="lic-block">
      <button class="faq-q" @click="licenseOpen = !licenseOpen">
        <span>{{ t("winHelp.license") }}</span>
        <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2"
          :style="{ transform: licenseOpen ? 'rotate(180deg)' : 'none' }">
          <path d="m6 9 6 6 6-6" />
        </svg>
      </button>

      <div v-if="licenseOpen" class="lic-body">
        <p class="lic-lead">{{ t("winHelp.licenseIntro") }}</p>

        <section v-for="sec in licenseSections" :key="sec.key" class="lic-sec">
          <p class="lic-sub">{{ sec.title }}</p>
          <p v-if="sec.lead" class="lic-text">{{ sec.lead }}</p>
          <div v-if="sec.counts.length" class="lic-counts">
            <span v-for="row in sec.counts" :key="row.license || row.key">
              {{ row.key ? t(row.key) : row.license }} · {{ row.count }}
            </span>
          </div>
          <div v-for="item in sec.items" :key="item.name" class="lic-row">
            <div class="lic-info">
              <span class="lic-name">{{ item.name }}</span>
              <span class="lic-meta">{{ item.version }} · {{ item.license }}</span>
              <span v-if="item.note" class="lic-note">{{ t(item.note) }}</span>
            </div>
            <button v-if="item.url" class="lic-open" @click="openUrl(item.url)">
              {{ t("winHelp.licenseUpstream") }}
            </button>
          </div>
        </section>

        <button class="lic-open lic-spdx" @click="openUrl(SPDX_LICENSE_LIST)">
          {{ t("winHelp.licenseSpdx") }}
        </button>
      </div>
    </div>

    <!-- 第三方服务与平台：本项目只是调用方，许可那一档放不下这类内容 -->
    <div class="lic-block">
      <button class="faq-q" @click="servicesOpen = !servicesOpen">
        <span>{{ t("winHelp.services") }}</span>
        <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2"
          :style="{ transform: servicesOpen ? 'rotate(180deg)' : 'none' }">
          <path d="m6 9 6 6 6-6" />
        </svg>
      </button>

      <div v-if="servicesOpen" class="lic-body">
        <p class="lic-lead">{{ t("winHelp.servicesIntro") }}</p>

        <div v-for="srv in THIRD_PARTY_SERVICES" :key="srv.name" class="lic-row">
          <div class="lic-info">
            <span class="lic-name">{{ srv.name }}</span>
            <span class="lic-note">{{ t(srv.purpose) }}</span>
          </div>
          <button v-if="srv.url" class="lic-open" @click="openUrl(srv.url)">
            {{ t("winHelp.serviceOpen") }}
          </button>
          <button v-for="link in srv.links" :key="link.url" class="lic-open" @click="openUrl(link.url)">
            {{ t(link.label) }}
          </button>
        </div>
      </div>
    </div>
  </WindowFrame>
</template>

<style scoped>
/* ---------- 启动器信息 ---------- */

.about {
  display: flex;
  gap: 14px;
  margin-bottom: 14px;
  padding: 16px 20px;
  background: var(--bg-card);
  border: 1px solid var(--border);
  border-radius: 12px;
}

.about-icon {
  width: 56px;
  height: 56px;
  flex-shrink: 0;
  border-radius: 12px;
}

.about-body {
  min-width: 0;
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.about-title {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 15px;
  font-weight: 700;
}

/* 构建通道徽章：与顶栏那一枚同口径，测试版用强调色、预发布版用黄色 */
.about-badge {
  padding: 2px 6px;
  border: 1px solid currentColor;
  border-radius: 999px;
  font-size: 10.5px;
  font-weight: 600;
}

.about-badge.is-debug {
  color: var(--accent);
}

.about-badge.is-prerelease {
  color: var(--yellow);
}

.about-sub {
  margin: 0 0 6px;
  font-size: 12.5px;
  color: var(--text-dim);
}

.about-row {
  display: flex;
  align-items: baseline;
  gap: 8px;
  font-size: 12.5px;
}

.about-label {
  flex-shrink: 0;
  min-width: 84px;
  font-size: 11.5px;
  color: var(--text-dim);
}

.about-value {
  color: var(--text);
}

/* SHA1 有 40 位：窄窗口下任意位置折行，别把内容区撑出横向滚动 */
.about-sha1 {
  font-family: "Cascadia Code", Consolas, monospace;
  font-size: 11.5px;
  color: var(--text);
  word-break: break-all;
}

.about-sha1.is-error {
  color: var(--red);
}

.help-list {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.faq-item {
  background: var(--bg-card);
  border: 1px solid var(--border);
  border-radius: 12px;
  overflow: hidden;
}

.faq-q {
  width: 100%;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 14px 20px;
  border: none;
  background: transparent;
  color: var(--text);
  font-size: 13px;
  font-weight: 600;
  font-family: inherit;
  cursor: pointer;
  text-align: left;
}

.faq-q svg {
  color: var(--text-dim);
  transition: transform 0.15s;
  flex-shrink: 0;
}

.faq-q:hover {
  background: var(--bg-hover);
}

.faq-a {
  padding: 4px 20px 16px;
  font-size: 13px;
  color: var(--text-dim);
  line-height: 1.7;
}

.footer-note {
  margin-top: 22px;
  padding: 16px 20px;
  background: var(--bg-card);
  border: 1px solid var(--border);
  border-radius: 12px;
  font-size: 12.5px;
  color: var(--text-dim);
  line-height: 1.8;
}

.footer-actions {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 8px;
  margin-top: 10px;
}

.footer-qq {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  font-size: 12.5px;
  color: var(--text-dim);
}

.qq-copy {
  height: var(--ctl-h);
  padding: 0 10px;
  border: 1px solid var(--border);
  border-radius: var(--ctl-radius);
  background: var(--bg-raised);
  color: var(--accent);
  font-size: 12px;
  font-family: inherit;
  cursor: pointer;
  transition: border-color 0.12s, background 0.12s;
}

.qq-copy:hover {
  border-color: var(--accent);
  background: var(--accent-soft);
}

/* ---------- 开源许可 ---------- */

/* 与 FAQ 卡片同一套外观：它也是"标题行可点开的一段内容" */
.lic-block {
  margin-top: 22px;
  background: var(--bg-card);
  border: 1px solid var(--border);
  border-radius: 12px;
  overflow: hidden;
}

.lic-body {
  padding: 4px 20px 16px;
  font-size: 13px;
  color: var(--text-dim);
  line-height: 1.7;
}

.lic-lead {
  margin: 0 0 10px;
}

.lic-sec + .lic-sec {
  margin-top: 16px;
}

.lic-sub {
  margin: 0 0 6px;
  font-size: 13px;
  font-weight: 600;
  color: var(--text);
}

.lic-text {
  margin: 0 0 8px;
  font-size: 12.5px;
}

/* 许可分布：窄窗口下自己换行，不横向滚 */
.lic-counts {
  display: flex;
  flex-wrap: wrap;
  gap: 4px 14px;
  margin-bottom: 8px;
  font-size: 11.5px;
}

.lic-row {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 6px 0;
}

.lic-row + .lic-row {
  border-top: 1px solid var(--border);
}

.lic-info {
  display: flex;
  flex-wrap: wrap;
  align-items: baseline;
  gap: 2px 8px;
  flex: 1;
  min-width: 0;
}

.lic-name {
  font-size: 12.5px;
  font-weight: 600;
  color: var(--text);
}

.lic-meta {
  font-size: 11.5px;
}

/* 补充说明整条另起一行 */
.lic-note {
  flex-basis: 100%;
  font-size: 11.5px;
}

.lic-open {
  flex-shrink: 0;
  padding: 3px 10px;
  border: 1px solid var(--border);
  border-radius: 8px;
  background: transparent;
  color: var(--text-dim);
  font-family: inherit;
  font-size: 11.5px;
  cursor: pointer;
  transition: background 0.15s, color 0.15s;
}

.lic-open:hover {
  background: var(--bg-hover);
  color: var(--text);
}

.lic-spdx {
  display: block;
  margin: 14px auto 0;
}
</style>
