var MmlWindowTheme = (function(exports, vue) {
	Object.defineProperty(exports, Symbol.toStringTag, { value: "Module" });
	//#region src/ReplaceWindow.vue?vue&type=script&setup=true&lang.ts
	var _hoisted_1 = { class: "replace-window" };
	var _hoisted_2 = {
		key: 1,
		class: "theme-missing"
	};
	var _hoisted_3 = {
		key: 2,
		class: "theme-panel"
	};
	/** 宿主在 App.vue 里挂出来的内置窗口表 */
	var ReplaceWindow_vue_vue_type_script_setup_true_lang_default = /*@__PURE__*/ (0, vue.defineComponent)({
		__name: "ReplaceWindow",
		setup(__props) {
			/** 内置主窗口；浏览器预览等没有宿主时为空 */
			const MainWindow = window.__mmlWindows?.main ?? null;
			/** 右下角附加面板是否展开 */
			const open = (0, vue.ref)(false);
			/** MainWindow 的关闭按钮只发 close 事件，这里转给启动器命令 */
			function closeWindow() {
				const mml = window.mml;
				if (!mml) return;
				mml.invoke("window_close_window", {
					kind: "main",
					instance: null
				}).catch(() => {});
			}
			return (_ctx, _cache) => {
				return (0, vue.openBlock)(), (0, vue.createElementBlock)("div", _hoisted_1, [
					(0, vue.unref)(MainWindow) ? ((0, vue.openBlock)(), (0, vue.createBlock)((0, vue.resolveDynamicComponent)((0, vue.unref)(MainWindow)), {
						key: 0,
						onClose: closeWindow
					}, null, 32)) : ((0, vue.openBlock)(), (0, vue.createElementBlock)("p", _hoisted_2, "当前环境没有内置主窗口，只能看到主题新增的内容。")),
					(0, vue.createElementVNode)("button", {
						class: "theme-fab",
						type: "button",
						onClick: _cache[0] || (_cache[0] = ($event) => open.value = !open.value)
					}, "主题按钮"),
					open.value ? ((0, vue.openBlock)(), (0, vue.createElementBlock)("div", _hoisted_3, [
						_cache[2] || (_cache[2] = (0, vue.createElementVNode)("p", { class: "theme-title" }, "这是主题新增的内容", -1)),
						_cache[3] || (_cache[3] = (0, vue.createElementVNode)("p", { class: "theme-text" }, "主窗口本身没有改，还是启动器的 MainWindow；其它窗口也不受影响。", -1)),
						(0, vue.createElementVNode)("button", {
							class: "theme-close",
							type: "button",
							onClick: _cache[1] || (_cache[1] = ($event) => open.value = false)
						}, "关闭")
					])) : (0, vue.createCommentVNode)("", true)
				]);
			};
		}
	});
	//#endregion
	//#region \0plugin-vue:export-helper
	var _plugin_vue_export_helper_default = (sfc, props) => {
		const target = sfc.__vccOpts || sfc;
		for (const [key, val] of props) target[key] = val;
		return target;
	};
	//#endregion
	//#region src/ReplaceWindow.vue
	var ReplaceWindow_default = /*#__PURE__*/ _plugin_vue_export_helper_default(ReplaceWindow_vue_vue_type_script_setup_true_lang_default, [["__scopeId", "data-v-0dcc67ed"]]);
	//#endregion
	//#region src/replace.ts
	/**
	* 按窗口 kind 返回要替换的页面
	*
	* - `kind`: 窗口标识
	*/
	function replace(kind) {
		return kind === "main" ? ReplaceWindow_default : null;
	}
	window.__mmlWindowPages.push(replace);
	//#endregion
	exports.replace = replace;
	return exports;
})({}, __mmlVue);
