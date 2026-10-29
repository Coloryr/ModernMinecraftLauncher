// 设置窗口 · 手柄标签
//
// 状态放在设置窗口这一层而不是标签组件里：切到别的标签时 GamepadTab 会被卸载，
// 编辑中的内容与未保存改动必须留在这里，切回来才不会丢。
import { computed, reactive, ref } from "vue";
import {
  commands,
  type GamepadAxisDto,
  type GamepadButtonDto,
  type GamepadConfigDto,
  type GamepadProfileDto,
} from "../../../lib/bindings";
import { t, tErr } from "../../../lib/i18n";
import { showToast } from "../../../lib/toast";
import { useWindowRefresh } from "../../../composables/useWindowRefresh";
import { isTauri } from "../../windowManager";

/** 按钮在手柄上的记号，设备标识不翻译 */
const BUTTON_LABEL: Record<GamepadButtonDto, string> = {
  south: "A",
  east: "B",
  north: "Y",
  west: "X",
  c: "C",
  z: "Z",
  leftTrigger: "LT",
  leftTrigger2: "LB",
  rightTrigger: "RT",
  rightTrigger2: "RB",
  select: "Select",
  start: "Start",
  mode: "Mode",
  leftThumb: "L3",
  rightThumb: "R3",
  dPadUp: "↑",
  dPadDown: "↓",
  dPadLeft: "←",
  dPadRight: "→",
};

/** 轴的记号 */
const AXIS_LABEL: Record<GamepadAxisDto, string> = {
  leftStickX: "LS X",
  leftStickY: "LS Y",
  leftZ: "LT",
  rightStickX: "RS X",
  rightStickY: "RS Y",
  rightZ: "RT",
  dPadX: "DPad X",
  dPadY: "DPad Y",
};

const ALL_BUTTONS = Object.keys(BUTTON_LABEL) as GamepadButtonDto[];
const ALL_AXES = Object.keys(AXIS_LABEL) as GamepadAxisDto[];

/**
 * 手柄标签的状态与操作
 *
 * # 返回值
 *
 * - `profiles` / `editing` / `config` / `active` / `loading` / `saving`: 配置列表与正在编辑的内容
 * - `dirty`: 编辑器是否有未保存改动
 * - `actions`: 可选动作名，来自后端
 * - `newButton` / `newAxis`: 新增绑定的草稿
 * - `createOpen` / `createName` / `createError` / `deleteOpen` / `pendingName`: 弹窗状态
 * - `BUTTON_LABEL` / `AXIS_LABEL` / `ALL_BUTTONS` / `ALL_AXES`: 按钮与轴的显示名
 * - `init` / `requestSelect` / `discardAndSelect`: 首次装载与切换配置
 * - `openCreate` / `doCreate` / `doDelete` / `activate` / `save`: 新建、删除、设为生效、保存
 * - `addButton` / `addAxis`: 往当前配置里加一条绑定
 */
export function useSettingsGamepad() {
  /** 配置列表 */
  const profiles = ref<GamepadProfileDto[]>([]);
  /** 正在编辑的配置名；空串表示没有配置 */
  const editing = ref("");
  /** 编辑中的内容；读取失败时是 null */
  const config = ref<GamepadConfigDto | null>(null);
  /** 最近一次读取 / 保存成功时的内容，用来判未保存 */
  const origin = ref("");
  const loading = ref(false);
  const saving = ref(false);

  /** 可选动作名 */
  const actions = ref<string[]>([]);

  /** 有无未保存的改动 */
  const dirty = computed(() => !!config.value && JSON.stringify(config.value) !== origin.value);

  /** 当前配置是否生效中 */
  const active = computed(() =>
    profiles.value.some((item) => item.name === editing.value && item.active),
  );

  /** 新建按钮绑定时的草稿 */
  const newButton = reactive({
    button: "south" as GamepadButtonDto,
    modifier: null as GamepadButtonDto | null,
    action: "Space",
  });
  /** 新建轴绑定时的草稿 */
  const newAxis = reactive({ axis: "leftStickY" as GamepadAxisDto, from: -1, to: -0.35, action: "W" });

  const createOpen = ref(false);
  const createName = ref("");
  const createError = ref("");
  const deleteOpen = ref(false);
  /** 待切换的配置名：有未保存改动时先问一句 */
  const pendingName = ref<string | null>(null);

  /** 是否已经装载过；切标签只卸载组件，不再重拉 */
  let loaded = false;
  let initializing = false;

  /** 按钮是否已经有普通绑定，加新行时用来挑下一个按钮 */
  function isBound(button: GamepadButtonDto): boolean {
    return !!config.value?.buttons.some((rule) => rule.button === button && !rule.modifier);
  }

  /** 同一个按钮加同一个修饰键只能绑一条 */
  function isTaken(button: GamepadButtonDto, modifier: GamepadButtonDto | null): boolean {
    return !!config.value?.buttons.some(
      (rule) => rule.button === button && (rule.modifier ?? null) === modifier,
    );
  }

  /** 配置名是否合法，与后端校验同一口径 */
  function validName(name: string): boolean {
    return (
      name.length > 0 &&
      name.length <= 32 &&
      !name.startsWith(".") &&
      !name.endsWith(".") &&
      !/[\\/:*?"<>|]/.test(name)
    );
  }

  /** 拉配置列表 */
  async function loadProfiles() {
    profiles.value = await commands.gamepad.getProfiles();
  }

  /** 打开一份配置；失败时保留编辑状态但清空内容，界面上给删除入口 */
  async function openProfile(name: string) {
    editing.value = name;
    config.value = null;
    origin.value = "";
    if (!name) return;

    loading.value = true;
    try {
      const data = await commands.gamepad.getConfig(name);
      config.value = data;
      origin.value = JSON.stringify(data);
    } catch (e) {
      showToast(tErr(e));
    } finally {
      loading.value = false;
    }
  }

  /** 首次装载：动作表 + 配置列表，默认打开生效中的那份 */
  async function init() {
    if (loaded || initializing || !isTauri()) return;
    initializing = true;
    try {
      actions.value = await commands.gamepad.getActions();
      await loadProfiles();
      const current = profiles.value.find((item) => item.active) ?? profiles.value[0];
      await openProfile(current?.name ?? "");
      loaded = true;
    } catch (e) {
      showToast(tErr(e));
    } finally {
      initializing = false;
    }
  }

  /** 单窗口模式切回本窗口时只刷新列表，不动正在编辑的内容 */
  function refreshProfiles() {
    if (!loaded) return;
    loadProfiles().catch((e) => showToast(tErr(e)));
  }

  useWindowRefresh(refreshProfiles);

  /** 点列表项：有未保存改动时先确认 */
  function requestSelect(name: string) {
    if (name === editing.value) return;
    if (dirty.value) {
      pendingName.value = name;
      return;
    }
    void openProfile(name);
  }

  /** 放弃改动并切换 */
  function discardAndSelect() {
    const name = pendingName.value;
    pendingName.value = null;
    if (name) void openProfile(name);
  }

  /** 新建：先保存未落盘的改动，再打开输入名字的弹窗 */
  async function openCreate() {
    if (dirty.value && !(await save())) return;
    createName.value = "";
    createError.value = "";
    createOpen.value = true;
  }

  /** 建一份新配置，内容复制当前生效的那份 */
  async function doCreate() {
    const name = createName.value.trim();
    if (!validName(name)) {
      createError.value = t("winGamepad.nameInvalid");
      return;
    }
    try {
      await commands.gamepad.createProfile(name);
      createOpen.value = false;
      await loadProfiles();
      await openProfile(name);
    } catch (e) {
      createError.value = tErr(e);
    }
  }

  /** 删除当前编辑的配置 */
  async function doDelete() {
    if (!editing.value) return;
    try {
      await commands.gamepad.deleteProfile(editing.value);
    } catch (e) {
      showToast(tErr(e));
      deleteOpen.value = false;
      return;
    }
    deleteOpen.value = false;
    await loadProfiles();
    const next = profiles.value.find((item) => item.active) ?? profiles.value[0];
    await openProfile(next?.name ?? "");
  }

  /**
   * 把当前配置设为生效；后端会写 Selected
   *
   * 只把 selected 同步到编辑内容与保存基线，未保存的改动仍然算未保存。
   */
  async function activate() {
    if (!editing.value) return;
    try {
      await commands.gamepad.setActive(editing.value);
      await loadProfiles();
      if (config.value) config.value.selected = true;
      if (origin.value) {
        const saved = JSON.parse(origin.value) as GamepadConfigDto;
        saved.selected = true;
        origin.value = JSON.stringify(saved);
      }
    } catch (e) {
      showToast(tErr(e));
    }
  }

  /**
   * 把输入框可能留下的空串归一成数字
   *
   * `<input type="number">` 清空时 `v-model.number` 给的是空串，先由 `validate` 拒绝；
   * 归一放在校验之后，只处理校验通过的数值。
   */
  function normalize(data: GamepadConfigDto) {
    data.mouse.cursorSpeed = Number(data.mouse.cursorSpeed);
    data.mouse.turnSpeed = Number(data.mouse.turnSpeed);
    data.mouse.deadzone = Number(data.mouse.deadzone);
    for (const rule of data.axes) {
      rule.from = Number(rule.from);
      rule.to = Number(rule.to);
    }
  }

  /** 保存前在本地校验，避免只拿到一句泛化的后端错误 */
  function validate(data: GamepadConfigDto): string | null {
    if (!Number.isFinite(data.mouse.cursorSpeed) || !Number.isFinite(data.mouse.turnSpeed)) {
      return t("winGamepad.invalidSpeed");
    }
    if (data.mouse.cursorSpeed < 0 || data.mouse.turnSpeed < 0) {
      return t("winGamepad.invalidSpeed");
    }
    if (!Number.isFinite(data.mouse.deadzone) || data.mouse.deadzone < 0 || data.mouse.deadzone >= 1) {
      return t("winGamepad.invalidDeadzone");
    }
    for (const rule of data.axes) {
      if (!rule.action) return t("winGamepad.invalidAction");
      const ok =
        Number.isFinite(rule.from) &&
        Number.isFinite(rule.to) &&
        rule.from >= -1 &&
        rule.from <= 1 &&
        rule.to >= -1 &&
        rule.to <= 1 &&
        rule.from !== rule.to;
      if (!ok) return t("winGamepad.invalidRange");
    }
    for (const rule of data.buttons) {
      if (!rule.action) return t("winGamepad.invalidAction");
      if (rule.modifier === rule.button) return t("winGamepad.invalidModifier");
    }
    return null;
  }

  /**
   * 保存当前配置，后端编译通过才落盘
   *
   * # 返回值
   *
   * 保存成功返回 true；没打开配置、校验失败或后端拒绝时返回 false
   */
  async function save(): Promise<boolean> {
    if (!config.value || !editing.value) return false;
    const problem = validate(config.value);
    if (problem) {
      showToast(problem);
      return false;
    }
    normalize(config.value);

    saving.value = true;
    try {
      await commands.gamepad.saveConfig(editing.value, config.value);
      origin.value = JSON.stringify(config.value);
      showToast(t("winGamepad.saved"));
      return true;
    } catch (e) {
      showToast(tErr(e));
      return false;
    } finally {
      saving.value = false;
    }
  }

  /** 加一条按钮绑定，加完自动跳到下一个没有普通绑定的按钮 */
  function addButton() {
    if (!config.value || isTaken(newButton.button, newButton.modifier)) return;
    config.value.buttons.push({
      button: newButton.button,
      modifier: newButton.modifier,
      action: newButton.action,
    });
    const next = ALL_BUTTONS.find((button) => !isBound(button));
    if (next) newButton.button = next;
  }

  /** 加一条轴绑定 */
  function addAxis() {
    if (!config.value) return;
    config.value.axes.push({
      axis: newAxis.axis,
      from: newAxis.from,
      to: newAxis.to,
      action: newAxis.action,
    });
  }

  return {
    profiles,
    editing,
    config,
    dirty,
    active,
    loading,
    saving,
    actions,
    newButton,
    newAxis,
    createOpen,
    createName,
    createError,
    deleteOpen,
    pendingName,
    BUTTON_LABEL,
    AXIS_LABEL,
    ALL_BUTTONS,
    ALL_AXES,
    isTaken,
    init,
    requestSelect,
    discardAndSelect,
    openCreate,
    doCreate,
    doDelete,
    activate,
    save,
    addButton,
    addAxis,
  };
}
