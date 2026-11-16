<script setup lang="ts">
// 账户平铺视图：皮肤 / 头像 / 披风 三图卡片
import { computed, ref } from "vue";
import { accountLocked } from "../../../lib/accountStore";
import { t } from "../../../lib/i18n";
import {
  accountAvatarUrl,
  accountCapeBackUrl,
  accountCapeUrl,
  accountSkinUrl,
  imageFailed,
  imageLoading,
  hasNoSkin,
  markImageFailed,
  markImageLoaded,
} from "../../../lib/accountImages";
import { capeDisplay, skinDisplay } from "../../../lib/settings";
import AccountActions from "../../../components/AccountActions.vue";
import AccountTypeBadge from "../../../components/AccountTypeBadge.vue";
import HighlightText from "../../../components/ui/HighlightText.vue";
import AccountEmpty from "../AccountEmpty.vue";
import type { AccountStoreDto } from "../../../lib/bindings";

const props = defineProps<{
  /** 要展示的账户，调用方已按锁定 / 筛选 / 搜索过滤好 */
  accounts: AccountStoreDto[];
  /** 列表是不是被筛过：空结果给对应文案 */
  filtering: boolean;
  /** 当前使用的账户 uuid，空串表示没有 */
  currentUuid: string;
  /** 搜索词，用来给命中的名字加底色 */
  query: string;
  /** 把账户的令牌状态翻成文案，右上角有效性提示用 */
  tokenLabel: (acc: AccountStoreDto) => string;
}>();

// 皮肤显示模式是否为 3D：仰视 / 俯视 / 旋转都用等距全身图，槽位尺寸一致
const isSkin3D = computed(
  () =>
    skinDisplay.value === "Skin3D" ||
    skinDisplay.value === "Skin3DD" ||
    skinDisplay.value === "Skin3DRotate",
);

// 只有 2D 正反面档的悬浮大图是正 + 背两张，其余档都是同一张图放大
const isCape2D = computed(() => capeDisplay.value === "Cape2D");

const emit = defineEmits<{
  /** 双击账户：切换为当前使用 */
  (e: "switch", acc: AccountStoreDto): void;
  /** 刷新该账户的令牌 */
  (e: "refresh", acc: AccountStoreDto): void;
  /** 预览该账户的皮肤 / 披风 */
  (e: "viewSkin", acc: AccountStoreDto): void;
  /** 重取该账户的皮肤渲染图 */
  (e: "refreshSkin", acc: AccountStoreDto): void;
  /** 重新登录该账户 */
  (e: "relogin", acc: AccountStoreDto): void;
  /** 编辑该账户 */
  (e: "edit", acc: AccountStoreDto): void;
  /** 删除该账户 */
  (e: "delete", acc: AccountStoreDto): void;
  /** 右键账户：打开操作菜单 */
  (e: "menu", payload: { event: MouseEvent; acc: AccountStoreDto }): void;
  /** 空状态里点「添加账户」 */
  (e: "add"): void;
}>();

/** 是否当前使用的账户 */
function isCurrent(acc: AccountStoreDto): boolean {
  return acc.uuid === props.currentUuid;
}

/** 点皮肤 / 披风图：先收起跟随鼠标的大图，再打开该账户的皮肤子页面 */
function openSkin(acc: AccountStoreDto) {
  hidePreview();
  emit("viewSkin", acc);
}

// 悬停图片浮动显示对应大图：头像取头部渲染，皮肤取全身图，披风取正面 + 背面两张并排
const preview = ref<{
  x: number;
  y: number;
  url: string;
  backUrl: string;
  kind: string;
} | null>(null);

/** 按图片类型取预览大图地址 */
const previewUrl = (kind: string, acc: AccountStoreDto): string =>
  kind === "avatar"
    ? accountAvatarUrl(acc)
    : kind === "skin"
      ? accountSkinUrl(acc)
      : accountCapeUrl(acc, capeDisplay.value);

/** 悬停时定位并展示大图；该图已加载失败就不展示 */
function showPreview(e: MouseEvent, acc: AccountStoreDto, kind: string) {
  if (imageFailed(acc, kind)) return;
  preview.value = {
    x: e.clientX,
    y: e.clientY,
    kind,
    url: previewUrl(kind, acc),
    // 披风 2D 档悬浮多带一张背面图（鞘翅各档没有背面，空串不渲染）
    backUrl: kind === "cape" && isCape2D.value ? accountCapeBackUrl(acc) : "",
  };
}

/** 大图跟随鼠标，靠近右 / 下缘时向内翻 */
function movePreview(e: MouseEvent) {
  if (!preview.value) return;
  // 右/下缘内翻，避免大图出窗：3D 等距图 320px 见方 + 边距
  preview.value.x = Math.min(e.clientX + 14, window.innerWidth - 352);
  preview.value.y = Math.min(e.clientY + 14, window.innerHeight - 368);
}

/** 收起悬浮大图 */
function hidePreview() {
  preview.value = null;
}
</script>

<template>
  <div class="acc-grid grid grid-cols-[repeat(auto-fill,minmax(340px,1fr))] gap-4" :class="{ empty: accounts.length === 0 }">
    <div v-for="acc in accounts" :key="acc.uuid" class="acc-card flex flex-col gap-3.5 rounded-[16px] border border-border bg-card p-[18px]" :class="{ current: isCurrent(acc) }"
      @dblclick="!accountLocked && emit('switch', acc)" @contextmenu.prevent="emit('menu', { event: $event, acc })">
      <div class="acc-images relative flex items-center justify-center gap-3 rounded-panel bg-side py-[18px]"
        :class="{ empty: imageFailed(acc, 'avatar') && imageFailed(acc, 'skin') && imageFailed(acc, 'cape') }">
        <span v-if="isCurrent(acc)" class="current-badge absolute left-2 top-2 whitespace-nowrap rounded-pill bg-accent px-[9px] py-0.5 text-caption text-on-accent">{{ t("account.current") }}</span>
        <!-- 平铺卡片看不到详情表格里的 Token 列，右上角直接标出这一条是否有效 -->
        <span class="validity-badge absolute right-2 top-2 whitespace-nowrap rounded-pill px-[9px] py-0.5 text-caption" :class="acc.tokenStatus">{{ tokenLabel(acc) }}</span>
        <!-- 加载中：图上叠转圈，onload 后消失（img 先渲染，加载完才能触发事件） -->
        <div v-if="!imageFailed(acc, 'avatar')" class="img-box img-avatar-box relative h-14 w-14 shrink-0">
          <!-- 头像也点开皮肤弹窗：三者是同一个入口，描边与悬停大图的口径要一致 -->
          <img :src="accountAvatarUrl(acc)" class="img-avatar h-14 w-14 cursor-pointer [image-rendering:pixelated]" role="button" tabindex="0"
            :class="{ pending: imageLoading(acc, 'avatar') }" :alt="t('account.avatar')" data-no-fallback
            @load="markImageLoaded(acc, 'avatar')" @error="markImageFailed(acc, 'avatar')"
            @mouseenter="showPreview($event, acc, 'avatar')" @mousemove="movePreview" @mouseleave="hidePreview"
            @click.stop="openSkin(acc)" @dblclick.stop="openSkin(acc)" @keydown.enter.prevent="openSkin(acc)" />
          <div v-if="imageLoading(acc, 'avatar')" class="img-spin absolute inset-0 flex items-center justify-center" />
        </div>
        <!-- 头像由皮肤渲染而来：皮肤都没有时不再显示"无头像"，只留皮肤/披风占位 -->
        <div v-else-if="!imageFailed(acc, 'skin')" class="img-ph img-avatar-ph flex h-14 w-14 select-none flex-col items-center justify-center gap-[5px] border border-dashed border-border text-[10px] text-dim">
          <svg class="opacity-[0.55]" viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" stroke-width="1.6"
            stroke-linecap="round">
            <circle cx="12" cy="8" r="4" />
            <path d="M4 21c0-4 3.6-6.5 8-6.5s8 2.5 8 6.5" />
          </svg>
          <span>{{ t("account.noAvatar") }}</span>
        </div>
        <div v-if="!imageFailed(acc, 'skin')" class="img-box img-skin-box relative h-24 w-12 shrink-0">
          <!-- 点开皮肤弹窗：与操作按钮里的「查看皮肤」同一个入口。
               双击的父级语义是"切换账户"，这里 .stop 掉，免得点两下顺手换了账户 -->
          <img :src="accountSkinUrl(acc)" class="img-skin h-24 w-12 cursor-pointer rounded-[5px] [image-rendering:pixelated]" role="button" tabindex="0"
            :class="{ 'mode-3d': isSkin3D, pending: imageLoading(acc, 'skin') }" :alt="t('account.skin')"
            data-no-fallback @load="markImageLoaded(acc, 'skin')" @error="markImageFailed(acc, 'skin')"
            @mouseenter="showPreview($event, acc, 'skin')" @mousemove="movePreview" @mouseleave="hidePreview"
            @click.stop="openSkin(acc)" @dblclick.stop="openSkin(acc)" @keydown.enter.prevent="openSkin(acc)" />
          <div v-if="imageLoading(acc, 'skin')" class="img-spin absolute inset-0 flex items-center justify-center" />
        </div>
        <div v-else class="img-ph img-skin-ph flex h-24 w-12 select-none flex-col items-center justify-center gap-[5px] rounded-[5px] border border-dashed border-border text-[10px] text-dim">
          <svg class="opacity-[0.55]" viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" stroke-width="1.6"
            stroke-linecap="round">
            <circle cx="12" cy="5" r="2.4" />
            <path d="M9.5 9h5l1.5 6h-2l-.8 7h-2.4l-.8-7h-2z" />
          </svg>
          <span>{{ t("account.noSkin") }}</span>
        </div>
        <div v-if="!imageFailed(acc, 'cape')" class="img-box img-cape-box relative h-24 w-12 shrink-0">
          <img :src="accountCapeUrl(acc, capeDisplay)" class="img-cape h-24 w-12 cursor-pointer rounded-[5px] object-contain [image-rendering:pixelated]" role="button" tabindex="0"
            :class="{ pending: imageLoading(acc, 'cape') }" :alt="t('account.cape')" data-no-fallback
            @load="markImageLoaded(acc, 'cape')" @error="markImageFailed(acc, 'cape')"
            @mouseenter="showPreview($event, acc, 'cape')" @mousemove="movePreview" @mouseleave="hidePreview"
            @click.stop="openSkin(acc)" @dblclick.stop="openSkin(acc)" @keydown.enter.prevent="openSkin(acc)" />
          <div v-if="imageLoading(acc, 'cape')" class="img-spin absolute inset-0 flex items-center justify-center" />
        </div>
        <div v-else class="img-ph img-cape-ph flex h-24 w-12 select-none flex-col items-center justify-center gap-[5px] rounded-[5px] border border-dashed border-border text-[10px] text-dim">
          <svg class="opacity-[0.55]" viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" stroke-width="1.6"
            stroke-linecap="round" stroke-linejoin="round">
            <path d="M8 3h8l1 3-1 3v12l-4-2.5L8 21V9L7 6z" />
          </svg>
          <span>{{ t("account.noCape") }}</span>
        </div>
      </div>
      <div class="acc-head flex min-w-0 items-center gap-2">
        <span class="acc-name min-w-0 flex-1 overflow-hidden text-ellipsis whitespace-nowrap text-[14px] font-bold">
          <HighlightText :text="acc.userName" :query="query" />
        </span>
        <AccountTypeBadge :auth-type="acc.authType" />
        <AccountActions :locked="accountLocked && acc.uuid === currentUuid" :can-refresh="acc.canRefresh"
          :can-relogin="acc.canRelogin" :can-edit="acc.canEdit" :has-skin="!hasNoSkin(acc)"
          @refresh="emit('refresh', acc)" @view-skin="emit('viewSkin', acc)" @refresh-skin="emit('refreshSkin', acc)"
          @relogin="emit('relogin', acc)" @edit="emit('edit', acc)" @delete="emit('delete', acc)" />
      </div>
    </div>
    <AccountEmpty v-if="accounts.length === 0" :title="filtering ? t('account.searchEmpty') : t('account.emptyTitle')"
      :hint="filtering ? undefined : t('account.emptyHint')" :show-add="!filtering" @add="emit('add')" />

    <!-- 悬停图片时的大图预览（跟随鼠标，不拦截事件），尺寸按图片类型区分 -->
    <Teleport to="body">
      <div v-if="preview" class="skin-float pointer-events-none fixed z-[300] rounded-card border border-border bg-card p-2 shadow-[var(--shadow-lg)]"
        :class="[preview.kind, preview.kind === 'skin' && isSkin3D ? 'skin3d' : '']"
        :style="{ left: preview.x + 'px', top: preview.y + 'px' }">
        <img class="block h-[160px] w-[160px] [image-rendering:pixelated]" :src="preview.url" alt="" />
        <img v-if="preview.backUrl" class="block h-[160px] w-[160px] [image-rendering:pixelated]" :src="preview.backUrl"
          alt="" />
      </div>
    </Teleport>
  </div>
</template>

<style scoped>
/* 排版 / 间距 / 字号 / 颜色在模板的工具类上；这里留状态类、伪类与按类型分档的
   悬浮大图尺寸 —— 它们都挂着状态或后代选择器，工具类表达不了 */
.acc-grid.empty {
  display: flex;
  flex-direction: column;
  flex: 1;
  min-height: 0;
}

/* 无皮肤时不再铺色块，占位直接落在卡片底色上 */
.acc-images.empty {
  background: transparent;
}

/* 当前使用的账户：描边 + 一圈光晕 */
.acc-card.current {
  border-color: var(--accent);
  box-shadow: 0 0 0 1px var(--accent);
}

/* 图片加载中先压透明，onload 后再显示 */
.img-box img.pending {
  opacity: 0;
}

/* 加载中转圈：定位与居中在工具类上，这里只剩伪元素与关键帧 */
.img-spin::after {
  content: "";
  width: 18px;
  height: 18px;
  border: 2px solid var(--border);
  border-top-color: var(--accent);
  border-radius: 50%;
  animation: img-spin 0.8s linear infinite;
}

@keyframes img-spin {
  to {
    transform: rotate(360deg);
  }
}

/* 三张图都是一个入口：描边跟着悬停 / 聚焦走，工具类写不了伪类组合 */
.img-avatar:hover,
.img-avatar:focus-visible,
.img-skin:hover,
.img-skin:focus-visible,
.img-cape:hover,
.img-cape:focus-visible {
  outline: 1px solid var(--accent);
  outline-offset: 1px;
}

/* 有效性徽章状态色：与列表的 token 标记同一套 */
.validity-badge.valid {
  background: color-mix(in srgb, var(--green) 16%, transparent);
  color: var(--green);
}

.validity-badge.expired {
  background: color-mix(in srgb, var(--red) 16%, transparent);
  color: var(--red);
}

/* 悬浮大图的基准尺寸在模板的工具类上；下面按图片类型覆盖，属于修饰类 + 后代选择器 */
.skin-float.skin img {
  width: 128px;
  height: 256px;
}

/* 皮肤 3D：等距全身图，contain 防拉伸 */
.skin-float.skin3d img {
  width: 320px;
  height: 320px;
  object-fit: contain;
  image-rendering: auto;
}

/* 披风悬浮大图 = 正面 + 背面两张并排 */
.skin-float.cape {
  display: flex;
  gap: 8px;
}

.skin-float.cape img {
  width: 160px;
  height: 256px;
}

/* 皮肤 3D 档：渲染图自带抗锯齿，不用 pixelated */
.img-skin.mode-3d {
  image-rendering: auto;
  object-fit: contain;
}
</style>
