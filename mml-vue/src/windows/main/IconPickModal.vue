<script setup lang="ts">
// 主窗口 · 修改图标（截图那一步）：把已选图片裁成实例图标
//
// 选图片不在这里——右键菜单「选择图片」直接开系统文件对话框，选到才开这个弹窗，
// 所以本组件只管"显示图片 + 选范围 + 选输出边长 + 应用"。
//
// 落盘在后端 `block_set_icon_area`：按选区裁剪 → 缩放到所选边长 → 写
// `<实例目录>/icon.png` → 把实例配置的 `Icon` 指过去 → 清缓存 + 广播
// `instance-change(edit)` 事件，InstanceIcon 靠它刷新。
//
// 弹窗**不可误关**：遮罩点击与 Esc 都不关闭，只认「取消 / 应用」两个按钮——
// 栽好的选区被一下点没，比多点一次按钮烦人得多。
import { ref, watch } from "vue";
import { t, tErr } from "../../lib/i18n";
import { commands } from "../../lib/bindings";
import { showToast } from "../../lib/toast";
import { KEYS, readNumber, writeNumber } from "../../lib/storage";
import BaseModal from "../../components/ui/BaseModal.vue";
import BaseButton from "../../components/ui/BaseButton.vue";
import IconCropStage from "./IconCropStage.vue";

const props = defineProps<{
  /** 目标实例 */
  uuid: string;
  /** 实例显示名，直接用作弹窗标题 */
  name: string;
  /** 已选中的本地图片路径（由父组件的文件对话框给出） */
  path: string;
}>();

/** 请求关闭弹窗；取消与应用都走它 */
const emit = defineEmits<{ (e: "close"): void }>();

/** 可选输出边长，单位 px：图标最大显示 84px，256 已足够；大档给需要高清图标的场合 */
const SIZES = [64, 128, 256, 512];
/** 记住上次选的边长；纯前端偏好，与主题 / 侧栏一样走本地存储 */
function readSize(): number {
  const n = readNumber(KEYS.iconPickSize, 256);
  return SIZES.includes(n) ? n : 256;
}

const size = ref(readSize());

function setSize(v: number) {
  size.value = v;
  writeNumber(KEYS.iconPickSize, v);
}

/** 预览图 data URL 与**原图**尺寸；选区按原图像素算，后端裁的也是原图 */
const src = ref("");
const imgW = ref(0);
const imgH = ref(0);
/** 选区（原图像素），由截图组件回报 */
const area = ref({ x: 0, y: 0, w: 0, h: 0 });
const reading = ref(false);
const applying = ref(false);

/**
 * 读图给截图区用
 *
 * Tauri 的 `<img>` 读不了本地路径，所以经后端转成 data URL；后端给的是**降采样后的预览**
 * 加原图尺寸，见 `IconSourceDto` —— 预览只用于选范围，裁剪仍按原图坐标做，清晰度不受影响。
 * 不要在这里拿 data URL 再 `new Image()` 量尺寸：那等于在页面里把原图整个解一遍，
 * 大图会把 WebView 卡住（改之前就是这样）。
 */
async function loadSource() {
  reading.value = true;
  src.value = "";
  try {
    const info = await commands.block.readIconSource(props.path);
    src.value = info.src;
    imgW.value = info.width;
    imgH.value = info.height;
  } catch (err) {
    // 读不出来（格式不支持 / 文件被删）就关掉，让用户重新右键选
    showToast(tErr(err));
    emit("close");
  } finally {
    reading.value = false;
  }
}

watch(() => props.path, loadSource, { immediate: true });

/** 应用：按选区裁剪并写盘 */
async function apply() {
  if (!src.value || applying.value || reading.value) return;
  applying.value = true;
  try {
    const a = area.value;
    const ok = await commands.block.setIconArea(
      props.uuid,
      props.path,
      a.x,
      a.y,
      a.w,
      a.h,
      size.value,
    );
    if (ok) {
      showToast(t("iconPick.done"));
      emit("close");
    }
  } catch (err) {
    showToast(tErr(err));
  } finally {
    applying.value = false;
  }
}
</script>

<template>
  <!-- 不可关闭：遮罩点击与 Esc 都不关（见文件头注释），只认下面两个按钮 -->
  <BaseModal :title="name" :closable="false" :width="700">
    <!-- 截图区；读图期间只留一行状态 -->
    <p v-if="reading" class="pick-loading">{{ t("iconPick.picking") }}</p>
    <template v-else-if="src">
      <div class="pick-stage">
        <IconCropStage :src="src" :width="imgW" :height="imgH" @change="area = $event" />
      </div>

      <!-- 输出分辨率：写盘的 icon.png 边长 -->
      <div class="pick-size">
        <span class="pick-size-label">{{ t("iconPick.size") }}</span>
        <div class="pick-size-opts">
          <button v-for="s in SIZES" :key="s" class="pick-size-opt" :class="{ active: s === size }" :disabled="applying"
            @click="setSize(s)">
            {{ s }}px
          </button>
        </div>
      </div>
    </template>

    <div class="modal-actions">
      <BaseButton :disabled="applying || reading" @click="emit('close')">
        {{ t("blocks.cancel") }}
      </BaseButton>
      <BaseButton variant="primary" :disabled="applying || reading || !src" @click="apply">
        {{ applying ? t("iconPick.applying") : t("iconPick.apply") }}
      </BaseButton>
    </div>
  </BaseModal>
</template>

<style scoped>
/* 截图区居中；宽度跟着图片走（组件内已按比例缩到上限以内） */
.pick-stage {
  display: flex;
  justify-content: center;
}

.pick-loading {
  padding: 40px 0;
  font-size: 12.5px;
  color: var(--text-dim);
  text-align: center;
}

/* 输出分辨率：与 SegmentedTabs 同一套视觉（本弹窗只此一处，不额外引组件） */
.pick-size {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  margin-top: 14px;
}

.pick-size-label {
  font-size: 12.5px;
  color: var(--text-dim);
}

/* 轨道用 --bg-side：弹窗底色就是 --bg-card，同色会看不出轨道 */
.pick-size-opts {
  display: inline-flex;
  gap: 4px;
  background: var(--bg-side);
  border: 1px solid var(--border);
  border-radius: 10px;
  padding: 2px;
}

.pick-size-opt {
  /* 与 SegmentedTabs 同一口径：内胆 = --ctl-h − 轨道 2×2 − 边框 1×2 */
  height: calc(var(--ctl-h) - 6px);
  padding: 0 12px;
  border: none;
  border-radius: 7px;
  background: transparent;
  color: var(--text-dim);
  font-family: inherit;
  font-size: 12.5px;
  cursor: pointer;
  transition: all 0.15s;
  white-space: nowrap;
}

.pick-size-opt:hover:not(:disabled) {
  color: var(--text);
}

.pick-size-opt.active {
  background: var(--accent);
  color: #fff;
  font-weight: 600;
}

.pick-size-opt:disabled {
  cursor: default;
  opacity: 0.6;
}
</style>
