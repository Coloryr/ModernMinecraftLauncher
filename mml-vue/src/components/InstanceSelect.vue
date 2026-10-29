<script setup lang="ts">
// 实例下拉选择，用于列表模式选中实例
// 整块控件**不带图标**：按钮与展开的列表都只有文字（名字 / 版本 / 加载器），详见下方模板
import { computed, nextTick, onMounted, onUnmounted, ref } from "vue";
import { t } from "../lib/i18n";
import type { InstanceInfoDto } from "../lib/bindings";

const props = defineProps<{
  /** 可选的实例列表 */
  instances: InstanceInfoDto[];
  /** 当前选中的实例 uuid；`null` 表示没选 */
  modelValue: string | null;
}>();

const emit = defineEmits<{
  /** `v-model` 更新：新选中的实例 uuid */
  (e: "update:modelValue", uuid: string): void;
  /** 下拉末尾的「添加实例」入口（由窗口决定怎么开新建窗口） */
  (e: "add"): void;
}>();

const open = ref(false);
const rootEl = ref<HTMLDivElement | null>(null);
const btnEl = ref<HTMLButtonElement | null>(null);
const listEl = ref<HTMLDivElement | null>(null);

/**
 * 浮层定位：**fixed**，不是 absolute
 *
 * 它所在的 `.list-mode` 是 `overflow-y: auto` 的滚动容器：absolute 浮层会被算进那个容器的
 * 可滚动区域 —— 一打开下拉，内容区就凭空多出一截能滚的空白（看着像"窗口被撑大了"）。
 * fixed 不进任何祖先的可滚动区域，最高能放多少也好算。
 */
const menuStyle = ref<Record<string, string>>({});

/**
 * 量一次按钮位置并定高
 *
 * 下方装不下整份内容、上方装得下时翻到上方。只封顶不换边的话，列表会为差出来的那十几
 * 像素白多一条滚动条：鼠标停在行上时滚轮才落到列表上，看上去就像列表自己在长高。
 */
function placeMenu() {
  const btn = btnEl.value;
  if (!btn) return;

  const rect = btn.getBoundingClientRect();
  const GAP = 6;
  const EDGE = 10;
  const below = window.innerHeight - rect.bottom - GAP - EDGE;
  const above = rect.top - GAP - EDGE;
  // 不限高时浮层要多高：没被截就是当前高度，被截过就把列表藏起来的那截加回去
  const list = listEl.value;
  const menu = list?.parentElement as HTMLElement | null;
  const hidden = list ? Math.max(0, list.scrollHeight - list.clientHeight) : 0;
  const natural = menu ? menu.offsetHeight + hidden : 0;
  const up = natural <= below ? false : natural <= above || above > below;

  menuStyle.value = {
    left: `${rect.left}px`,
    width: `${rect.width}px`,
    // 用满那一侧的空间而不是封顶 280：实例多时少滚几次，140 是"再挤也要露出一行多"的下限
    maxHeight: `${Math.max(140, up ? above : below)}px`,
    ...(up
      ? { bottom: `${window.innerHeight - rect.top + GAP}px` }
      : { top: `${rect.bottom + GAP}px` }),
  };
}

/**
 * 打开时把当前实例带进视野
 *
 * 浮层每次打开都是重新挂载的，scrollTop 必然回到 0：不滚一下，实例一多就得每次从最上面往下
 * 翻着找，切到别的实例再打开也一样。
 */
function revealActive() {
  const list = listEl.value;
  const active = list?.querySelector<HTMLElement>(".option.active");
  if (!list || !active) return;
  // 用布局值而不是 getBoundingClientRect：此刻浮层还在播 enter 动画，rect 带着缩放
  const top = active.offsetTop - list.offsetTop;
  const bottom = top + active.offsetHeight;
  if (top < list.scrollTop) list.scrollTop = top;
  else if (bottom > list.scrollTop + list.clientHeight) list.scrollTop = bottom - list.clientHeight;
}

async function toggle() {
  if (open.value) {
    open.value = false;
    return;
  }
  open.value = true;
  // 渲染出来才量得到按钮位置和当前实例那一行
  await nextTick();
  placeMenu();
  revealActive();
}

/** 点浮层外面收起；弹层留在组件根内，所以只判根元素即可 */
function onDocMousedown(e: MouseEvent) {
  if (open.value && rootEl.value && !rootEl.value.contains(e.target as Node)) {
    open.value = false;
  }
}

/** 滚动或改窗口大小：fixed 浮层跟不住，直接收起。内容区自己的滚动不会冒泡到 window，所以用 capture */
function closeOnMove(e?: Event) {
  if (!open.value) return;
  // 浮层自己内部滚（实例多时要滚着找）不算"位置变了"，别把下拉收走
  if (e?.target instanceof Node && rootEl.value?.contains(e.target)) return;
  open.value = false;
}

onMounted(() => {
  document.addEventListener("mousedown", onDocMousedown);
  window.addEventListener("resize", closeOnMove);
  window.addEventListener("scroll", closeOnMove, true);
});

onUnmounted(() => {
  document.removeEventListener("mousedown", onDocMousedown);
  window.removeEventListener("resize", closeOnMove);
  window.removeEventListener("scroll", closeOnMove, true);
});

const selected = computed(() =>
  props.instances.find((i) => i.uuid === props.modelValue) ?? null,
);

function pick(inst: InstanceInfoDto) {
  emit("update:modelValue", inst.uuid);
  open.value = false;
}

/** 添加实例：先收起下拉，再交给窗口处理 */
function addNew() {
  open.value = false;
  emit("add");
}
</script>

<template>
  <div class="instance-select" ref="rootEl">
    <button class="select-btn" ref="btnEl" @click="toggle">
      <span class="select-text">
        <span v-if="selected" class="sel-name">{{ selected.name }}</span>
        <span v-else class="sel-placeholder">{{ t("launch.selectInstance") }}</span>
        <span v-if="selected" class="sel-sub">
          {{ selected.version }}
          <template v-if="selected.loader !== 'normal'">
            <span class="loader-text">{{ t(`add.loader.${selected.loader}`) }}</span>
          </template>
        </span>
      </span>
      <svg class="chevron" :class="{ flip: open }" viewBox="0 0 24 24" width="14" height="14" fill="none"
        stroke="currentColor" stroke-width="2">
        <path d="m6 9 6 6 6-6" />
      </svg>
    </button>

    <Transition name="drop">
      <div v-if="open" class="select-menu" :style="menuStyle">
        <!-- 实例列表：整块浮层里**唯一**可滚动的地方，
             所以右侧滚动条只在这一段，不会延伸到下面的"添加实例"上去。
             每行只有文字：图标（实例头像 / 加号）在这个下拉里一律不画 -->
        <div class="menu-list" ref="listEl">
          <button v-for="inst in instances" :key="inst.uuid" class="option"
            :class="{ active: inst.uuid === modelValue, running: inst.running }" @click="pick(inst)">
            <span class="option-text">
              <span class="option-name">{{ inst.name }}</span>
              <span class="option-sub">
                {{ inst.version }}
                <template v-if="inst.loader !== 'normal'">
                  <span class="loader-text">{{ t(`add.loader.${inst.loader}`) }}</span>
                </template>
              </span>
            </span>
            <span v-if="inst.running" class="run-dot" v-tip="t('launch.running')"></span>
          </button>
        </div>

        <!-- 浮层底部固定：新建实例入口在滚动区之外，实例再多也一直看得见 -->
        <div class="menu-foot">
          <div class="menu-sep"></div>
          <button class="option add-option" @click="addNew">
            <span class="option-text">
              <span class="option-name">{{ t("add.title") }}</span>
            </span>
          </button>
        </div>
      </div>
    </Transition>
  </div>
</template>

<style scoped>
.instance-select {
  position: relative;
  width: 100%;
  max-width: 460px;
}

.select-btn {
  width: 100%;
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 10px 14px;
  border-radius: 12px;
  border: 1px solid var(--border);
  background: var(--bg-card);
  color: var(--text);
  cursor: pointer;
  transition: all 0.15s;
  font-family: inherit;
}

.select-btn:hover {
  border-color: var(--accent);
  background: var(--bg-hover);
}

.select-text {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  min-width: 0;
  line-height: 1.3;
  text-align: left;
}

.sel-name {
  font-size: 13px;
  font-weight: 600;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  max-width: 100%;
}

.sel-placeholder {
  font-size: 13px;
  color: var(--text-dim);
}

.sel-sub {
  font-size: 11.5px;
  color: var(--text-dim);
  display: inline-flex;
  align-items: center;
  gap: 4px;
}

.loader-text {
  font-size: 10.5px;
  padding: 0 6px;
  border-radius: 8px;
  background: var(--accent-soft);
  color: var(--accent);
  line-height: 1.6;
}

.chevron {
  color: var(--text-dim);
  transition: transform 0.15s;
  flex-shrink: 0;
}

.chevron.flip {
  transform: rotate(180deg);
}

.select-menu {
  /* fixed：绝不参与 .list-mode 那个滚动容器的可滚动区域；
     left / top / width / maxHeight 全部由 placeMenu() 现算，见脚本 */
  position: fixed;
  display: flex;
  flex-direction: column;
  background: var(--bg-modal);
  border: 1px solid var(--border);
  border-radius: 12px;
  box-shadow: var(--shadow-lg);
  padding: 6px;
  /* 自己滚会把滚动条拉到整块浮层的高度（跨过下面的"添加实例"），
     所以这里不滚：溢出交给 .menu-list  */
  overflow: hidden;
  z-index: 300;
}

/* 实例列表：唯一可滚动的地方 */
.menu-list {
  /* 内容少时按内容高（1 1 auto），超过浮层 max-height 才收缩并在自己内部滚动 */
  flex: 1 1 auto;
  min-height: 0;
  overflow-y: auto;
  scrollbar-gutter: stable;
  /* 见 styles/scrollbar.css */
  /* 滚到底不把滚动交给页面：浮层是 fixed，页面一动它就脱离按钮，还会被 closeOnMove 收掉 */
  overscroll-behavior: contain;
}

.option {
  position: relative;
  width: 100%;
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 8px 10px;
  border: none;
  border-radius: 8px;
  background: transparent;
  color: var(--text);
  cursor: pointer;
  font-family: inherit;
  text-align: left;
}

.option:hover {
  background: var(--bg-hover);
}

.option.active {
  background: var(--accent-soft);
  outline: 1px solid var(--accent-border);
  /* outline 画在元素外沿，而列表是滚动容器 overflow-y: auto：向外那 1px 会被裁掉，
     表现就是"选中框左右两边被切掉了"。向内收 1px 即可 —— 与侧栏实例行同一套处理，
     见 MainSidebar 的 .inst-row.active，那边是被收起动画的 overflow: hidden 裁的 */
  outline-offset: -1px;
}

.option-text {
  flex: 1;
  display: flex;
  flex-direction: column;
  min-width: 0;
  line-height: 1.3;
}

.option-name {
  font-size: 13px;
  font-weight: 600;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.option-sub {
  font-size: 11.5px;
  color: var(--text-dim);
  display: inline-flex;
  align-items: center;
  gap: 4px;
}

.option-sub .loader-text {
  font-size: 10.5px;
}

/* 运行中的绿点：贴右缘并**脱离文档流**，不给选项行带来任何尺寸影响 */
.option.running .run-dot {
  position: absolute;
  top: 50%;
  right: 10px;
  transform: translateY(-50%);
}

/* 给绿点让位：名字 / 副标题的省略号不会压到它 */
.option.running .option-text {
  padding-right: 16px;
}

.run-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: var(--green);
  box-shadow: 0 0 5px var(--green);
  flex-shrink: 0;
}

/* 新建实例入口：与实例项之间用分隔线隔开 */
.menu-sep {
  height: 1px;
  background: var(--border);
  margin: 6px 4px;
}

/* 底部固定的一条：在滚动区之外，右侧自然没有滚动条 */
.menu-foot {
  flex: none;
}

.add-option {
  color: var(--accent);
}
</style>
