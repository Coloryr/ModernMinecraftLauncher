<script setup lang="ts">
// 设置窗口 · Java 标签：手动添加 / 扫描 / 扫描目录 / 压缩包导入 / 下载 / 列表
import { ref } from "vue";
import { t } from "../../../../lib/i18n";
import { useModalKeys } from "../../../../composables/useModalKeys";
import BaseButton from "../../../../components/ui/BaseButton.vue";
import BaseModal from "../../../../components/ui/BaseModal.vue";
import CollapsePanel from "../../../../components/ui/CollapsePanel.vue";
import GlyphIcon from "../../../../components/ui/GlyphIcon.vue";
import RowMenu from "../../../../components/ui/RowMenu.vue";
import SettingsGroup from "../SettingsGroup.vue";
import type { useSettingsJava } from "../../composables/useSettingsJava";

const props = defineProps<{
  /** Java 标签的状态与操作方法，来自 useSettingsJava */
  settings: ReturnType<typeof useSettingsJava>;
  /** 需要高亮脉冲的分组 id（搜索命中） */
  flashGroup?: string;
}>();

const {
  inTauri,
  javaList,
  javaGroups,
  toggleVendor,
  vendorOpen,
  scanning,
  scanningDir,
  newJavaName,
  newJavaPath,
  addingJava,
  browseJava,
  addJava,
  removeJava,
  removeJavaGroup,
  removeAllJava,
  scanJava,
  scanJavaDir,
  importJavaArchive,
  importingJava,
  importProgress,
  importPercent,
  confirmRemoveAll,
  downloadJava,
} = props.settings;

/** 分组右键菜单：位置 + 该分组下所有 Java 的名字 */
const groupMenu = ref<{ x: number; y: number; vendor: string; names: string[] } | null>(null);

/** 待确认删除的分组；确认后逐个删掉 */
const confirmRemoveGroup = ref<{ vendor: string; names: string[] } | null>(null);

// 删除确认框：Esc = 取消（分组确认优先，其次才是"删除全部"）
useModalKeys((e) => {
  if (e.key !== "Escape") return;
  if (confirmRemoveGroup.value) confirmRemoveGroup.value = null;
  else if (confirmRemoveAll.value) confirmRemoveAll.value = false;
});

/**
 * 在发行商分组标题上右键
 *
 * - `e`: 右键事件，用来取光标位置
 * - `group`: 该分组，菜单里的删除动作用它的名字与条目
 */
function onGroupMenu(e: MouseEvent, group: { vendor: string; items: { name: string }[] }) {
  e.preventDefault();
  groupMenu.value = {
    x: e.clientX,
    y: e.clientY,
    vendor: group.vendor,
    names: group.items.map((j) => j.name),
  };
}

/** 菜单选择：目前只有"删除分组" */
function onGroupMenuPick(id: string) {
  const menu = groupMenu.value;
  groupMenu.value = null;
  if (!menu || id !== "removeGroup") return;
  confirmRemoveGroup.value = { vendor: menu.vendor, names: menu.names };
}

/** 确认删除分组：删完收起确认框 */
async function removeGroup() {
  const target = confirmRemoveGroup.value;
  if (!target) return;
  confirmRemoveGroup.value = null;
  await removeJavaGroup(target.names);
}
</script>

<template>
  <template v-if="inTauri">
    <SettingsGroup id="javaAdd" title-key="winSettings.javaAdd" :flash="flashGroup === 'javaAdd'">
      <!-- 手动添加：输入名字和路径 -->
      <div class="java-add-row">
        <input v-model="newJavaName" class="field-input java-add-name" :placeholder="t('winSettings.javaName')"
          spellcheck="false" autocomplete="off" />
        <input v-model="newJavaPath" class="field-input field-grow" :placeholder="t('winSettings.javaPath')"
          spellcheck="false" autocomplete="off" />
        <!-- 这两枚只放图标：文字走 tooltip 与读屏名，输入框才有地方 -->
        <button class="ui-btn-icon" :disabled="scanning || scanningDir" v-tip="t('args.browse')"
          :aria-label="t('args.browse')" @click="browseJava">
          <GlyphIcon name="folder" :size="15" :weight="1.8" />
        </button>
        <button class="ui-btn-icon is-accent"
          :disabled="addingJava || scanning || scanningDir || !newJavaName.trim() || !newJavaPath.trim()"
          v-tip="t('winSettings.javaAdd')" :aria-label="t('winSettings.javaAdd')" @click="addJava">
          <GlyphIcon name="plus" :size="15" :weight="2" />
        </button>
      </div>
      <div class="set-save-row java-add-actions">
        <BaseButton size="sm" :disabled="scanning || scanningDir" @click="scanJava">
          <span v-if="scanning" class="loading-spin sm" />
          {{ scanning ? t("winSettings.javaScanning") : t("winSettings.javaScan") }}
        </BaseButton>
        <BaseButton size="sm" :disabled="scanning || scanningDir" @click="scanJavaDir">
          <span v-if="scanningDir" class="loading-spin sm" />
          {{ scanningDir ? t("winSettings.javaScanning") : t("winSettings.javaScanDir") }}
        </BaseButton>
        <BaseButton size="sm" :disabled="scanning || scanningDir" @click="importJavaArchive">
          {{ t("winSettings.javaImport") }}
        </BaseButton>
        <!-- 下载是这条动作行的主操作，单独推到最右，免得挨着扫描 / 导入被误点 -->
        <BaseButton class="java-download" size="sm" variant="primary" :disabled="scanning || scanningDir"
          @click="downloadJava">
          {{ t("winSettings.javaDownload") }}
        </BaseButton>
      </div>

      <!-- 压缩包导入进度条：解包 / 识别阶段经 settings-java-progress 事件推进 -->
      <div v-if="importingJava || importProgress" class="java-progress">
        <span class="java-progress-label">{{ t("winSettings.javaImporting") }}</span>
        <div class="java-progress-track">
          <div class="java-progress-fill" :class="{ rolling: !importProgress?.total }"
          :style="importProgress?.total ? { width: importPercent + '%' } : undefined" />
        </div>
        <span class="java-progress-text">{{ importProgress ? `${importProgress.now}/${importProgress.total}` : ""
        }}</span>
        <span v-if="importProgress?.subText" class="java-progress-sub" v-tip="importProgress.subText">{{
          importProgress.subText }}</span>
      </div>

      <div v-if="javaList.length === 0" class="java-empty">{{ t("winSettings.javaNone") }}</div>
      <!-- 按发行商分组：每组是一条与实例设置同款的折叠卡片，不再套一层白底面板 -->
      <template v-else>
        <div v-for="g in javaGroups" :key="g.vendor" class="java-type collapse-card">
          <button class="collapse-head" :class="{ merged: vendorOpen(g.vendor) }"
            :aria-expanded="vendorOpen(g.vendor)" @click="toggleVendor(g.vendor)"
            @contextmenu="onGroupMenu($event, g)">
            <span class="java-type-name">{{ g.vendor }}</span>
            <span class="java-count">{{ g.items.length }}</span>
            <GlyphIcon class="collapse-chevron" :class="{ flip: vendorOpen(g.vendor) }" name="chevron-down"
              :size="11" :weight="2.4" />
          </button>
          <CollapsePanel :open="vendorOpen(g.vendor)">
            <div class="java-grid">
              <div v-for="j in g.items" :key="j.name" class="java-card">
                <div class="java-card-top">
                  <span class="java-name" v-tip="j.name">{{ j.name }}</span>
                  <BaseButton size="sm" variant="danger" :disabled="scanning || scanningDir"
                    @click="removeJava(j.name)">
                    {{ t("winSettings.javaRemove") }}
                  </BaseButton>
                </div>
                <span class="java-meta">{{ j.version }} · {{ j.arch }}</span>
                <span class="java-path" v-tip="j.path">{{ j.path }}</span>
              </div>
            </div>
          </CollapsePanel>
        </div>
      </template>

      <!-- 删除全部：放在项目列表最下面，没有 Java 时连按钮都不出现 -->
      <div v-if="javaList.length" class="java-list-actions">
        <BaseButton size="sm" variant="danger" block :disabled="scanning || scanningDir"
          @click="confirmRemoveAll = true">
          {{ t("winSettings.javaRemoveAll") }}
        </BaseButton>
      </div>
    </SettingsGroup>

    <!-- 分组右键菜单：目前只有"删除分组" -->
    <RowMenu v-if="groupMenu" :x="groupMenu.x" :y="groupMenu.y"
      :items="[{ id: 'removeGroup', label: t('winSettings.javaRemoveGroup', { vendor: groupMenu.vendor }), danger: true }]"
      @pick="onGroupMenuPick" @close="groupMenu = null" />

    <!-- 删除分组确认 -->
    <BaseModal v-if="confirmRemoveGroup" :title="t('winSettings.javaRemoveGroup', { vendor: confirmRemoveGroup.vendor })"
      below-titlebar :closable="false"
      @close="confirmRemoveGroup = null">
      <p class="field-desc">
        {{ t("winSettings.javaRemoveGroupConfirm", { vendor: confirmRemoveGroup.vendor, n: confirmRemoveGroup.names.length }) }}
      </p>
      <div class="set-save-row">
        <BaseButton size="sm" @click="confirmRemoveGroup = null">{{ t("actions.cancel") }}</BaseButton>
        <BaseButton size="sm" variant="danger" @click="removeGroup">{{ t("actions.confirm") }}</BaseButton>
      </div>
    </BaseModal>

    <!-- 删除全部确认 -->
    <BaseModal v-if="confirmRemoveAll" :title="t('winSettings.javaRemoveAll')" below-titlebar :closable="false"
      @close="confirmRemoveAll = false">
      <p class="field-desc">{{ t("winSettings.javaRemoveAllConfirm") }}</p>
      <div class="set-save-row">
        <BaseButton size="sm" @click="confirmRemoveAll = false">{{ t("actions.cancel") }}</BaseButton>
        <BaseButton size="sm" variant="danger" @click="removeAllJava">{{ t("actions.confirm") }}</BaseButton>
      </div>
    </BaseModal>

  </template>
  <p v-else class="field-desc">{{ t("winSettings.tauriOnly") }}</p>
</template>
