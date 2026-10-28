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
import AccountEmpty from "../AccountEmpty.vue";
import type { AccountStoreDto } from "../../../lib/bindings";

const props = defineProps<{
  /** 要展示的账户，调用方已按锁定 / 筛选 / 搜索过滤好 */
  accounts: AccountStoreDto[];
  /** 列表是不是被筛过：空结果给对应文案 */
  filtering: boolean;
  /** 当前使用的账户 uuid，空串表示没有 */
  currentUuid: string;
}>();

// 皮肤显示模式是否为 3D：仰视 / 俯视 / 旋转都用等距全身图，槽位与悬浮样式一致
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

// 悬停图片浮动显示对应大图：头像取头部渲染，皮肤取全身图，披风取正面 + 背面两张并排
const preview = ref<{
  x: number;
  y: number;
  url: string;
  backUrl: string;
  kind: string;
  acc: AccountStoreDto;
} | null>(null);

/** 按图片类型取预览大图地址 */
const previewUrl = (kind: string, acc: AccountStoreDto): string =>
  kind === "avatar"
    ? acc.avatar || accountAvatarUrl(acc)
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
    acc,
  };
}

function movePreview(e: MouseEvent) {
  if (!preview.value) return;
  // 右/下缘内翻，避免大图出窗：3D 等距图 320px 见方 + 边距
  preview.value.x = Math.min(e.clientX + 14, window.innerWidth - 352);
  preview.value.y = Math.min(e.clientY + 14, window.innerHeight - 368);
}

function hidePreview() {
  preview.value = null;
}

/** 点皮肤 / 披风图：先收起跟随鼠标的大图，再打开该账户的皮肤弹窗 */
function openSkin(acc: AccountStoreDto) {
  hidePreview();
  emit("viewSkin", acc);
}
</script>

<template>
  <div class="acc-grid">
    <div v-for="acc in accounts" :key="acc.uuid" class="acc-card" :class="{ current: isCurrent(acc) }"
      @dblclick="!accountLocked && emit('switch', acc)"
      @contextmenu.prevent="emit('menu', { event: $event, acc })">
      <div class="acc-images"
        :class="{ empty: imageFailed(acc, 'avatar') && imageFailed(acc, 'skin') && imageFailed(acc, 'cape') }">
        <span v-if="isCurrent(acc)" class="current-badge">{{ t("account.current") }}</span>
        <!-- 加载中：图上叠转圈，onload 后消失（img 先渲染，加载完才能触发事件） -->
        <div v-if="!imageFailed(acc, 'avatar')" class="img-box img-avatar-box">
          <img :src="acc.avatar || accountAvatarUrl(acc)" class="img-avatar"
            :class="{ pending: imageLoading(acc, 'avatar') }" :alt="t('account.avatar')" data-no-fallback
            @load="markImageLoaded(acc, 'avatar')" @error="markImageFailed(acc, 'avatar')"
            @mouseenter="showPreview($event, acc, 'avatar')" @mousemove="movePreview" @mouseleave="hidePreview" />
          <div v-if="imageLoading(acc, 'avatar')" class="img-spin" />
        </div>
        <!-- 头像由皮肤渲染而来：皮肤都没有时不再显示"无头像"，只留皮肤/披风占位 -->
        <div v-else-if="!imageFailed(acc, 'skin')" class="img-ph img-avatar-ph">
          <svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" stroke-width="1.6"
            stroke-linecap="round">
            <circle cx="12" cy="8" r="4" />
            <path d="M4 21c0-4 3.6-6.5 8-6.5s8 2.5 8 6.5" />
          </svg>
          <span>{{ t("account.noAvatar") }}</span>
        </div>
        <div v-if="!imageFailed(acc, 'skin')" class="img-box img-skin-box">
          <!-- 点开皮肤弹窗：与操作按钮里的「查看皮肤」同一个入口。
               双击的父级语义是"切换账户"，这里 .stop 掉，免得点两下顺手换了账户 -->
          <img :src="accountSkinUrl(acc)" class="img-skin" role="button" tabindex="0"
            :class="{ 'mode-3d': isSkin3D, pending: imageLoading(acc, 'skin') }" :alt="t('account.skin')"
            data-no-fallback @load="markImageLoaded(acc, 'skin')" @error="markImageFailed(acc, 'skin')"
            @mouseenter="showPreview($event, acc, 'skin')" @mousemove="movePreview" @mouseleave="hidePreview"
            @click.stop="openSkin(acc)" @dblclick.stop="openSkin(acc)" @keydown.enter.prevent="openSkin(acc)" />
          <div v-if="imageLoading(acc, 'skin')" class="img-spin" />
        </div>
        <div v-else class="img-ph img-skin-ph">
          <svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" stroke-width="1.6"
            stroke-linecap="round">
            <circle cx="12" cy="5" r="2.4" />
            <path d="M9.5 9h5l1.5 6h-2l-.8 7h-2.4l-.8-7h-2z" />
          </svg>
          <span>{{ t("account.noSkin") }}</span>
        </div>
        <div v-if="!imageFailed(acc, 'cape')" class="img-box img-cape-box">
          <img :src="accountCapeUrl(acc, capeDisplay)" class="img-cape" role="button" tabindex="0"
            :class="{ pending: imageLoading(acc, 'cape') }" :alt="t('account.cape')" data-no-fallback
            @load="markImageLoaded(acc, 'cape')" @error="markImageFailed(acc, 'cape')"
            @mouseenter="showPreview($event, acc, 'cape')" @mousemove="movePreview" @mouseleave="hidePreview"
            @click.stop="openSkin(acc)" @dblclick.stop="openSkin(acc)" @keydown.enter.prevent="openSkin(acc)" />
          <div v-if="imageLoading(acc, 'cape')" class="img-spin" />
        </div>
        <div v-else class="img-ph img-cape-ph">
          <svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" stroke-width="1.6"
            stroke-linecap="round" stroke-linejoin="round">
            <path d="M8 3h8l1 3-1 3v12l-4-2.5L8 21V9L7 6z" />
          </svg>
          <span>{{ t("account.noCape") }}</span>
        </div>
      </div>
      <div class="acc-head">
        <span class="acc-name">{{ acc.userName }}</span>
        <AccountTypeBadge :auth-type="acc.authType" />
        <AccountActions :locked="accountLocked && acc.uuid === currentUuid" :can-refresh="acc.canRefresh" :can-relogin="acc.canRelogin" :can-edit="acc.canEdit"
          :has-skin="!hasNoSkin(acc)" @refresh="emit('refresh', acc)" @view-skin="emit('viewSkin', acc)"
          @refresh-skin="emit('refreshSkin', acc)" @relogin="emit('relogin', acc)" @edit="emit('edit', acc)"
          @delete="emit('delete', acc)" />
      </div>
    </div>
    <AccountEmpty v-if="accounts.length === 0"
      :title="filtering ? t('account.searchEmpty') : t('account.emptyTitle')"
      :hint="filtering ? undefined : t('account.emptyHint')" :show-add="!filtering"
      @add="emit('add')" />

    <!-- 悬停图片时的大图预览（跟随鼠标，不拦截事件），尺寸按图片类型区分 -->
    <Teleport to="body">
      <div v-if="preview" class="skin-float"
        :class="[preview.kind, preview.kind === 'skin' && isSkin3D ? 'skin3d' : '']"
        :style="{ left: preview.x + 'px', top: preview.y + 'px' }">
        <img :src="preview.url" alt="" />
        <img v-if="preview.backUrl" :src="preview.backUrl" alt="" />
      </div>
    </Teleport>
  </div>
</template>

<style scoped>
.acc-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(340px, 1fr));
  gap: 16px;
}

/* 空状态：横跨全部列并在内容区居中。
   48px 的上下留白是有意的：它要在一片卡片格里撑出一块呼吸区，
   与全局 .empty-tip 的 10px 不是一个场景，不回全局值 */
.empty-tip {
  grid-column: 1 / -1;
  padding: 48px 0;
}

.acc-card {
  background: var(--bg-card);
  border: 1px solid var(--border);
  border-radius: 16px;
  padding: 18px;
  display: flex;
  flex-direction: column;
  gap: 14px;
}

.acc-images {
  position: relative;
  display: flex;
  align-items: center;
  gap: 12px;
  justify-content: center;
  padding: 18px 0;
  background: var(--bg-side);
  border-radius: 12px;
}

/* 无皮肤时不再铺色块，占位直接落在卡片底色上 */
.acc-images.empty {
  background: transparent;
}

.current-badge {
  position: absolute;
  top: 8px;
  left: 8px;
  font-size: 10.5px;
  padding: 2px 9px;
  border-radius: 20px;
  background: var(--accent);
  color: #fff;
  white-space: nowrap;
}

/* 图片槽容器：相对定位，加载中时上面叠转圈 */
.img-box {
  position: relative;
  flex-shrink: 0;
}

.img-box img.pending {
  opacity: 0;
  /* 加载中先隐藏（遮住 alt 文本），onload 后再显示 */
}

/* 加载中转圈（img 上层居中） */
.img-spin {
  position: absolute;
  inset: 0;
  display: flex;
  align-items: center;
  justify-content: center;
}

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

.img-avatar-box,
.img-avatar {
  width: 56px;
  height: 56px;
}

.img-avatar {
  image-rendering: pixelated;
}

.img-skin-box,
.img-skin {
  width: 48px;
  height: 96px;
}

.img-skin {
  border-radius: 5px;
  image-rendering: pixelated;
}

/* 披风渲染输出为 10×16 竖图（cape_2d_draw），槽位与皮肤同宽 48px，
   高度取槽位高，contain 居中不变形 */
.img-cape-box {
  width: 48px;
  height: 96px;
}

.img-cape {
  width: 48px;
  height: 96px;
  object-fit: contain;
  border-radius: 5px;
  image-rendering: pixelated;
}

/* 皮肤 / 披风图点开皮肤弹窗（与操作按钮里的「查看皮肤」同一入口）：
   跟随鼠标的大图只说明"能看大图"，要点得动还得给个描边信号 */
.img-skin,
.img-cape {
  cursor: pointer;
}

.img-skin:hover,
.img-skin:focus-visible,
.img-cape:hover,
.img-cape:focus-visible {
  outline: 1px solid var(--accent);
  outline-offset: 1px;
}

/* 图片拉取失败 / 无皮肤时的占位：图标 + 横排文字，三个槽位样式统一 */
.img-ph {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 5px;
  font-size: 10px;
  color: var(--text-dim);
  user-select: none;
}

.img-ph svg {
  opacity: 0.55;
}

.img-avatar-ph {
  width: 56px;
  height: 56px;
  border: 1px dashed var(--border);
}

.img-skin-ph {
  width: 48px;
  height: 96px;
  border: 1px dashed var(--border);
  border-radius: 5px;
}

.img-cape-ph {
  width: 48px;
  height: 96px;
  border: 1px dashed var(--border);
  border-radius: 5px;
}

.acc-head {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
}

.acc-name {
  flex: 1;
  min-width: 0;
  font-size: 14px;
  font-weight: 700;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.acc-card.current {
  border-color: var(--accent);
  box-shadow: 0 0 0 1px var(--accent);
}

/* 悬停图片时的大图：Teleport 到 body，跟随鼠标，按图片类型取不同尺寸 */
.skin-float {
  position: fixed;
  z-index: 999;
  pointer-events: none;
  padding: 8px;
  background: var(--bg-card);
  border: 1px solid var(--border);
  border-radius: 10px;
  box-shadow: var(--shadow-lg);
}

.skin-float img {
  display: block;
  width: 160px;
  height: 160px;
  image-rendering: pixelated;
}

.skin-float.skin img {
  width: 128px;
  height: 256px;
}

/* 皮肤显示模式 = 3D：等距全身图，渲染图为 272x532，contain 防拉伸变形 */
.skin-float.skin3d img {
  width: 320px;
  height: 320px;
  object-fit: contain;
  image-rendering: auto;
}

/* 披风悬浮大图 = 正面 + 背面两张并排，各 160x256，gap 分开间距 */
.skin-float.cape {
  display: flex;
  gap: 8px;
}

.skin-float.cape img {
  width: 160px;
  height: 256px;
}

/* 皮肤显示模式 = 3D：渲染图为 272×532 竖图，铺满整个槽位（contain 按宽缩放），
   渲染图自带抗锯齿，不用 pixelated */
.img-skin.mode-3d {
  image-rendering: auto;
  object-fit: contain;
}
</style>
