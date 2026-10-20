<script setup lang="ts">
// 复用启动器原始主窗口，只在右下角加一个自己的按钮和面板。
// MainWindow 直接取宿主暴露的组件，不把内置窗口再打一份进主题包。
import { ref, type Component } from "vue";
import { createCommands, type MmlApi } from "mml-ipc";

/** 宿主在 App.vue 里挂出来的内置窗口表 */
const hosted = (window as unknown as { __mmlWindows?: Record<string, Component> }).__mmlWindows;
/** 内置主窗口；浏览器预览等没有宿主时为空 */
const MainWindow = hosted?.main ?? null;

/** 类型化命令表，和启动器自己前端用的是同一份；首次调用时再建 */
let commands: ReturnType<typeof createCommands> | null = null;

/** 取宿主命令表；浏览器预览没有 window.mml 时返回 null */
function getCommands() {
  if (commands) return commands;
  const host = (window as unknown as { mml?: MmlApi }).mml;
  if (!host) return null;
  commands = createCommands(host.invoke.bind(host));
  return commands;
}

/** 右下角附加面板是否展开 */
const open = ref(false);
/** 命令调用失败时的提示 */
const commandError = ref("");

/** MainWindow 的关闭按钮只发 close 事件，这里转给启动器命令 */
function closeWindow() {
  void getCommands()?.windows.closeWindow("main", null).catch(() => {});
}

/** 主题按钮：展开面板，并通过命令切换亮 / 暗主题 */
async function onThemeClick() {
  open.value = !open.value;
  const commands = getCommands();
  if (!commands) return;
  commandError.value = "";
  try {
    const config = await commands.windows.getGuiConfig();
    const theme = config.theme === "Dark" ? "Light" : "Dark";
    await commands.windows.saveGuiConfig({ ...config, theme });
  } catch {
    commandError.value = "命令调用失败";
  }
}
</script>

<template>
  <div class="replace-window">
    <component :is="MainWindow" v-if="MainWindow" @close="closeWindow" />
    <p v-else class="theme-missing">当前环境没有内置主窗口，只能看到主题新增的内容。</p>

    <!-- 主题自己新增的内容：右下角按钮 + 小面板 -->
    <button class="theme-fab" type="button" @click="onThemeClick">主题按钮</button>
    <div v-if="open" class="theme-panel">
      <p class="theme-title">这是主题新增的内容</p>
      <p class="theme-text">主窗口本身没有改，还是启动器的 MainWindow；其它窗口也不受影响。</p>
      <p v-if="commandError" class="theme-text">{{ commandError }}</p>
      <button class="theme-close" type="button" @click="open = false">关闭</button>
    </div>
  </div>
</template>

<style scoped>
.replace-window {
  height: 100vh;
}

.theme-missing {
  padding: 24px;
  color: var(--text-dim);
  font-size: 12.5px;
}

.theme-fab {
  position: fixed;
  right: 20px;
  bottom: 20px;
  z-index: 40;
  height: 38px;
  padding: 0 18px;
  border: 1px solid var(--accent-border);
  border-radius: 999px;
  background: var(--accent-grad);
  color: #fff;
  font-size: 12.5px;
  font-weight: 600;
  cursor: pointer;
  box-shadow: 0 6px 22px color-mix(in srgb, var(--accent) 35%, transparent);
}

.theme-panel {
  position: fixed;
  right: 20px;
  bottom: 70px;
  z-index: 40;
  width: 240px;
  padding: 14px 16px;
  border: 1px solid var(--border);
  border-radius: 12px;
  background: var(--bg-card);
  box-shadow: 0 10px 30px rgb(0 0 0 / 30%);
}

.theme-title {
  margin: 0 0 6px;
  font-size: 13px;
  font-weight: 600;
}

.theme-text {
  margin: 0 0 12px;
  color: var(--text-dim);
  font-size: 12px;
  line-height: 1.6;
}

.theme-close {
  border: 0;
  background: none;
  color: var(--accent);
  font-size: 12.5px;
  cursor: pointer;
}
</style>
