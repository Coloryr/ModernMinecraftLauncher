// 截图用的控件预览页：只由 scripts/screenshot 打开，不进启动器本体。
import { createApp } from "vue";
import "../../../src/styles/index.css";
import { vTip } from "../../../src/lib/tip";
import ControlsGallery from "./ControlsGallery.vue";
import "./gallery.css";

createApp(ControlsGallery).directive("tip", vTip).mount("#app");
