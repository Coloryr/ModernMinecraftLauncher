<script setup lang="ts">
// 分组右键菜单：重命名 / 转移内容 / 清空分组 / 删除分组 / 启用所有 / 禁用所有 / 全选 / 删除所有模组
//
// 动作**全部抛给父组件**（ModPane）处理 —— 只有它认识分组、模组、确认弹窗与
// "移到分组"弹窗。这里只管画菜单与定位。
//
// 定位与主窗口的右键菜单同一套：`position: fixed` 钉在点击处，渲染后量一次并收进窗口。
// 贴右下角时菜单会被裁掉，实例菜单 6 项约 285px 高，实测过。
// 关闭：点菜单外面 / 按 Esc / 选中任意一项。
import { computed, nextTick, onBeforeUnmount, onMounted, ref } from "vue";
import { t } from "../../../../lib/i18n";

/** 菜单项（父组件按这个分发动作） */
export type GroupMenuAction =
  | "select-all"
  | "rename-group"
  | "delete-group"
  | "enable-all"
  | "disable-all"
  | "move-all"
  | "move-out"
  | "delete-mods";

const props = defineProps<{
  /** 点击处的横坐标，视口坐标 */
  x: number;
  /** 点击处的纵坐标，视口坐标 */
  y: number;
  /** 分组名（确认弹窗里显示） */
  label: string;
  /** 只有自建分组能改名 / 删；状态分组是内核按启用状态分的，改不了也删不掉 */
  custom: boolean;
  /** 组里的模组数：为空时"转移 / 删除所有模组"都没意义，置灰 */
  count: number;
  /**
   * 组里**还有禁用中的模组** → 才给"启用所有"
   *
   * 判据由父组件按组内实际状态算：已启用分组里没有禁用的模组，就不该出现这一项；
   * 识别失败的模组改不了启用状态，不计入。
   */
  canEnable: boolean;
  /** 组里**还有启用中的模组** → 才给"禁用所有" */
  canDisable: boolean;
  /** 组里可多选的模组已全部选中 → 菜单项变"取消全选" */
  allSelected: boolean;
}>();

const emit = defineEmits<{
  /** 选了一个动作，由 ModPane 执行 */
  (e: "action", kind: GroupMenuAction): void;
  /** 收起菜单 */
  (e: "close"): void;
}>();

const el = ref<HTMLElement | null>(null);
const pos = ref({ x: props.x, y: props.y });
const empty = computed(() => props.count === 0);

onMounted(async () => {
  await nextTick();
  const box = el.value?.getBoundingClientRect();
  if (box) {
    const pad = 8;
    pos.value = {
      x: Math.max(pad, Math.min(props.x, window.innerWidth - box.width - pad)),
      y: Math.max(pad, Math.min(props.y, window.innerHeight - box.height - pad)),
    };
  }
  // 捕获阶段监听：点菜单内部时 `el.contains` 会命中，不会误关
  window.addEventListener("pointerdown", onOutside, true);
  window.addEventListener("keydown", onKey, true);
});

onBeforeUnmount(() => {
  window.removeEventListener("pointerdown", onOutside, true);
  window.removeEventListener("keydown", onKey, true);
});

function onOutside(e: PointerEvent) {
  if (!el.value?.contains(e.target as Node)) emit("close");
}

function onKey(e: KeyboardEvent) {
  if (e.key === "Escape") emit("close");
}

/** 选中一项：先抛动作再关菜单，父组件收到动作时菜单还在，方便它自己决定关不关 */
function pick(kind: GroupMenuAction) {
  emit("action", kind);
  emit("close");
}
</script>

<template>
  <div ref="el" class="row-ctx-menu" :style="{ left: `${pos.x}px`, top: `${pos.y}px` }" @pointerdown.stop
    @contextmenu.prevent>
    <!-- 组本身的操作：重命名 → 内容的去处（转移 / 清空）→ 删除分组。
         重命名 / 清空 / 删组只给自建分组；已启用 / 已禁用 / 识别失败这三个状态分组是内核按状态
         分的，改不了名也删不掉。**空分组照样能删** —— 删掉一个空分组正是它最常见的用法 -->
    <button v-if="custom" class="row-ctx-menu-item" @click="pick('rename-group')">
      {{ t("resource.groupRename") }}
    </button>
    <button class="row-ctx-menu-item" :disabled="empty" @click="pick('move-all')">
      {{ t("resource.moveGroupItems") }}
    </button>
    <button v-if="custom" class="row-ctx-menu-item" :disabled="empty" @click="pick('move-out')">
      {{ t("resource.ungroupGroup") }}
    </button>
    <button v-if="custom" class="row-ctx-menu-item danger" @click="pick('delete-group')">
      {{ t("resource.groupDelete") }}
    </button>
    <span class="row-ctx-menu-sep" />

    <!-- 启用 / 禁用**按组内实际状态给**，用户口径是已启用分组只显示"禁用所有"，其它同理：
         组里还有禁用的模组才出现"启用所有"，还有启用的才出现"禁用所有"；
         识别失败分组里的模组改不了状态，两项都不出现。
         分隔线放在条件块内，两项都不出现时不会多出一条空线 -->
    <template v-if="canEnable || canDisable">
      <button v-if="canEnable" class="row-ctx-menu-item" @click="pick('enable-all')">
        {{ t("resource.enableAll") }}
      </button>
      <button v-if="canDisable" class="row-ctx-menu-item" @click="pick('disable-all')">
        {{ t("resource.disableAll") }}
      </button>
      <span class="row-ctx-menu-sep" />
    </template>

    <!-- 全选 / 取消全选：整组加进多选，与表格里分组行的勾选框同一个开关。
         空组没什么可选的，置灰 -->
    <button class="row-ctx-menu-item" :disabled="empty" @click="pick('select-all')">
      {{ t(allSelected ? "resource.deselectAll" : "resource.selectAll") }}
    </button>
    <span class="row-ctx-menu-sep" />

    <button class="row-ctx-menu-item danger" :disabled="empty" @click="pick('delete-mods')">
      {{ t("resource.deleteGroupMods") }}
    </button>
  </div>
</template>
