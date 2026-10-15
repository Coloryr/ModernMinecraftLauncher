<script setup lang="ts">
// NBT 树上的一个节点：复合递归成员、列表与数组递归元素，自己也是个可点开的行
//
// 每行是"键 | 值"两栏，中间一条竖线；分栏比例固定，缩进只作用在键列，所以各级的竖线
// 都对齐在同一条线上，哪个值属于哪个键不用猜。
//
// 标量就地改：点一下变输入框，回车或失焦提交，Esc 撤掉。复合、列表与数组自己没有值，
// 只能展开看成员 / 元素、往里加、或整行删掉 —— 数组的元素不是独立标签，但在界面上和
// 列表一样是一行一个下标，好改也好数。行尾悬停出现 ＋（加子项）与 ✕（删这一行）。
// 所有改动都只是登记到树上的登记表里（见 nbtEdit），落盘统一等标题栏的保存键。
//
// 搜索时不看手动展开状态：命中的分支一律铺开，否则搜到的东西藏在收起的分支里看不见。
import { computed, inject, nextTick, ref } from "vue";
import type { NbtKindDto } from "../../../lib/bindings";
import { t } from "../../../lib/i18n";
import { showToast } from "../../../lib/toast";
import GlyphIcon from "../../../components/ui/GlyphIcon.vue";
import {
  ADDABLE_KINDS,
  MAX_ARRAY_EDIT,
  NBT_EDIT,
  elementKindOf,
  isArrayKind,
  isBranchKind,
  isScalarKind,
  kidsOf,
  pathKey,
  valueOk,
  type NbtTreeNode,
} from "./nbtEdit";

defineOptions({ name: "NbtNode" });

const props = defineProps<{
  /** 本节点 */
  node: NbtTreeNode;
  /** 缩进层级，从 0 起；前两层默认展开 */
  depth: number;
  /** 从根到本节点的路径：复合成员是键名、列表与数组的元素是下标字符串，根节点是空数组 */
  path: string[];
  /** 搜索命中的路径集合，由树一次算好；null = 没有搜索，不过滤 */
  hits?: Set<string> | null;
  /** 改本节点键名的回调；只有复合的成员给，列表 / 数组元素按序号认，根标签没有键名 */
  renameSelf?: (name: string) => void;
  /** 删掉本节点；根节点没有父容器，不给这个回调，行尾也就不显示 ✕ */
  removeSelf?: () => void;
}>();

/** 复合、列表与没超上限的数组才有下钻 */
const isBranch = computed(() => isBranchKind(props.node.kind, props.node.length));
/** 是不是列表：加元素不用键名，类型也由元素定 */
const isList = computed(() => props.node.kind === "List");
/** 是不是复合：只有它的成员才有键名可改 */
const isCompound = computed(() => props.node.kind === "Compound");
/** 是不是数组：元素在界面上与列表一样，但类型随数组自己 */
const isArray = computed(() => isArrayKind(props.node.kind));
/** 元素按序号排的容器（列表与数组）：加子项不用键名 */
const anonymous = computed(() => isList.value || isArray.value);

/** 登记表；没有它（脱离 NbtTree 单用）时整个树按只读处理 */
const edit = inject(NBT_EDIT, null);

// 展开状态记在会话里（见 nbtSession）：切走再切回来还是这一份样子，不重新收起来
/** 用户手动定过的展开状态；没定过就是 undefined */
const opened = computed(() => edit?.openedOf(props.path));
/** 展开状态：定过看定过的，没定过按默认档（前两层展开） */
const open = computed(() => opened.value ?? props.depth < 2);

/** 下钻的子节点：复合用 `children`，列表与数组用 `items` */
const below = computed(() => kidsOf(props.node));
/** 键列的缩进，单位 px；封顶在 12 层，免得极深的层级把竖线顶出可见区 */
const indent = computed(() => Math.min(props.depth, 12) * 14);

// ---------- 搜索 ----------

/** 自身或后代没命中就整块不画 */
const visible = computed(() => !props.hits || props.hits.has(pathKey(props.path)));
/** 后代里还有命中的：搜索期间这些分支要铺开 */
const hitBelow = computed(
  () => !!props.hits && kidsOf(props.node).some((child) => props.hits?.has(pathKey([...props.path, child.name]))),
);
const showChildren = computed(() => isBranch.value && (open.value || hitBelow.value));

// ---------- 值的显示与编辑 ----------

/** 行尾的值：容器（列表与数组）给元素个数，标量给值 */
const valueLabel = computed(() => {
  if (isList.value || isArray.value) return t("config.nbtCount", { n: props.node.length });
  return props.node.value;
});

/** 这一行的值能不能改：只有标量；容器只能增删 */
const editable = computed(() => !!edit && isScalarKind(props.node.kind));
/** 行尾给哪个：空字符串给一对引号，否则这一格是一片空白，既看不见也点不中 */
const shown = computed(() =>
  props.node.kind === "String" && props.node.value === "" ? '""' : valueLabel.value,
);

const editing = ref(false);
const draft = ref("");
const inputEl = ref<HTMLInputElement | null>(null);

/** 点值开输入框：起手是这一行的值，光标选中全部，接着敲就是替换 */
function startEdit() {
  if (!editable.value || editing.value) return;
  draft.value = props.node.value;
  editing.value = true;
  void nextTick(() => inputEl.value?.select());
}

/** 撤销这一行的改动 */
function cancelEdit() {
  editing.value = false;
}

/** 提交输入框：先按类型自检，不让整批保存输在一个明显打错的值上（后端还会再判一次） */
function commitEdit() {
  if (!editing.value || !edit) return;
  editing.value = false;
  if (!valueOk(props.node.kind, draft.value)) {
    showToast(t("err.nbtValue"));
    return;
  }
  // 没变就不记：别让保存键为一个原样值亮起来
  if (draft.value === props.node.value) return;
  edit.set(props.node, props.path, draft.value);
}

/** 点这一行：容器就展开 / 收起，状态记进会话 */
function toggle() {
  if (!isBranch.value) return;
  edit?.setOpened(props.path, !open.value);
}

// ---------- 改键名 ----------

/** 改名输入开着没有 */
const renaming = ref(false);
/** 键名草稿 */
const draftName = ref("");
const nameInputEl = ref<HTMLInputElement | null>(null);

/** 键名那格的提示：能改名的把"双击改键名"附在名字后面 */
const nameTip = computed(() =>
  props.renameSelf ? `${props.node.name}\n${t("config.nbtRename")}` : props.node.name,
);

/** 双击键名开输入框：起手是现在的键名，光标选中全部，接着敲就是替换 */
function startRename() {
  if (!props.renameSelf || renaming.value) return;
  draftName.value = props.node.name;
  renaming.value = true;
  void nextTick(() => nameInputEl.value?.select());
}

/** 撤掉改名 */
function cancelRename() {
  renaming.value = false;
}

/** 提交新键名：空名当场拦住，同名等于没改（重名由树那边判并提示） */
function commitRename() {
  if (!renaming.value || !props.renameSelf) return;
  renaming.value = false;
  const name = draftName.value.trim();
  if (name === "") {
    showToast(t("config.nbtKeyEmpty"));
    return;
  }
  if (name === props.node.name) return;
  props.renameSelf(name);
}

/** 把子节点的改名请求转给登记表：键名归属的是父容器，所以由父这边带上自己的路径与下标 */
function renameChild(index: number, name: string) {
  edit?.rename(props.node, props.path, index, name);
}

// ---------- 增删 ----------

/** 新增行开着没有 */
const adding = ref(false);
/** 新成员的键名，只有复合要 */
const draftKey = ref("");
/** 新标签的类型；列表非空时被元素类型顶掉 */
const draftKind = ref<NbtKindDto>("Int");
/** 新标签的值文本 */
const draftValue = ref("");

/** 列表元素类型：非空列表取第一个元素的，空列表由下拉里选的定 */
const listKind = computed(() => props.node.items[0]?.kind ?? draftKind.value);
/** 这次要加的类型：列表与数组由容器自己定，复合看下拉 */
const addKind = computed<NbtKindDto>(() => {
  if (isArray.value) return elementKindOf(props.node.kind) ?? "Int";
  if (isList.value) return listKind.value;
  return draftKind.value;
});
/** 还要不要填值：复合与列表是空容器，没有值 */
const needsValue = computed(() => addKind.value !== "Compound" && addKind.value !== "List");
/** 类型锁死：数组由数组自己定，列表非空由元素定 */
const kindLocked = computed(() => isArray.value || (isList.value && props.node.items.length > 0));
/** 还能不能加：数组到编辑上限就不再给 ＋ */
const canAdd = computed(
  () => isBranch.value && (!isArray.value || props.node.length < MAX_ARRAY_EDIT),
);

/** 开新增行：顺带铺开这一层，不然草稿行藏在收起的分支里 */
function startAdd() {
  adding.value = true;
  edit?.setOpened(props.path, true);
  draftKey.value = "";
  draftValue.value = "";
  // 类型固定的容器（数组、非空列表）下拉是禁用的，得先把值摆对，不然显示的是上一次选的那个
  const fixed = elementKindOf(props.node.kind) ?? props.node.items[0]?.kind;
  if (fixed) draftKind.value = fixed;
}

/** 确认新增：先自检键名与值，再交给登记表 */
function confirmAdd() {
  if (!edit) return;
  const kind = addKind.value;
  const key = draftKey.value.trim();
  if (!anonymous.value && key === "") {
    showToast(t("config.nbtKeyEmpty"));
    return;
  }
  if (!anonymous.value && props.node.children.some((child) => child.name === key)) {
    showToast(t("config.nbtKeyExists"));
    return;
  }
  if (needsValue.value && !valueOk(kind, draftValue.value)) {
    showToast(t("err.nbtValue"));
    return;
  }
  edit.add(props.node, props.path, key, kind, needsValue.value ? draftValue.value : "");
  adding.value = false;
  draftKey.value = "";
  draftValue.value = "";
}

/** 撤掉新增行 */
function cancelAdd() {
  adding.value = false;
}

// ---------- 类型徽章的着色 ----------

/** 数字家族（含三种数组）、字符串、复合、列表各一色，End 中性 */
const kindClass = computed(() => {
  switch (props.node.kind) {
    case "Byte":
    case "Short":
    case "Int":
    case "Long":
    case "Float":
    case "Double":
    case "ByteArray":
    case "IntArray":
    case "LongArray":
      return "num";
    case "String":
      return "str";
    case "Compound":
      return "compound";
    case "List":
      return "list";
    default:
      return "";
  }
});
</script>

<template>
  <div v-if="visible" class="nbt-node">
    <div class="nbt-row" :class="{ branch: isBranch, edited: node.touched }" @click="toggle">
      <!-- 键列：箭头 + 键名 + 类型。缩进给在这一列里，竖线才能在各级之间对齐 -->
      <div class="nbt-key" :style="{ paddingLeft: `${indent}px` }">
        <span class="nbt-caret" :class="{ open: showChildren }">
          <GlyphIcon v-if="isBranch" name="chevron-down" :size="12" :weight="2.4" />
        </span>
        <input v-if="renaming" ref="nameInputEl" v-model="draftName" class="nbt-input" spellcheck="false"
          @click.stop @dblclick.stop @keydown.enter.prevent="commitRename" @keydown.esc="cancelRename"
          @blur="commitRename" />
        <span v-else class="nbt-name" :class="{ renamable: !!renameSelf }" v-tip="nameTip"
          @dblclick.stop="startRename">{{ node.name }}</span>
        <span class="nbt-kind" :class="kindClass">{{ node.kind }}</span>
      </div>
      <!-- 值列：与键列之间由一条竖线分开，谁属于谁一眼看得出来 -->
      <div class="nbt-val">
        <!-- 输入框吞掉点击：别让点它把行当成"展开 / 收起" -->
        <input v-if="editing" ref="inputEl" v-model="draft" class="nbt-input" spellcheck="false" @click.stop
          @keydown.enter.prevent="commitEdit" @keydown.esc="cancelEdit" @blur="commitEdit" />
        <!-- 不 stop：值列也属于整行，点分支的个数那一格同样该展开 / 收起 -->
        <span v-else class="nbt-value" :class="{ editable }" v-tip="editable ? t('config.nbtEdit') : ''"
          @click="startEdit">{{ shown }}</span>
        <!-- 行尾操作：悬停才出现，不占常驻宽度；点它们不能顺带展开 / 收起这一行 -->
        <span class="nbt-acts">
          <button v-if="canAdd" class="nbt-act" v-tip="t('config.nbtAddChild')"
            :aria-label="t('config.nbtAddChild')" @click.stop="startAdd">
            <GlyphIcon name="plus" :size="12" :weight="2.2" />
          </button>
          <button v-if="removeSelf" class="nbt-act" v-tip="t('config.nbtRemove')" :aria-label="t('config.nbtRemove')"
            @click.stop="removeSelf">
            <GlyphIcon name="close" :size="12" :weight="2.2" />
          </button>
        </span>
      </div>
    </div>

    <div v-if="isBranch && showChildren">
      <!-- 新增行贴在这一层的第一个子行位置：不用先滚到底就能填 -->
      <div v-if="adding" class="nbt-add" :style="{ paddingLeft: `${indent + 14}px` }">
        <input v-if="!anonymous" v-model="draftKey" class="nbt-input nbt-add-key" spellcheck="false"
          :placeholder="t('config.nbtNewKey')" @keydown.enter.prevent="confirmAdd" @keydown.esc="cancelAdd" />
        <select v-model="draftKind" class="field-select nbt-add-kind" :disabled="kindLocked"
          :aria-label="t('config.nbtNewKind')" v-tip="t('config.nbtNewKind')">
          <option v-for="kind in ADDABLE_KINDS" :key="kind" :value="kind">{{ kind }}</option>
        </select>
        <input v-if="needsValue" v-model="draftValue" class="nbt-input nbt-add-value" spellcheck="false"
          :placeholder="t('config.nbtNewValue')" @keydown.enter.prevent="confirmAdd" @keydown.esc="cancelAdd" />
        <button class="nbt-act ok" v-tip="t('config.nbtAddConfirm')" :aria-label="t('config.nbtAddConfirm')"
          @click="confirmAdd">
          <GlyphIcon name="check" :size="12" :weight="2.4" />
        </button>
        <button class="nbt-act" v-tip="t('config.nbtAddCancel')" :aria-label="t('config.nbtAddCancel')"
          @click="cancelAdd">
          <GlyphIcon name="close" :size="12" :weight="2.2" />
        </button>
      </div>

      <NbtNode v-for="(child, index) in below" :key="`${child.name}-${index}`" :node="child" :depth="depth + 1"
        :path="[...path, child.name]" :hits="hits" :remove-self="() => edit?.remove(node, path, index)"
        :rename-self="isCompound ? (name: string) => renameChild(index, name) : undefined" />
    </div>
  </div>
</template>

<style scoped>
/* 键值两栏：左键右值，中间一条竖线；行高写死，点开输入框、加子节点都不会把行撑高。
   分栏比例固定，所有行的竖线才对得上 —— 缩进只作用在键列里 */
.nbt-row {
  display: grid;
  grid-template-columns: minmax(0, 42%) minmax(0, 1fr);
  align-items: stretch;
  height: 22px;
  border-radius: 5px;
  font-size: 12.5px;
  font-family: ui-monospace, "Cascadia Mono", Consolas, monospace;
}

.nbt-row.branch {
  cursor: pointer;
}

/* 整行都要有悬停底：行尾那两颗操作键压着一层同色渐变，行本身没有底就会糊出一块灰 */
.nbt-row:hover {
  background: var(--bg-hover);
}

/* 键列：每行只画自己那一段分隔线，行挨着行就连成一条 */
.nbt-key {
  display: flex;
  align-items: center;
  gap: 6px;
  min-width: 0;
  padding-right: 8px;
  border-right: 1px solid var(--border);
}

/* 箭头：收起来朝右、展开朝下，与配置窗口左栏的文件树同一套画法（SVG 图标 + 旋转）。
   叶子节点留同宽的空格，同一层的键名才与分支行对齐 */
.nbt-caret {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 12px;
  flex-shrink: 0;
  color: var(--text-dim);
  transform: rotate(-90deg);
  transition: transform 0.12s;
}

.nbt-caret.open {
  transform: rotate(0);
}

/* 键名可被挤压：长名字截断，类型徽章始终贴在竖线一侧 */
.nbt-name {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--text);
}

/* 可改名的键名：双击改，光标给文本型 —— 这一格的"能编辑"就靠它和提示说 */
.nbt-name.renamable {
  cursor: text;
}

.nbt-kind {
  flex-shrink: 0;
  margin-left: auto;
  padding: 0 5px;
  border-radius: 4px;
  background: var(--bg-hover);
  color: var(--text-dim);
  font-size: 10.5px;
}

/* 类型徽章按类型上色：数字家族（含三种数组）/ 字符串 / 复合 / 列表各一色，End 保持中性灰。
   底色取同色的浅淡版（currentColor 就是上面那条 color），色值固定不跟强调色走。
   值本身是正文亮色，类型只在这里体现 */
.nbt-kind.num {
  color: var(--nbt-num);
}

.nbt-kind.str {
  color: var(--nbt-str);
}

.nbt-kind.compound {
  color: var(--nbt-compound);
}

.nbt-kind.list {
  color: var(--nbt-list);
}

.nbt-kind.num,
.nbt-kind.str,
.nbt-kind.compound,
.nbt-kind.list {
  background: color-mix(in srgb, currentColor 16%, transparent);
}

/* 值列：操作按钮要绝对定位在右端，这一格因此是定位上下文 */
.nbt-val {
  position: relative;
  display: flex;
  align-items: center;
  min-width: 0;
  padding-left: 8px;
}

/* 值可被挤压；颜色用正文亮色，与键同亮 —— 键与值的分界交给中间那条竖线，不靠灰度差 */
.nbt-value {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--text);
}

/* 可改的值：悬停给下划线，明确"这一格能点" */
.nbt-value.editable {
  cursor: text;
}

.nbt-value.editable:hover {
  text-decoration: underline dotted;
  text-underline-offset: 3px;
}

/* 改过的行：值与未保存提示同一个黄色，扫一眼就知道哪几处待保存 */
.nbt-row.edited .nbt-value {
  color: var(--yellow);
}

/* 高度必须与行内其它东西一致：18 + 两侧各 2 的余量就是整行 22 */
.nbt-input {
  flex: 1;
  min-width: 0;
  box-sizing: border-box;
  height: 18px;
  padding: 0 4px;
  border: 1px solid var(--accent-border);
  border-radius: 4px;
  background: var(--bg-solid, var(--bg-card));
  color: var(--text);
  font-family: inherit;
  font-size: inherit;
  line-height: 16px;
  outline: none;
}

/* 行尾操作：悬停这一行才出现。绝对定位，不占宽度，值多长都不会被顶开 */
.nbt-acts {
  position: absolute;
  top: 0;
  right: 0;
  bottom: 0;
  display: flex;
  align-items: center;
  gap: 2px;
  padding-left: 6px;
  opacity: 0;
  /* 按钮压住的值会被抠掉一块，底色补上才不显得悬空 */
  background: linear-gradient(to right, transparent, var(--bg-hover) 40%);
  pointer-events: none;
}

.nbt-row:hover .nbt-acts,
.nbt-row:focus-within .nbt-acts {
  opacity: 1;
  pointer-events: auto;
}

.nbt-act {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 16px;
  height: 16px;
  padding: 0;
  border: none;
  border-radius: 4px;
  background: transparent;
  color: var(--text-dim);
  cursor: pointer;
}

.nbt-act:hover {
  background: var(--bg-raised);
  color: var(--text);
}

.nbt-act.ok:hover {
  color: var(--green);
}

/* 新增行：与树行同高，缩进对齐这一层的子行 */
.nbt-add {
  display: flex;
  align-items: center;
  gap: 6px;
  height: 22px;
  padding-right: 8px;
}

/* 挂 .field-select 只覆盖尺寸，外观 / 箭头 / 弹层都还在 controls.css 那一份。
   两处必须一起改：
   - `min-height`：那边为了"没有占位项的下拉"定死了 `--ctl-h`（32px），只改 height 的话
     下拉实际还是 32px，塞进 22px 的树行里会把整行顶爆、压住上下两行；
   - `line-height`：那边定死 20px，比这一档控件的内容盒还高，文字会被挤偏。 */
.nbt-add-kind {
  flex: 0 0 auto;
  box-sizing: border-box;
  width: 108px;
  height: 18px;
  min-height: 0;
  padding: 0 18px 0 5px;
  background-position: right 4px center;
  background-size: 10px;
  font-size: 11px;
  line-height: 16px;
}

.nbt-add-key {
  flex: 0 1 30%;
}

.nbt-add-value {
  flex: 1 1 auto;
}
</style>
