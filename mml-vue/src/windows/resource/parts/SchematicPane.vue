<script setup lang="ts">
// 结构文件列表：打开目录、删除
//
// 名字有两层，别混：
// - **标题**：NBT 元数据里的 `Name`（作者自己起的名字），没有就退回文件名；
// - **副标题**：**文件名**（磁盘上的那个，删除 / 打开目录都按它走）+ 作者 · 尺寸 ·
//   方块数 · 方块种类 + 描述。
//
// 之所以两个都要：`Name` 与文件名经常不一致（作者改了名没改文件，或者反过来），
// 而用户要找的是磁盘上那个文件 —— 只显示 `Name` 时根本对不上文件夹里看到的东西
// （与光影包列表同一口径，见 ShaderPane 的 sub）。
//
// `author` / `description` **只有 litematic 会填**，它的 `Metadata` 里有这两个字段，
// `.schem` / `.nbt` 那两个 reader 不设，所以那两种格式这两段是空的、整段不渲染。
import { t } from "../../../lib/i18n";
import { deleteSchematic } from "../../../lib/api";
import ContentHead from "./ContentHead.vue";
import FormattedText from "../../../components/ui/FormattedText.vue";
import ListSkeleton from "./ListSkeleton.vue";
import ResourceRow from "./ResourceRow.vue";
import type { useResourceData } from "../composables/useResourceData";
import type { useResourceOps } from "../composables/useResourceOps";
import type { SchematicItemDto } from "../../../lib/bindings";

const props = defineProps<{
  /** 数据层，结构文件列表与加载态从这里取 */
  data: ReturnType<typeof useResourceData>;
  /** 共用操作层，确认弹窗与打开目录从这里取 */
  ops: ReturnType<typeof useResourceOps>;
}>();

const { schematics, instanceUuid, loading } = props.data;
const { busy, askDelete, openFolder } = props.ops;

function remove(item: SchematicItemDto) {
  askDelete(titleOf(item), () => deleteSchematic(instanceUuid.value, item.file));
}

/** 标题：NBT 里的名字，没有就用文件名 */
function titleOf(item: SchematicItemDto): string {
  return item.name || item.file;
}

/**
 * 副标题前半段：文件名 · 作者 · 尺寸 · 方块数 · 方块种类
 *
 * 文件名放**最前**：`.item-sub` 是单行 + 省略号，放最后会被长作者名 / 长尺寸挤掉，
 * 那就等于没显示。标题已经就是文件名时（没 `Name`）不再重复一遍
 */
function metaLabel(item: SchematicItemDto): string {
  return [
    titleOf(item) === item.file ? "" : item.file,
    item.author,
    t("resource.dims", { w: item.width, h: item.height, l: item.length }),
    t("resource.blockCount", { count: item.blockCount }),
    t("resource.blockTypes", { count: item.blockTypes }),
  ]
    .filter(Boolean)
    .join(" · ");
}
</script>

<template>
  <ContentHead :data="data">
  </ContentHead>

  <div v-if="loading" class="item-list">
    <ListSkeleton />
  </div>
  <div v-else class="item-list">
    <ResourceRow v-for="item in schematics" :key="item.file" :name="titleOf(item)">
      <template #badges>
        <span class="badge badge-dim">{{ item.typeName }}</span>
        <span v-if="item.fail" class="badge badge-red">{{ t("resource.modFail") }}</span>
      </template>
      <!--
        描述单独铺在后面、且走 FormattedText：litematic 的 `Metadata.Description` 是作者
        自己写的一句话，可能带 `§` 格式码，与材质包 / 数据包那两处同一套做法。
        只有 litematic 会填它，其余格式为空、整段不渲染
      -->
      <template #sub>
        <span>{{ metaLabel(item) }}</span>
        <template v-if="item.description"> · </template>
        <FormattedText v-if="item.description" :text="item.description" />
      </template>
      <template #actions>
        <button class="mini-btn" :disabled="busy" @click="openFolder('schematics', item.file)">
          {{ t("resource.openFolder") }}
        </button>
        <button class="mini-btn danger" :disabled="busy" @click="remove(item)">
          {{ t("resource.delete") }}
        </button>
      </template>
    </ResourceRow>
    <div v-if="!schematics.length" class="empty-tip">{{ t("resource.empty") }}</div>
  </div>
</template>
