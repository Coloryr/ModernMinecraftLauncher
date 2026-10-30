<script setup lang="ts">
// 截图用的控件总览：每个分区都能单独打开（?section=xxx），由 scenarios.mjs 逐个截图。
// 这里只摆组件和示例文字，控件外观全部来自 src/styles 与组件本身。
import { computed, ref } from "vue";
import BaseButton from "../../../src/components/ui/BaseButton.vue";
import BaseDatePicker from "../../../src/components/ui/BaseDatePicker.vue";
import BaseModal from "../../../src/components/ui/BaseModal.vue";
import BaseSwitch from "../../../src/components/ui/BaseSwitch.vue";
import CollapsePanel from "../../../src/components/ui/CollapsePanel.vue";
import GlyphIcon from "../../../src/components/ui/GlyphIcon.vue";
import IconTabs from "../../../src/components/ui/IconTabs.vue";
import NumberStepper from "../../../src/components/ui/NumberStepper.vue";
import SearchBox from "../../../src/components/ui/SearchBox.vue";
import SegmentedTabs from "../../../src/components/ui/SegmentedTabs.vue";

const section = new URLSearchParams(location.search).get("section") || "all";

const TITLES: Record<string, string> = {
  buttons: "按钮",
  inputs: "输入框",
  selects: "下拉框",
  "select-open": "下拉框 · 展开",
  switches: "开关",
  segmented: "分段切换",
  steppers: "数字步进器",
  search: "搜索框",
  form: "表单行",
  date: "日期选择",
  "date-open": "日期选择 · 展开",
  modal: "弹窗",
  collapse: "折叠面板",
  tabs: "图标页签",
  icons: "图标",
};

const title = computed(() => TITLES[section] ?? "全部控件");

/** 当前分区是否要渲染 */
function show(name: string) {
  return section === "all" || section === name;
}

const switchOff = ref(false);
const switchOn = ref(true);
const switchDisabled = ref(false);

const segOptions = [
  { value: "list", label: "列表" },
  { value: "grid", label: "网格" },
  { value: "group", label: "分组" },
];
const segIcons = [
  { value: "list", label: "列表", icon: "list" },
  { value: "grid", label: "网格", icon: "grid" },
  { value: "folder", label: "文件夹", icon: "folder" },
];
const segText = ref("grid");
const segIcon = ref("grid");
const segDisabled = ref("list");

const stepper = ref(8);
const stepperBig = ref(4096);
const searchEmpty = ref("");
const searchFilled = ref("Fabric");
const date = ref("2026-10-28");
const dateEmpty = ref("");
const modalOpen = ref(true);
const tab = ref("new");
const tabDisabled = ref("archive");

const tabs = [
  { id: "new", labelKey: "add.modeNew", icon: "cube" },
  { id: "archive", labelKey: "add.modeArchive", icon: "box" },
  { id: "folder", labelKey: "add.modeFolder", icon: "folder" },
  { id: "online", labelKey: "add.modeOnline", icon: "globe" },
];

const icons = [
  "plus", "minus", "check", "star", "ban", "trash", "alert", "close", "search",
  "chevron-left", "chevron-right", "chevron-down", "arrow-up", "arrow-down",
  "play", "gamepad", "pause", "stop", "power", "gear", "document", "terminal",
  "wrench", "log", "sliders", "save", "news", "check-square", "history", "puzzle",
  "image", "download", "export", "update", "refresh", "folder", "folder-plus",
  "archive", "package", "palette", "server", "sun", "cube", "flask", "user", "coffee",
] as const;
</script>

<template>
  <div class="gallery">
    <header class="g-head">
      <h1>控件预览</h1>
      <span class="g-sub">{{ title }}</span>
    </header>

    <section v-if="show('buttons')" class="card">
      <h2>按钮<code>BaseButton / .ui-btn</code></h2>
      <p class="g-note">五种变体、三档尺寸，以及禁用、整行、图标与分页按钮。</p>

      <div class="g-block">
        <div class="g-caption">变体</div>
        <div class="g-row">
          <BaseButton variant="primary">主操作</BaseButton>
          <BaseButton variant="accent">强调</BaseButton>
          <BaseButton variant="plain">次要</BaseButton>
          <BaseButton variant="danger">危险</BaseButton>
          <BaseButton variant="ghost">幽灵</BaseButton>
        </div>
      </div>

      <div class="g-block">
        <div class="g-caption">尺寸</div>
        <div class="g-row">
          <BaseButton variant="plain" size="sm">小号</BaseButton>
          <BaseButton variant="plain" size="md">中号</BaseButton>
          <BaseButton variant="primary" size="lg">大号主操作</BaseButton>
        </div>
      </div>

      <div class="g-block">
        <div class="g-caption">状态</div>
        <div class="g-row">
          <BaseButton variant="primary" disabled>禁用主操作</BaseButton>
          <BaseButton variant="plain" disabled>禁用次要</BaseButton>
          <BaseButton variant="danger" disabled>禁用危险</BaseButton>
        </div>
      </div>

      <div class="g-block g-narrow">
        <div class="g-caption">整行</div>
        <BaseButton variant="primary" block>占满整行</BaseButton>
      </div>

      <div class="g-block">
        <div class="g-caption">图标按钮</div>
        <div class="g-row">
          <button class="ui-btn-icon" aria-label="添加">
            <GlyphIcon name="plus" :size="15" />
          </button>
          <button class="ui-btn-icon" aria-label="刷新">
            <GlyphIcon name="refresh" :size="15" />
          </button>
          <button class="ui-btn-icon" aria-label="删除">
            <GlyphIcon name="trash" :size="15" />
          </button>
          <button class="ui-btn-icon" aria-label="关闭" disabled>
            <GlyphIcon name="close" :size="15" />
          </button>
        </div>
      </div>

      <div class="g-block">
        <div class="g-caption">分页按钮</div>
        <div class="g-row">
          <button class="page-btn">上一页</button>
          <button class="page-btn">下一页</button>
          <button class="page-btn" disabled>已到末页</button>
        </div>
      </div>
    </section>

    <section v-if="show('inputs')" class="card">
      <h2>输入框<code>.field-input</code></h2>
      <p class="g-note">常态、已填、聚焦、校验失败、禁用与多行输入。</p>

      <div class="g-grid three">
        <div class="g-item">
          <div class="g-caption">占位</div>
          <input class="field-input" placeholder="实例名称" />
        </div>
        <div class="g-item">
          <div class="g-caption">已填</div>
          <input class="field-input" value="1.21.1" />
        </div>
        <div class="g-item">
          <div class="g-caption">聚焦</div>
          <input id="input-focus" class="field-input" value="Forge 47.2.0" />
        </div>
        <div class="g-item">
          <div class="g-caption">校验失败</div>
          <input class="field-input is-invalid" value="" placeholder="不能为空" />
          <div class="error-text">实例名称不能为空</div>
        </div>
        <div class="g-item">
          <div class="g-caption">禁用</div>
          <input class="field-input" value="由启动器管理" disabled />
        </div>
        <div class="g-item">
          <div class="g-caption">多行</div>
          <textarea class="field-input" rows="3">-Xmx4096M
-XX:+UseG1GC</textarea>
        </div>
      </div>
    </section>

    <section v-if="show('selects')" class="card">
      <h2>下拉框<code>.field-select</code></h2>
      <p class="g-note">关闭态：常态、校验失败、禁用；展开态见「下拉框 · 展开」。</p>

      <div class="g-grid three">
        <div class="g-item">
          <div class="g-caption">常态</div>
          <select class="field-select">
            <option>1.21.1</option>
            <option>1.20.6</option>
            <option>1.19.4</option>
          </select>
        </div>
        <div class="g-item">
          <div class="g-caption">校验失败</div>
          <select class="field-select is-invalid">
            <option>请选择游戏版本</option>
            <option>1.21.1</option>
          </select>
          <div class="error-text">请先选择游戏版本</div>
        </div>
        <div class="g-item">
          <div class="g-caption">禁用</div>
          <select class="field-select" disabled>
            <option>跟随实例设置</option>
          </select>
        </div>
      </div>
    </section>

    <section v-if="show('select-open')" class="card">
      <h2>下拉框 · 展开<code>.field-select</code></h2>
      <p class="g-note">页面内渲染的弹层，跟随主题配色并带展开动效。</p>

      <div class="g-item g-narrow">
        <div class="g-caption">游戏版本</div>
        <select id="select-open" class="field-select">
          <option>1.21.4</option>
          <option>1.21.1</option>
          <option>1.20.6</option>
          <option>1.20.1</option>
          <option>1.19.4</option>
          <option>1.18.2</option>
          <option>1.16.5</option>
          <option>1.12.2</option>
        </select>
      </div>
    </section>

    <section v-if="show('switches')" class="card">
      <h2>开关<code>BaseSwitch</code></h2>
      <p class="g-note">关闭、打开、禁用三种状态。</p>

      <div class="g-row">
        <label class="g-switch">
          <BaseSwitch v-model="switchOff" /><span>关闭</span>
        </label>
        <label class="g-switch">
          <BaseSwitch v-model="switchOn" /><span>打开</span>
        </label>
        <label class="g-switch">
          <BaseSwitch v-model="switchDisabled" disabled /><span>禁用</span>
        </label>
      </div>
    </section>

    <section v-if="show('segmented')" class="card">
      <h2>分段切换<code>SegmentedTabs</code></h2>
      <p class="g-note">文字、纯图标与禁用三种形态。</p>

      <div class="g-block">
        <div class="g-caption">文字</div>
        <SegmentedTabs v-model="segText" :options="segOptions" />
      </div>
      <div class="g-block">
        <div class="g-caption">纯图标</div>
        <SegmentedTabs v-model="segIcon" :options="segIcons" icon-only />
      </div>
      <div class="g-block">
        <div class="g-caption">禁用</div>
        <SegmentedTabs v-model="segDisabled" :options="segOptions" disabled />
      </div>
    </section>

    <section v-if="show('steppers')" class="card">
      <h2>数字步进器<code>NumberStepper</code></h2>
      <p class="g-note">一体式「- 输入框 +」，可手动输入，也可按步长加减。</p>

      <div class="g-grid two">
        <div class="g-item g-fit">
          <div class="g-caption">步长 1</div>
          <NumberStepper v-model="stepper" :min="1" :max="32" />
        </div>
        <div class="g-item g-fit">
          <div class="g-caption">步长 512</div>
          <NumberStepper v-model="stepperBig" :min="0" :max="65536" :step="512" />
        </div>
      </div>
    </section>

    <section v-if="show('search')" class="card">
      <h2>搜索框<code>SearchBox</code></h2>
      <p class="g-note">放大镜、输入与清空键；外观与全 app 的输入框同一份。</p>

      <div class="g-grid two">
        <div class="g-item">
          <div class="g-caption">空</div>
          <SearchBox v-model="searchEmpty" placeholder="搜索实例" width="260px" />
        </div>
        <div class="g-item">
          <div class="g-caption">有内容</div>
          <SearchBox v-model="searchFilled" placeholder="搜索实例" width="260px" />
        </div>
      </div>
    </section>

    <section v-if="show('form')" class="card">
      <h2>表单行<code>.field-label / .field-hint / .path-row</code></h2>
      <p class="g-note">标签、说明、两列字段与路径选择行。</p>

      <div class="g-block">
        <label class="field-label">实例名称</label>
        <input class="field-input" value="Fabric 生存" />
        <div class="field-hint">用于主窗口列表显示，重名时会提示。</div>
      </div>

      <div class="g-block">
        <label class="field-label">游戏目录</label>
        <div class="path-row">
          <input class="field-input" value="D:\Minecraft\Fabric 生存" />
          <BaseButton variant="plain" size="sm" class="pick-btn">选择</BaseButton>
        </div>
      </div>

      <div class="g-block">
        <div class="field-row-2">
          <div>
            <label class="field-label">最大内存，单位 MB</label>
            <input class="field-input" value="4096" />
          </div>
          <div>
            <label class="field-label">Java 路径</label>
            <input class="field-input is-invalid" value="" placeholder="尚未选择" />
            <div class="error-text">请先选择 Java</div>
          </div>
        </div>
      </div>
    </section>

    <section v-if="show('date')" class="card">
      <h2>日期选择<code>BaseDatePicker</code></h2>
      <p class="g-note">已选与未选两种状态；展开态见「日期选择 · 展开」。</p>

      <div class="g-grid two">
        <div class="g-item">
          <div class="g-caption">已选日期</div>
          <BaseDatePicker v-model="date" aria-label="选择日期" />
        </div>
        <div class="g-item">
          <div class="g-caption">未选日期</div>
          <BaseDatePicker v-model="dateEmpty" aria-label="选择日期" />
        </div>
      </div>
    </section>

    <section v-if="show('date-open')" class="card">
      <h2>日期选择 · 展开<code>BaseDatePicker</code></h2>
      <p class="g-note">自绘月历，跟随主题配色，挂在 body 上避免被容器裁切。</p>

      <div class="g-item g-narrow date-open">
        <div class="g-caption">选择日期</div>
        <BaseDatePicker v-model="date" aria-label="选择日期" />
      </div>
    </section>

    <section v-if="show('modal')" class="card">
      <h2>弹窗<code>BaseModal</code></h2>
      <p class="g-note">标题、关闭键、内容区与底部操作按钮。</p>
      <div class="g-panel">弹窗覆盖在窗口内容之上，点遮罩空白处不会关闭。</div>
    </section>

    <BaseModal v-if="show('modal') && modalOpen" title="重命名实例" :width="420" :below-titlebar="false"
      @close="modalOpen = false">
      <label class="field-label">新的实例名称</label>
      <input class="field-input" value="Fabric 生存" />
      <div class="field-hint">改名不会移动游戏目录。</div>
      <div class="g-row g-modal-actions">
        <BaseButton variant="plain" @click="modalOpen = false">取消</BaseButton>
        <BaseButton variant="primary" @click="modalOpen = false">确定</BaseButton>
      </div>
    </BaseModal>

    <section v-if="show('collapse')" class="card">
      <h2>折叠面板<code>CollapsePanel</code></h2>
      <p class="g-note">展开与收起；收起后高度精确为 0，内容状态仍保留。</p>

      <div class="g-block">
        <div class="g-caption">展开</div>
        <CollapsePanel :open="true">
          <div class="g-panel">这里是可以展开 / 收起的内容区域。</div>
        </CollapsePanel>
      </div>
      <div class="g-block">
        <div class="g-caption">收起</div>
        <CollapsePanel :open="false">
          <div class="g-panel">这段内容在收起时高度为 0。</div>
        </CollapsePanel>
      </div>
    </section>

    <section v-if="show('tabs')" class="card">
      <h2>图标页签<code>IconTabs</code></h2>
      <p class="g-note">图标加文字，按数量等分；可带数量尾巴，也可整组禁用。</p>

      <div class="g-block">
        <div class="g-caption">选中「从头新建」</div>
        <IconTabs v-model="tab" :tabs="tabs" />
      </div>
      <div class="g-block">
        <div class="g-caption">整组禁用</div>
        <IconTabs v-model="tabDisabled" :tabs="tabs" disabled />
      </div>
    </section>

    <section v-if="show('icons')" class="card">
      <h2>图标<code>GlyphIcon</code></h2>
      <p class="g-note">统一 24 视窗、currentColor 描边，按图标名排列。</p>
      <div class="g-icons">
        <div v-for="name in icons" :key="name" class="g-icon">
          <GlyphIcon :name="name" :size="18" />
          <span>{{ name }}</span>
        </div>
      </div>
    </section>
  </div>
</template>
