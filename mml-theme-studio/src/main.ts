import { createApp } from 'vue';
import '../../mml-vue/src/styles/themes.css';
import '../../mml-vue/src/styles/controls.css';
import '../../mml-vue/src/styles/scrollbar.css';
import './studio.css';
import App from './App.vue';
import { bootstrap } from './api';
import { locale } from './i18n';

// 先取状态再挂载：首帧就是正确的主题与语言，不会先闪一下默认配色。
try {
  const state = await bootstrap();
  locale.value = state.locale;
  document.documentElement.dataset.theme = state.theme;
} catch {
  // 主进程暂不可用时仍挂载界面，错误会在界面里显示
}

createApp(App).mount('#app');
