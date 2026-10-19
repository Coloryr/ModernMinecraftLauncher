<script setup lang="ts">
// 帮助窗口：FAQ 手风琴 + 开源许可 + 问题反馈提示，文案取当前语言的 locale 包
//
// 开源许可只给**上游地址**：许可全文不随安装包分发，点按钮由系统浏览器打开
// （见 ./thirdParty.ts 的说明）。
import { computed, ref } from "vue";
import WindowFrame from "../../components/ui/WindowFrame.vue";
import { t, locale } from "../../lib/i18n";
import { api } from "../../lib/api";
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

const faqs = computed(() => (locale.value === "en_us" ? enFaqs : zhFaqs));

const openIndex = ref<number | null>(0);

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
</script>

<template>
  <WindowFrame :title="t('features.help')" body-gutter @close="$emit('close')">
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
  padding: 15px 18px;
  border: none;
  background: transparent;
  color: var(--text);
  font-size: 14px;
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
  padding: 4px 18px 16px;
  font-size: 13px;
  color: var(--text-dim);
  line-height: 1.7;
}

.footer-note {
  margin-top: 22px;
  padding: 16px 18px;
  background: var(--bg-card);
  border: 1px solid var(--border);
  border-radius: 12px;
  font-size: 12.5px;
  color: var(--text-dim);
  line-height: 1.8;
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
  padding: 4px 18px 16px;
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
  font-size: 12px;
}

.lic-row {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 7px 0;
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
  font-size: 12px;
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
