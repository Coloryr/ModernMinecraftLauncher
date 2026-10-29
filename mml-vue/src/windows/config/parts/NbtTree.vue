<script setup lang="ts">
// NBT 文件视图：拉整个文件的树，交给 NbtNode 递归画；改动在这里登记与保存
//
// 显示的是**工作树**：值改过、成员增删过都就地改在这棵树上，"改了哪几处"直接看得到。
// 每做一步同时记一条改动（见 nbtEdit 的 [`NbtEditApi`]），标题栏那颗保存键按发生顺序
// 整批发给 `config_set_nbt`；后端按打开时的文件字节 SHA1 核对磁盘上还是不是那一份，
// 再照同一顺序套用到磁盘那一棵树上 —— 两边从同一份出发、走同一串操作，结果自然一致。
//
// 读到的东西挂在**会话**上（见 nbtSession）：切到别的文件再切回来还是同一棵工作树，
// 不重新读盘，没保存的改动也不会丢。只有会话还没读过（`loaded` 为假）时才去读盘。
// 保存成功后重新读一遍，会话就地刷新。
import { computed, provide, ref, watch } from "vue";
import type { NbtEditDto, NbtKindDto } from "../../../lib/bindings";
import { t, tErr } from "../../../lib/i18n";
import { showToast } from "../../../lib/toast";
import { openConfigNbt, writeConfigNbt } from "../../../lib/api";
import SearchBox from "../../../components/ui/SearchBox.vue";
import { KEYS, readNumber, writeNumber } from "../../../lib/storage";
import NbtNode from "./NbtNode.vue";
import { openSession, sessionKey, type NbtSession } from "./nbtSession";
import {
  NBT_EDIT,
  elementKindOf,
  isScalarKind,
  kidsOf,
  pathKey,
  splitArrayText,
  type NbtEditApi,
  type NbtTreeNode,
} from "./nbtEdit";

const props = defineProps<{
  /** 实例 uuid */
  uuid: string;
  /** 来源，空串为实例本身，否则是存档目录名 */
  source: string;
  /** 来源内的相对文件路径 */
  file: string;
  /** 区域文件里的区块表项下标；普通 NBT 文件不传 */
  chunk?: number;
  /** 打开后要定位到的节点路径，来自「查找实体 / 方块」；不传就正常展示整棵树 */
  reveal?: string[];
}>();

const emit = defineEmits<{
  /** 有没有未保存的改动，窗口标题栏据此点亮保存键 */
  (e: "dirty", value: boolean): void;
}>();

/** 当前文件在会话表里的键；区域文件带上区块下标，一个区块一份会话 */
const key = computed(() => sessionKey(props.uuid, props.source, props.file, props.chunk));
/** 当前文件的会话；切文件就是换一份会话，读过的那些原样留着 */
const session = ref<NbtSession>(openSession(props.uuid, props.source, props.file, props.chunk));
const loading = ref(false);
const saving = ref(false);
/** 搜索词，空串 = 不过滤 */
const query = ref("");

/** 外部要求定位的节点路径；用户一动手搜索就清掉，交回正常筛选 */
const focus = ref<string[] | null>(props.reveal?.length ? props.reveal : null);
watch(
  () => props.reveal,
  (value) => {
    focus.value = value?.length ? value : null;
  },
);

/** 记一条改动 */
function pushOp(op: NbtEditDto) {
  session.value.ops.push(op);
  session.value.dirty = true;
}

/** 造一个新标签：显示口径与后端 `nbt_node` 一致，值先按输入原样放上去 */
function blankNode(name: string, kind: NbtKindDto, value: string): NbtTreeNode {
  const node: NbtTreeNode = {
    name,
    kind,
    value: isScalarKind(kind) ? value : "",
    length: 0,
    children: [],
    items: [],
    touched: true,
  };

  // 新建数组时给的那串数是初始内容：本地也摊成元素行，别等保存后重新读盘才对得上
  const itemKind = elementKindOf(kind);
  if (itemKind) {
    node.items = splitArrayText(value).map((text, index) => blankNode(String(index), itemKind, text));
    node.length = node.items.length;
  }

  return node;
}

/** 列表与数组的元素名字就是下标：增删之后重排一遍，显示的下标才不会与实际位置对不上 */
function renumber(items: NbtTreeNode[]) {
  items.forEach((item, index) => {
    item.name = String(index);
  });
}

/** 复合成员按键名排序，与后端 `nbt_node` 的口径一致 */
function sortChildren(children: NbtTreeNode[]) {
  children.sort((a, b) => a.name.localeCompare(b.name));
}

/** 交给行节点的登记表，见 nbtEdit；每一条都同时改工作树与改动台账 */
const editApi: NbtEditApi = {
  openedOf: (path) => session.value.expanded.get(pathKey(path)),
  setOpened: (path, open) => {
    session.value.expanded.set(pathKey(path), open);
  },
  set: (node, path, value) => {
    node.value = value;
    node.touched = true;
    pushOp({ op: "Set", path: [...path], kind: null, value });
  },
  add: (container, containerPath, key, kind, value) => {
    if (container.kind === "Compound") {
      const node = blankNode(key, kind, value);
      container.children.push(node);
      sortChildren(container.children);
      pushOp({ op: "Add", path: [...containerPath, key], kind, value });
      return;
    }
    // 列表与数组都是按下标排的元素序列：数组的元素类型随数组自己，不看下拉
    const itemKind = elementKindOf(container.kind) ?? kind;
    const node = blankNode(String(container.items.length), itemKind, value);
    container.items.push(node);
    renumber(container.items);
    container.length = container.items.length;
    pushOp({ op: "Add", path: [...containerPath, node.name], kind: itemKind, value });
  },
  rename: (container, containerPath, index, newName) => {
    const child = container.children[index];
    if (!child || child.name === newName) return;
    // 新键已被别的成员占用：说一声，别让后端白拒一次
    if (container.children.some((item) => item.name === newName)) {
      showToast(t("config.nbtKeyExists"));
      return;
    }

    const oldName = child.name;
    pushOp({ op: "Rename", path: [...containerPath, oldName], value: newName });

    // 展开状态是按路径记的：键名一改，这一棵子树的旧路径就作废了，删掉（下次按默认档展开）
    const oldKey = pathKey([...containerPath, oldName]);
    const prefix = `${oldKey.slice(0, -1)},`;
    for (const key of [...session.value.expanded.keys()]) {
      if (key === oldKey || key.startsWith(prefix)) session.value.expanded.delete(key);
    }

    child.name = newName;
    sortChildren(container.children);
  },
  remove: (container, containerPath, index) => {
    const isCompound = container.kind === "Compound";
    const items = isCompound ? container.children : container.items;
    const child = items[index];
    if (!child) return;
    // 下标与键名都取"删之前"的那一份：后端按同一顺序套用，看到的就是同一棵树
    pushOp({
      op: "Remove",
      path: [...containerPath, isCompound ? child.name : String(index)],
      kind: null,
      value: "",
    });
    items.splice(index, 1);
    if (!isCompound) {
      renumber(container.items);
      container.length = container.items.length;
    }
  },
};
provide(NBT_EDIT, editApi);

/**
 * 搜索命中的路径集合，null = 没有搜索
 *
 * 整棵树只走这一遍：每个节点各判一次自己的名字，命中就把自己与各级祖先都收进来。
 * 交给行节点的是集合而不是搜索词，免得每个行节点各自再走一遍子树。
 */
const hits = computed<Set<string> | null>(() => {
  // 外部定位优先：只留目标那一支，各级祖先都收进来，目标那一行才会亮着
  if (focus.value) {
    const set = new Set<string>([pathKey([])]);
    const path: string[] = [];
    for (const name of focus.value) {
      path.push(name);
      set.add(pathKey(path));
    }
    return set;
  }

  const q = query.value.trim().toLowerCase();
  if (q === "" || !session.value.tree) return null;

  const set = new Set<string>();
  const walk = (node: NbtTreeNode, path: string[]): boolean => {
    let hit = node.name.toLowerCase().includes(q);
    for (const child of kidsOf(node)) {
      // 子节点命中时这一层也得留着，否则命中项没有可挂的父行
      if (walk(child, [...path, child.name])) hit = true;
    }
    if (hit) set.add(pathKey(path));
    return hit;
  };
  walk(session.value.tree, []);
  return set;
});

/** 搜索没命中任何键：树还在、只是这一遍筛完是空的 */
const searchEmpty = computed(() => !!hits.value && !hits.value.has(pathKey([])));

/** 读盘填会话：新读一份就把未保存的改动清掉（改动只对得上刚读到的这一份） */
async function load() {
  loading.value = true;
  const now = session.value;
  try {
    const data = await openConfigNbt(props.uuid, props.source, props.file, props.chunk);
    now.tree = data.node;
    now.sha1 = data.sha1;
    now.ops = [];
    now.dirty = false;
    now.loaded = true;
  } catch (e) {
    now.tree = null;
    // 读不出来就不动 sha1：树没了，保存键点不动；留着它，读一次成功的会话还能接着用
    now.ops = [];
    now.dirty = false;
    now.loaded = false;
    showToast(tErr(e));
  } finally {
    loading.value = false;
  }
}

/** 切到某个文件：换会话；读过的直接摆出来，没读过的才去打开 */
async function activate() {
  session.value = openSession(props.uuid, props.source, props.file, props.chunk);
  if (!session.value.loaded) await load();
}

/** 保存登记下来的改动；没有改动或正在保存就直接返回 */
async function save() {
  const now = session.value;
  if (saving.value || now.ops.length === 0) return;
  // 交出去的是普通对象：会话里那份是响应式的，别把代理带进 IPC
  const ops = now.ops.map((op) => ({ ...op, path: [...op.path] }));

  saving.value = true;
  try {
    now.sha1 = await writeConfigNbt(props.uuid, props.source, props.file, props.chunk, now.sha1, ops);
    await load();
    showToast(t("config.saved"));
  } catch (e) {
    showToast(tErr(e));
  } finally {
    saving.value = false;
  }
}

defineExpose({ save });

// 有没有改动往外报：标题栏据此显示未保存、点亮保存键
watch(
  () => session.value.dirty,
  (value) => emit("dirty", value),
  { immediate: true },
);

/** 键列宽度，单位像素；0 表示按默认比例（42%） */
const keyWidth = ref(readNumber(KEYS.nbtKeyWidth, 0));

/** 是否正在拖中间那条竖线 */
const resizing = ref(false);

/** 键列宽度的上下限，单位像素 */
const KEY_MIN_WIDTH = 140;
const KEY_MAX_WIDTH = 720;

/** 键列宽度绑到行上；0 时不写变量，走 CSS 里的 42% */
const treeStyle = computed(() =>
  keyWidth.value ? { "--nbt-key-w": `${keyWidth.value}px` } : {},
);

/**
 * 拖动键值之间的竖线改键列宽度
 *
 * 按指针位移算，不读容器左缘 —— 树有内边距，用绝对坐标会差一截。
 * 松手才写 localStorage：列宽属于允许放本地的临时数据，见 AGENTS.md §7.1。
 *
 * - `event`: 竖线上的按下事件
 */
function startResize(event: PointerEvent) {
  event.preventDefault();
  resizing.value = true;
  const startX = event.clientX;
  const startWidth = keyWidth.value || 0;

  function onMove(e: PointerEvent) {
    const next = Math.min(
      KEY_MAX_WIDTH,
      Math.max(KEY_MIN_WIDTH, startWidth + e.clientX - startX),
    );
    keyWidth.value = next;
  }

  function onUp() {
    resizing.value = false;
    window.removeEventListener("pointermove", onMove);
    window.removeEventListener("pointerup", onUp);
    window.removeEventListener("pointercancel", onUp);
    if (keyWidth.value) {
      writeNumber(KEYS.nbtKeyWidth, keyWidth.value);
    }
  }

  window.addEventListener("pointermove", onMove);
  window.addEventListener("pointerup", onUp);
  // 指针被系统收走时也要收尾，否则 resizing 会一直亮着、监听也摘不掉
  window.addEventListener("pointercancel", onUp);
}

// 换来源或换文件就换会话；`immediate` 让首次打开就有内容
watch(key, () => void activate(), { immediate: true });
</script>

<template>
  <div class="nbt-view">
    <div class="nbt-head">
      <SearchBox v-model="query" :placeholder="t('config.nbtSearch')" :clear-label="t('config.nbtSearchClear')"
        width="100%" @update:model-value="focus = null" />
    </div>

    <div class="nbt-tree" :style="treeStyle">
      <!-- 键值之间的竖线：拖动改键列宽度 -->
      <div class="nbt-split" :class="{ on: resizing }" @pointerdown="startResize" />
      <div v-if="loading" class="empty-tip">{{ t("config.loading") }}</div>
      <NbtNode v-else-if="session.tree && !searchEmpty" :node="session.tree" :depth="0" :path="[]" :hits="hits" />
      <p v-else class="empty-tip">{{ session.tree ? t("config.nbtSearchEmpty") : t("config.nbtEmpty") }}</p>
    </div>
  </div>
</template>

<style scoped>
.nbt-view {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 0;
}

/* 搜索固定在树上方：与配置窗口左栏的搜索同一处位置口径 */
.nbt-head {
  flex-shrink: 0;
  padding: 8px 10px 6px;
}

/* 树自己滚；不描边、不垫底，直接站在窗口底色上 */
.nbt-tree {
  flex: 1;
  min-height: 0;
  overflow: auto;
  padding: 0 10px 8px;
}
/* 热区要相对树定位 */
.nbt-tree {
  position: relative;
}

/* 键值之间那条竖线：6px 的透明热区压在上面，看起来仍是行与行画出来的那条细线 */
.nbt-split {
  position: absolute;
  top: 0;
  bottom: 0;
  /* 10px 是 .nbt-tree 的左右内边距：竖线画在行内，热区得跟着内缩同样的距离 */
  left: calc(10px + var(--nbt-key-w, 42%) - 3px);
  width: 6px;
  cursor: col-resize;
  z-index: 1;
}

.nbt-split:hover,
.nbt-split.on {
  background: var(--accent-soft);
}
</style>
