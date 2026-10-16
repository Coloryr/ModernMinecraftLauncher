<script setup lang="ts">
// 皮肤预览弹窗：2D 平面图 / 3D 模型（skinview3d）预览当前选中账户，
// 并列出该账户所有皮肤 / 披风；正版账户可"设为装备"，第三方规范只有单皮肤单披风
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import BaseModal from "../../components/ui/BaseModal.vue";
import BaseButton from "../../components/ui/BaseButton.vue";
import BaseSwitch from "../../components/ui/BaseSwitch.vue";
import GlyphIcon from "../../components/ui/GlyphIcon.vue";
import SegmentedTabs from "../../components/ui/SegmentedTabs.vue";
import { t, tErr } from "../../lib/i18n";
import { showToast } from "../../lib/toast";
import { currentAccount } from "../../lib/accountStore";
import { getImageBaseUrl } from "../../lib/api";
import { commands, type TexturesDto, type TextureItemDto } from "../../lib/bindings";
import type { AccountStoreDto } from "../../lib/bindings";
import { capeRenderUrl } from "../../lib/accountImages";
import { capeDisplay } from "../../lib/settings";
import * as skinview3d from "skinview3d";

/** 展示模式 */
type Mode = "2d" | "3d";

/** 2D / 3D 分段切换选项 */
const MODE_OPTIONS = computed(() => [
  { value: "3d", label: t("winSkin.mode3d") },
  { value: "2d", label: t("winSkin.mode2d") },
]);

/** 皮肤类型：auto 自动检测，old 1.7 旧版，new 1.8 新版经典，slim 纤细 */
type SkinTypeOpt = "auto" | "old" | "new" | "slim";

const mode = ref<Mode>("3d");
/** 是否渲染披风 */
const showCape = ref(true);

/** 皮肤类型选项（URI段值 + i18n键） */
const SKIN_TYPES: Array<{ value: SkinTypeOpt; labelKey: string }> = [
  { value: "auto", labelKey: "winSkin.skinAuto" },
  { value: "new", labelKey: "winSkin.skinClassic" },
  { value: "slim", labelKey: "winSkin.skinSlim" },
  { value: "old", labelKey: "winSkin.skinOld" },
];

const skinType = ref<SkinTypeOpt>("auto");
/** 用户手动选过型号后，切换预览对象不再自动跟随服务器声明 */
const skinTypeTouched = ref(false);

/** 动画类型，仅 3D；与 demo 一致的 8 种 + 不播放 */
type AnimType = "none" | "idle" | "walk" | "run" | "fly" | "wave" | "crouch" | "hit" | "swim";
/** 当前动画（默认不播放） */
const animType = ref<AnimType>("none");
/** 是否自动旋转（仅3D） */
const autoRotate = ref(false);
/** 名牌（仅3D，显示账户名） */
const showNametag = ref(false);

/** 动画类型选项（值 + i18n键） */
const ANIM_TYPES: Array<{ value: AnimType; labelKey: string }> = [
  { value: "none", labelKey: "winSkin.animNone" },
  { value: "idle", labelKey: "winSkin.animIdle" },
  { value: "walk", labelKey: "winSkin.animWalk" },
  { value: "run", labelKey: "winSkin.animRun" },
  { value: "fly", labelKey: "winSkin.animFly" },
  { value: "wave", labelKey: "winSkin.animWave" },
  { value: "crouch", labelKey: "winSkin.animCrouch" },
  { value: "hit", labelKey: "winSkin.animHit" },
  { value: "swim", labelKey: "winSkin.animSwim" },
];

/** 动画类型 → 动画实例工厂 */
const ANIM_FACTORY: Record<Exclude<AnimType, "none">, () => skinview3d.PlayerAnimation> = {
  idle: () => new skinview3d.IdleAnimation(),
  walk: () => new skinview3d.WalkingAnimation(),
  run: () => new skinview3d.RunningAnimation(),
  fly: () => new skinview3d.FlyingAnimation(),
  wave: () => new skinview3d.WaveAnimation(),
  crouch: () => new skinview3d.CrouchAnimation(),
  hit: () => new skinview3d.HitAnimation(),
  swim: () => new skinview3d.SwimAnimation(),
};

/** 请求关闭弹窗 */
const emit = defineEmits<{ close: [] }>();

const base = ref("");
/** 皮肤获取失败（离线账户 / 下载失败） */
const noSkin = ref(false);
/** 预览加载中：3D 模型 / 2D 贴图未就绪时在预览区盖加载指示 */
const loading = ref(false);
/** 纹理列表加载中：打开弹窗 / 换账户后首次拉取较慢，列表区给个提示 */
const listLoading = ref(false);
/** 待上传的本地皮肤文件路径，空表示无；选中后选型号直传，须在下方 immediate watch 之前声明 */
const uploadFile = ref("");

/**
 * 目标账户由入口传进来：账户条目上的「查看皮肤」按钮点的是哪一条就预览哪一条
 *
 * 不传时回退到全局当前账户，兼容旧的「只看当前账户」用法
 */
const props = defineProps<{ account?: AccountStoreDto | null }>();

const target = computed(() => props.account ?? currentAccount.value);

/** 当前账户的皮肤 / 披风纹理列表，离线 / 查询失败为 null 走占位图 */
const textures = ref<TexturesDto | null>(null);

/** 是否正版账户：第三方规范只有单皮肤单披风，无装备操作 */
const isOauth = computed(() => target.value?.authType === "microsoft");

/** 是否第三方在线账户：皮肤站 / 统一通行证 / 外置登录，皮肤管理在对应网页完成 */
const isThirdParty = computed(
  () => !!target.value && !isOauth.value && target.value.authType !== "offline",
);

/** 用系统浏览器打开账户对应皮肤站的网页 */
async function openSkinSite() {
  const acc = target.value;
  if (!acc) return;
  try {
    await commands.account.openSkinSite(acc.authType, acc.uuid);
  } catch (e) {
    showToast(tErr(e));
  }
}

/** 当前装备的皮肤：无 active 取第一个，离线 / 查询失败为 null */
const activeSkin = computed(() => {
  const list = textures.value?.skins ?? [];
  return list.find((item) => item.active) ?? list[0] ?? null;
});

/** 当前装备的披风（同上） */
const activeCape = computed(() => {
  const list = textures.value?.capes ?? [];
  return list.find((item) => item.active) ?? list[0] ?? null;
});

/** 预览选中的皮肤 / 披风 sha1（默认当前装备项，点列表切换） */
const previewSkinSha1 = ref("");
const previewCapeSha1 = ref("");

/** 预览目标：列表点选优先，否则当前装备项 */
const selectedSkin = computed(() => {
  const list = textures.value?.skins ?? [];
  return list.find((item) => item.sha1 === previewSkinSha1.value) ?? activeSkin.value;
});
const selectedCape = computed(() => {
  const list = textures.value?.capes ?? [];
  return list.find((item) => item.sha1 === previewCapeSha1.value) ?? activeCape.value;
});

/** 纹理列表变化时把预览选中重置回当前装备项 */
watch(textures, () => {
  previewSkinSha1.value = activeSkin.value?.sha1 ?? "";
  previewCapeSha1.value = activeCape.value?.sha1 ?? "";
});

// 预览对象变化：型号下拉跟随服务器声明的版型。纤细皮肤贴图与经典同版面，
// 自动检测分不出来，必须显式跟随；用户手动选过则尊重手选
watch(selectedSkin, (s) => {
  if (!skinTypeTouched.value) {
    skinType.value = s?.model === "slim" ? "slim" : "auto";
  }
});

// 账户变化：重拉列表。不先清空，装备披风等操作触发的 account-change 刷新时
// 旧列表继续显示，避免预览区整个塌掉重载；换账户才在失败时清空
let texturesUuid = "";
watch(
  target,
  async (acc) => {
    if (!acc) {
      textures.value = null;
      texturesUuid = "";
      return;
    }
    const accountChanged = acc.uuid !== texturesUuid;
    uploadFile.value = "";
    listLoading.value = true;
    try {
      textures.value = await commands.account.getTextures(acc.authType, acc.uuid);
      texturesUuid = acc.uuid;
    } catch {
      if (accountChanged) textures.value = null;
    } finally {
      listLoading.value = false;
    }
  },
  { immediate: true },
);

// 取图走 sha1 型 URI；2D 全身图的型号：用户没动下拉时跟随服务器声明的型号。
// 纤细皮肤贴图与经典版面相同，纯像素检测分不出来，必须带上；动了就用选的
const modelParam = computed(() => {
  if (skinType.value !== "auto") return skinType.value;
  return selectedSkin.value?.model === "slim" ? "slim" : "auto";
});

// 2D 全身图：后端按全局显示模式配置决定渲染，这里只传皮肤型号
const skin2dUrl = computed(() =>
  base.value && selectedSkin.value ? `${base.value}/skin/${selectedSkin.value.sha1}/${modelParam.value}` : "",
);
const skinrawUrl = computed(() =>
  base.value && selectedSkin.value ? `${base.value}/skinraw/${selectedSkin.value.sha1}` : "",
);
const caperawUrl = computed(() =>
  base.value && selectedCape.value ? `${base.value}/caperaw/${selectedCape.value.sha1}` : "",
);
const cape2dUrl = computed(() =>
  base.value && selectedCape.value
    ? capeRenderUrl(selectedCape.value.sha1, capeDisplay.value)
    : "",
);

/** 型号字段转显示名：slim 是纤细 Alex，空是经典 Steve */
function modelLabel(model: string): string {
  return model === "slim" ? t("winSkin.skinSlim") : t("winSkin.skinClassic");
}

/** 列表缩略图：型号跟随条目声明，纤细皮肤不渲染成经典版型 */
function skinThumbUrl(item: TextureItemDto): string {
  return `${base.value}/skin/${item.sha1}/${item.model === "slim" ? "slim" : "new"}`;
}

/** 披风列表缩略图，档位跟随"披风显示"配置 */
function capeThumbUrl(item: TextureItemDto): string {
  return capeRenderUrl(item.sha1, capeDisplay.value);
}

// ---- 装备操作（正版） ----

/** 正在装备的 sha1（按钮禁用标记） */
const equipBusy = ref("");

/** 上传进行中 */
const uploading = ref(false);

/** 打开系统文件对话框选皮肤 PNG，浏览器没有对话框插件、按钮本身也不显示 */
async function pickSkinFile() {
  try {
    const { open } = await import("@tauri-apps/plugin-dialog");
    const picked = await open({
      title: t("account.uploadSkin"),
      multiple: false,
      filters: [{ name: "PNG", extensions: ["png"] }],
    });
    if (typeof picked === "string" && picked) {
      uploadFile.value = picked;
    }
  } catch {
    /* 非桌面环境 */
  }
}

/** 上传选中的皮肤文件并装备，variant 为经典或纤细 */
async function doUpload(variant: "classic" | "slim") {
  const acc = target.value;
  if (!acc || !uploadFile.value || uploading.value) return;
  uploading.value = true;
  try {
    await commands.account.uploadSkin(acc.authType, acc.uuid, variant, uploadFile.value);
    uploadFile.value = "";
    textures.value = await commands.account.getTextures(acc.authType, acc.uuid);
    showToast(t("account.uploadOk"));
  } catch (e) {
    showToast(tErr(e));
  } finally {
    uploading.value = false;
  }
}

/** 装备选中的皮肤 / 披风，仅正版账户可用 */
async function equip(kind: "skin" | "cape", item: TextureItemDto) {
  const acc = target.value;
  if (!acc || equipBusy.value) return;
  equipBusy.value = item.sha1;
  try {
    if (kind === "skin") {
      await commands.account.setActiveSkin(acc.authType, acc.uuid, item.sha1);
    } else {
      await commands.account.setActiveCape(acc.authType, acc.uuid, item.sha1);
    }
    // 后端已广播 account-change，这里重拉列表刷新 active 标记与预览
    textures.value = await commands.account.getTextures(acc.authType, acc.uuid);
    showToast(t("account.equipOk"));
  } catch (e) {
    showToast(tErr(e));
  } finally {
    equipBusy.value = "";
  }
}

// ---- 3D 渲染（skinview3d） ----
const stage = ref<HTMLDivElement | null>(null);
const canvas = ref<HTMLCanvasElement | null>(null);
let viewer: skinview3d.SkinViewer | null = null;

/** 销毁 3D 观察器 */
function destroyViewer() {
  viewer?.dispose();
  viewer = null;
}

/** 皮肤类型转 skinview3d 模型：auto 优先按服务器标记的纤细型号，比贴图像素检测可靠 */
const modelOpt = computed(() => {
  switch (skinType.value) {
    case "slim":
      return "slim" as const;
    case "new":
      return "default" as const;
    default:
      return selectedSkin.value?.model === "slim" ? ("slim" as const) : ("auto-detect" as const);
  }
});

/** 按当前模式与贴图重建 3D 观察器 */
function setupViewer() {
  destroyViewer();
  noSkin.value = false;
  if (mode.value !== "3d" || !canvas.value || !stage.value || !skinrawUrl.value) return;

  viewer = new skinview3d.SkinViewer({
    canvas: canvas.value,
    width: stage.value.clientWidth,
    height: stage.value.clientHeight,
    model: modelOpt.value,
    // 观感对齐 skinview3d 官方 demo 的默认参数，即 fov 70 / zoom 0.9
    fov: 70,
    zoom: 0.9,
  });
  // 灯光同样对齐 demo：全局光 3 / 相机光 0.6
  viewer.globalLight.intensity = 3;
  viewer.cameraLight.intensity = 0.6;
  // 允许旋转 / 滚轮缩放，禁平移（与 demo 控制项一致）
  viewer.controls.enablePan = false;
  viewer.autoRotate = autoRotate.value;
  // demo 的自动旋转速度默认 2
  viewer.autoRotateSpeed = 2;
  applyAnim();

  // 皮肤 + 披风都就绪（或确定失败）才撤掉加载指示
  // loadSkin 必须显式传 model：不传时默认按贴图自动检测，纤细皮肤贴图版面
  // 与经典相同，检测结果恒为经典，会覆盖构造时传的 model 选项
  loading.value = true;
  const skinP = viewer
    .loadSkin(skinrawUrl.value, { model: modelOpt.value })
    .catch(() => {
      noSkin.value = true;
    });
  // 没有披风的账户加载失败是正常的，静默忽略
  const capeP =
    showCape.value && caperawUrl.value
      ? viewer.loadCape(caperawUrl.value).catch(() => { })
      : Promise.resolve();
  Promise.allSettled([skinP, capeP]).finally(() => {
    loading.value = false;
  });

  if (showNametag.value && target.value) {
    viewer.nameTag = target.value.userName;
  }
}

/** 按下拉选择挂载/清空动画 */
function applyAnim() {
  if (!viewer) return;
  viewer.animation = animType.value === "none" ? null : ANIM_FACTORY[animType.value]();
}

watch([mode, skinrawUrl, modelOpt], () => setupViewer());

// 披风开关 / 切换预览披风：不重建观察器，直接挂载 / 卸载（皮肤不跟着重载）
watch([caperawUrl, showCape], () => {
  if (!viewer || mode.value !== "3d") return;
  if (showCape.value && caperawUrl.value) {
    loading.value = true;
    viewer
      .loadCape(caperawUrl.value)
      .catch(() => { })
      .finally(() => {
        loading.value = false;
      });
  } else {
    viewer.loadCape(null);
  }
});

// 名牌开关：跟随账户名
watch([showNametag, target], () => {
  if (!viewer || mode.value !== "3d") return;
  viewer.nameTag = showNametag.value && target.value ? target.value.userName : null;
});

// 自动旋转开关：直接开关观察器的自转，不重建
watch(autoRotate, () => {
  if (!viewer) return;
  viewer.autoRotate = autoRotate.value;
});

// 动画下拉：切换动画类型 / 清空
watch(animType, () => applyAnim());

// 2D 图片换源时清除上一次的失败标记，并重新进入加载中状态（@load 撤掉）
watch(skin2dUrl, () => {
  noSkin.value = false;
  if (skin2dUrl.value) loading.value = true;
});

// ---- 2D 平移 / 缩放 ----
const viewScale = ref(1);
const viewX = ref(0);
const viewY = ref(0);
const MIN_SCALE = 0.2;
const MAX_SCALE = 8;
/** 平移拖拽中（光标形态切换用） */
const panning = ref(false);

/** 回到初始视图：缩放 1、居中 */
function resetView() {
  viewScale.value = 1;
  viewX.value = 0;
  viewY.value = 0;
}

/** 滚轮缩放：以光标为缩放中心 */
function onWheel(e: WheelEvent) {
  e.preventDefault();
  const stageEl = stage.value;
  if (!stageEl) return;
  const factor = e.deltaY < 0 ? 1.15 : 1 / 1.15;
  const next = Math.min(MAX_SCALE, Math.max(MIN_SCALE, viewScale.value * factor));
  const applied = next / viewScale.value;
  const rect = stageEl.getBoundingClientRect();
  const cx = e.clientX - rect.left - rect.width / 2;
  const cy = e.clientY - rect.top - rect.height / 2;
  viewX.value = cx - applied * (cx - viewX.value);
  viewY.value = cy - applied * (cy - viewY.value);
  viewScale.value = next;
}

let lastPointer: { x: number; y: number } | null = null;

function onPointerDown(e: PointerEvent) {
  if (mode.value !== "2d") return;
  panning.value = true;
  lastPointer = { x: e.clientX, y: e.clientY };
  (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
}

function onPointerMove(e: PointerEvent) {
  if (!panning.value || !lastPointer) return;
  viewX.value += e.clientX - lastPointer.x;
  viewY.value += e.clientY - lastPointer.y;
  lastPointer = { x: e.clientX, y: e.clientY };
}

function onPointerUp() {
  panning.value = false;
  lastPointer = null;
}

// 模式切换 / 换图时回到初始视图
watch([mode, skin2dUrl], resetView);

onMounted(async () => {
  base.value = await getImageBaseUrl();
  setupViewer();
});

onBeforeUnmount(destroyViewer);
</script>

<template>
  <BaseModal :title="t('account.viewSkin')" :width="780" fixed-height="520px" below-titlebar @close="emit('close')">
    <div class="skin-layout">
      <!-- 预览区：2D / 3D 切换在展示区上方 -->
      <div class="preview">
        <!-- 2D / 3D 分段切换（统一 SegmentedTabs） -->
        <SegmentedTabs class="mode-toggle" :model-value="mode" :options="MODE_OPTIONS"
          @update:model-value="mode = $event as Mode" />

        <div ref="stage" class="preview-stage" :class="{ pan: mode === '2d', grabbing: panning }"
          @wheel.prevent="onWheel" @pointerdown="onPointerDown" @pointermove="onPointerMove" @pointerup="onPointerUp"
          @pointercancel="onPointerUp" @dblclick="resetView">
          <canvas v-show="mode === '3d'" ref="canvas" class="gl-canvas"></canvas>
          <div v-if="mode === '2d'" class="pan-wrap"
            :style="{ transform: `translate(${viewX}px, ${viewY}px) scale(${viewScale})` }">
            <img v-if="skin2dUrl" :src="skin2dUrl" class="flat-skin" draggable="false" @load="loading = false"
              @error="noSkin = true, (loading = false)" />
            <img v-if="showCape && cape2dUrl" :src="cape2dUrl" class="flat-cape" draggable="false" alt="" />
          </div>
          <div v-if="(loading || (listLoading && !selectedSkin)) && !noSkin" class="preview-loading">
            <span class="spinner"></span>
          </div>
          <p v-if="noSkin" class="preview-tip">{{ t("winSkin.noSkin") }}</p>
        </div>
        <p v-if="mode === '3d' && !noSkin" class="stage-hint">{{ t("winSkin.hint") }}</p>
      </div>

      <!-- 右侧面板：皮肤类型 + 控制器 + 纹理列表 -->
      <div class="skin-side">
        <label class="field-label first">{{ t("winSkin.skinType") }}</label>
        <select v-model="skinType" class="field-select" @change="skinTypeTouched = true">
          <option v-for="opt in SKIN_TYPES" :key="opt.value" :value="opt.value">
            {{ t(opt.labelKey) }}
          </option>
        </select>

        <h2 class="info-title ctl-title">{{ t("winSkin.controls") }}</h2>
        <div class="ctl-row">
          <span>{{ t("winSkin.showCape") }}</span>
          <BaseSwitch v-model="showCape" />
        </div>
        <!-- 动画 / 自转 / 名牌只在 3D 有意义 -->
        <template v-if="mode === '3d'">
          <label class="field-label">{{ t("winSkin.animation") }}</label>
          <select v-model="animType" class="field-select">
            <option v-for="opt in ANIM_TYPES" :key="opt.value" :value="opt.value">
              {{ t(opt.labelKey) }}
            </option>
          </select>
          <div class="ctl-row">
            <span>{{ t("winSkin.autoRotate") }}</span>
            <BaseSwitch v-model="autoRotate" />
          </div>
          <div class="ctl-row">
            <span>{{ t("winSkin.showNametag") }}</span>
            <BaseSwitch v-model="showNametag" />
          </div>
        </template>

        <!-- 纹理列表：点击切换预览，正版可设为装备 -->
        <h2 class="info-title ctl-title list-head">
          {{ t("account.skinsList") }}
          <BaseButton v-if="isOauth && base" variant="accent" size="sm" :disabled="listLoading || uploading"
            @click="pickSkinFile">
            {{ t("account.uploadSkin") }}
          </BaseButton>
          <!-- 第三方：皮肤上传 / 装备在皮肤站网页面板完成 -->
          <BaseButton v-if="isThirdParty" variant="accent" size="sm" :disabled="listLoading" @click="openSkinSite">
            {{ t("account.openSkinSite") }}
          </BaseButton>
        </h2>
        <!-- 待上传文件：选型号直传 -->
        <div v-if="uploadFile" class="tex-item upload-row">
          <span class="tex-name" v-tip="uploadFile">
            {{ uploadFile.split("\\").pop()?.split("/").pop() }}
          </span>
          <BaseButton size="sm" :disabled="uploading" @click="doUpload('classic')">
            {{ t("winSkin.skinClassic") }}
          </BaseButton>
          <BaseButton size="sm" :disabled="uploading" @click="doUpload('slim')">
            {{ t("winSkin.skinSlim") }}
          </BaseButton>
          <BaseButton variant="ghost" size="sm" :disabled="uploading" @click="uploadFile = ''">
            <GlyphIcon name="close" :size="14" :weight="2.2" />
          </BaseButton>
        </div>
        <div class="tex-list">
          <div v-for="item in textures?.skins ?? []" :key="item.sha1" class="tex-item"
            :class="{ selected: item.sha1 === previewSkinSha1 }" @click="previewSkinSha1 = item.sha1">
            <img :src="skinThumbUrl(item)" class="tex-thumb skin" draggable="false" alt="" />
            <div class="tex-meta">
              <span class="tex-name" v-tip="item.name">{{ item.name }}</span>
              <span class="tex-model">{{ modelLabel(item.model) }}</span>
            </div>
            <BaseButton v-if="isOauth" size="sm" :disabled="item.active || equipBusy === item.sha1"
              @click.stop="equip('skin', item)">
              {{ item.active ? t("account.equipped") : t("account.equip") }}
            </BaseButton>
          </div>
          <p v-if="listLoading" class="tex-empty">{{ t("account.loadingTextures") }}</p>
          <p v-else-if="!(textures?.skins ?? []).length" class="tex-empty">{{ t("winSkin.noSkin") }}</p>
        </div>

        <h2 class="info-title ctl-title">{{ t("account.capesList") }}</h2>
        <div class="tex-list">
          <div v-for="item in textures?.capes ?? []" :key="item.sha1" class="tex-item"
            :class="{ selected: item.sha1 === previewCapeSha1 }" @click="previewCapeSha1 = item.sha1">
            <img :src="capeThumbUrl(item)" class="tex-thumb cape" draggable="false" alt="" />
            <div class="tex-meta">
              <span class="tex-name" v-tip="item.name">{{ item.name }}</span>
            </div>
            <BaseButton v-if="isOauth" size="sm" :disabled="item.active || equipBusy === item.sha1"
              @click.stop="equip('cape', item)">
              {{ item.active ? t("account.equipped") : t("account.equip") }}
            </BaseButton>
          </div>
          <p v-if="listLoading" class="tex-empty">{{ t("account.loadingTextures") }}</p>
          <p v-else-if="!(textures?.capes ?? []).length" class="tex-empty">{{ t("account.noCape") }}</p>
        </div>
      </div>
    </div>
  </BaseModal>
</template>

<style scoped>
.skin-layout {
  display: flex;
  gap: 18px;
  flex-wrap: wrap;
}

.preview {
  flex: 1;
  min-width: 320px;
}

/* SegmentedTabs（2D / 3D）下方留缝 */
.mode-toggle {
  margin-bottom: 12px;
}

.preview-stage {
  height: 380px;
  background: var(--bg-card);
  border: 1px solid var(--border);
  border-radius: 14px;
  display: flex;
  align-items: center;
  justify-content: center;
  position: relative;
  overflow: hidden;
}

.gl-canvas {
  width: 100%;
  height: 100%;
}

/* 2D 平移 / 缩放：拖拽光标 + 变换容器 */
.preview-stage.pan {
  cursor: grab;
  touch-action: none;
}

.preview-stage.pan.grabbing {
  cursor: grabbing;
}

.pan-wrap {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 100%;
  height: 100%;
  will-change: transform;
}

.flat-skin {
  max-width: 70%;
  max-height: 100%;
  image-rendering: pixelated;
}

.flat-cape {
  max-width: 25%;
  max-height: 60%;
  image-rendering: pixelated;
}

.preview-tip {
  position: absolute;
  bottom: 16px;
  left: 0;
  right: 0;
  text-align: center;
  font-size: 12px;
  color: var(--text-dim);
}

/* 加载指示：贴图 / 模型未就绪时盖在预览区中央 */
.preview-loading {
  position: absolute;
  inset: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  pointer-events: none;
}

.spinner {
  width: 28px;
  height: 28px;
  border: 3px solid var(--border);
  border-top-color: var(--text-dim);
  border-radius: 50%;
  animation: skin-spin 0.8s linear infinite;
}

@keyframes skin-spin {
  to {
    transform: rotate(360deg);
  }
}

.stage-hint {
  margin-top: 8px;
  font-size: 12px;
  color: var(--text-dim);
}

/* 滚动只发生在右侧列表面板内部：弹窗体高度已被 fixed-height 锁死，
   不会再有第二条滚动条；预览区不会跟着滚走 */
.skin-side {
  width: 260px;
  max-height: 430px;
  overflow-y: auto;
  scrollbar-gutter: stable;
  /* 行与滚动条之间留 8px：滚动容器向外借 8px、再用内边距收回 ——
     行不往左缩（槽位由上面的 scrollbar-gutter 常驻），滚动条画在这 8px 上
     （与项目列表 .res-list 同一做法，见 styles/scrollbar.css） */
  margin-right: -8px;
  padding-right: 8px;
}

.info-title {
  font-size: 14px;
  font-weight: 700;
  margin-bottom: 10px;
}

/* 面板顶部第一个标签不用给上边距 */
.field-label.first {
  margin-top: 0;
}

/* 控制器区：披风 / 动画 / 自转 / 名牌 */
.ctl-title {
  margin-top: 18px;
}

.ctl-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  font-size: 13px;
  color: var(--text);
  padding: 7px 0;
}

/* 纹理列表 */
.tex-list {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

/* 列表标题行：标题 + 上传按钮 */
.list-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
}

/* 待上传文件行 */
.upload-row {
  cursor: default;
}

.tex-item {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 6px 8px;
  background: var(--bg-card);
  border: 1px solid var(--border);
  border-radius: 10px;
  cursor: pointer;
}

.tex-item.selected {
  border-color: var(--accent, #7c6cf6);
}

.tex-thumb {
  flex: none;
  image-rendering: pixelated;
  object-fit: contain;
  background: var(--bg);
}

.tex-thumb.skin {
  width: 26px;
  height: 44px;
}

.tex-thumb.cape {
  width: 40px;
  height: 44px;
}

.tex-meta {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.tex-name {
  font-size: 12px;
  color: var(--text);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.tex-model {
  font-size: 11px;
  color: var(--text-dim);
}

.tex-empty {
  font-size: 12px;
  color: var(--text-dim);
  padding: 4px 2px;
}
</style>
