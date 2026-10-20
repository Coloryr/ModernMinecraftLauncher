(function(vue) {
	//#region ../mml-ipc/src/bindings.ts
	/** 创建命令包装：传入传输层的 invoke，Tauri 页面与自定义主页各用各的 */
	function createCommands(invoke) {
		return {
			windows: {
				activateDecoration: () => invoke("window_activate_decoration"),
				closeWindow: (kind, instance) => invoke("window_close_window", {
					kind,
					instance
				}),
				dropModel: (kind) => invoke("window_drop_model", { kind }),
				ensureModel: (kind) => invoke("window_ensure_model", { kind }),
				getGuiConfig: () => invoke("window_get_gui_config"),
				getWindowSizes: () => invoke("window_get_window_sizes"),
				isMaximized: () => invoke("window_is_maximized"),
				minimize: () => invoke("window_minimize"),
				openWindow: (kind, instance, query) => invoke("window_open_window", {
					kind,
					instance,
					query
				}),
				probeDrop: (path) => invoke("window_probe_drop", { path }),
				reopenWindow: (kind, instance, query) => invoke("window_reopen_window", {
					kind,
					instance,
					query
				}),
				restartApp: () => invoke("window_restart_app"),
				saveGuiConfig: (config) => invoke("window_save_gui_config", { config }),
				setTitle: (title) => invoke("window_set_title", { title }),
				startDragging: () => invoke("window_start_dragging"),
				toggleMaximize: () => invoke("window_toggle_maximize"),
				urlOfText: (text) => invoke("window_url_of_text", { text })
			},
			account: {
				addAccount: (name, accountType, server, password) => invoke("account_add_account", {
					name,
					accountType,
					server,
					password
				}),
				cancelOAuth: () => invoke("account_cancel_oauth"),
				editOffline: (uuid, newName, newUuid) => invoke("account_edit_offline", {
					uuid,
					newName,
					newUuid
				}),
				getAccounts: () => invoke("account_get_accounts"),
				getTextures: (accountType, uuid) => invoke("account_get_textures", {
					accountType,
					uuid
				}),
				openBrowser: (url) => invoke("account_open_browser", { url }),
				openSkinSite: (accountType, uuid) => invoke("account_open_skin_site", {
					accountType,
					uuid
				}),
				refreshAccountToken: (uuid) => invoke("account_refresh_account_token", { uuid }),
				refreshSkin: (uuid) => invoke("account_refresh_skin", { uuid }),
				removeAccount: (uuid) => invoke("account_remove_account", { uuid }),
				setActiveCape: (accountType, uuid, sha1) => invoke("account_set_active_cape", {
					accountType,
					uuid,
					sha1
				}),
				setActiveSkin: (accountType, uuid, sha1) => invoke("account_set_active_skin", {
					accountType,
					uuid,
					sha1
				}),
				setCurrentAccount: (uuid) => invoke("account_set_current_account", { uuid }),
				uploadSkin: (accountType, uuid, variant, path) => invoke("account_upload_skin", {
					accountType,
					uuid,
					variant,
					path
				})
			},
			add: {
				answerNameConflict: (id, answer) => invoke("add_answer_name_conflict", {
					id,
					answer
				}),
				cancel: () => invoke("add_cancel"),
				createNew: (name, version, loader, loaderVersion, group) => invoke("add_create_new", {
					name,
					version,
					loader,
					loaderVersion,
					group
				}),
				detectArchive: (path) => invoke("add_detect_archive", { path }),
				getLoaderVersions: (loader, mc) => invoke("add_get_loader_versions", {
					loader,
					mc
				}),
				getLoaders: () => invoke("add_get_loaders"),
				getPackTypes: () => invoke("add_get_pack_types"),
				getSupportLoaders: (mc) => invoke("add_get_support_loaders", { mc }),
				getVersionTypes: () => invoke("add_get_version_types"),
				importArchive: (path, packType, name, group, unselect) => invoke("add_import_archive", {
					path,
					packType,
					name,
					group,
					unselect
				}),
				importFolder: (path, name, group) => invoke("add_import_folder", {
					path,
					name,
					group
				}),
				importUrl: (url, name, group) => invoke("add_import_url", {
					url,
					name,
					group
				}),
				listArchive: (path) => invoke("add_list_archive", { path }),
				listDir: (path) => invoke("add_list_dir", { path }),
				scanFolder: (path) => invoke("add_scan_folder", { path }),
				setCloseGuard: (enabled) => invoke("add_set_close_guard", { enabled })
			},
			addModpack: {
				cancel: (pid, fid) => invoke("add_modpack_cancel", {
					pid,
					fid
				}),
				clearDone: () => invoke("add_modpack_clear_done"),
				detail: (source, pid) => invoke("add_modpack_detail", {
					source,
					pid
				}),
				file: (source, pid, page, version) => invoke("add_modpack_file", {
					source,
					pid,
					page,
					version
				}),
				install: (source, projectId, fileId, group) => invoke("add_modpack_install", {
					source,
					projectId,
					fileId,
					group
				}),
				lastSource: () => invoke("add_modpack_last_source"),
				list: (source, page, sort, category, filter, version) => invoke("add_modpack_list", {
					source,
					page,
					sort,
					category,
					filter,
					version
				}),
				setLastSource: (source) => invoke("add_modpack_set_last_source", { source }),
				status: () => invoke("add_modpack_status")
			},
			addResource: {
				categories: (source, fileType) => invoke("add_resource_categories", {
					source,
					fileType
				}),
				clearDone: () => invoke("add_resource_clear_done"),
				dependencies: (game, source, pid, fid, fileType) => invoke("add_resource_dependencies", {
					game,
					source,
					pid,
					fid,
					fileType
				}),
				detail: (source, pid) => invoke("add_resource_detail", {
					source,
					pid
				}),
				download: (game, source, pid, fid, fileType, world) => invoke("add_resource_download", {
					game,
					source,
					pid,
					fid,
					fileType,
					world
				}),
				file: (game, source, pid, fileType, page, version, loader) => invoke("add_resource_file", {
					game,
					source,
					pid,
					fileType,
					page,
					version,
					loader
				}),
				gameVersions: (source) => invoke("add_resource_game_versions", { source }),
				lastSource: () => invoke("add_resource_last_source"),
				list: (game, source, fileType, page, sort, category, filter, version, loader) => invoke("add_resource_list", {
					game,
					source,
					fileType,
					page,
					sort,
					category,
					filter,
					version,
					loader
				}),
				saves: (game) => invoke("add_resource_saves", { game }),
				setLastSource: (source) => invoke("add_resource_set_last_source", { source }),
				sortType: (source) => invoke("add_resource_sort_type", { source }),
				sourceType: () => invoke("add_resource_source_type"),
				status: () => invoke("add_resource_status")
			},
			block: {
				getGroups: () => invoke("block_get_groups"),
				list: (lang) => invoke("block_list", { lang }),
				readIconSource: (path) => invoke("block_read_icon_source", { path }),
				renderCancel: () => invoke("block_render_cancel"),
				renderStart: (force) => invoke("block_render_start", { force }),
				setGroups: (data) => invoke("block_set_groups", { data }),
				setIcon: (uuid, id) => invoke("block_set_icon", {
					uuid,
					id
				}),
				setIconArea: (uuid, path, x, y, w, h, size) => invoke("block_set_icon_area", {
					uuid,
					path,
					x,
					y,
					w,
					h,
					size
				}),
				skinAdd: (input) => invoke("block_skin_add", { input }),
				skinRemove: (name) => invoke("block_skin_remove", { name }),
				status: () => invoke("block_status")
			},
			collect: {
				addGroup: (name) => invoke("collect_add_group", { name }),
				clear: (group) => invoke("collect_clear", { group }),
				getData: () => invoke("collect_get_data"),
				imageUrl: (url) => invoke("collect_image_url", { url }),
				projectIcon: (source, pid) => invoke("collect_project_icon", {
					source,
					pid
				}),
				projectItem: (source, pid, fileType) => invoke("collect_project_item", {
					source,
					pid,
					fileType
				}),
				removeGroup: (name) => invoke("collect_remove_group", { name }),
				removeItems: (uuids, group) => invoke("collect_remove_items", {
					uuids,
					group
				}),
				setGroupItems: (group, uuids) => invoke("collect_set_group_items", {
					group,
					uuids
				}),
				star: (source, fileType, pid, name, icon, url, star) => invoke("collect_star", {
					source,
					fileType,
					pid,
					name,
					icon,
					url,
					star
				})
			},
			colormc: {
				checkColormc: () => invoke("window_check_colormc"),
				migrateColormc: (source, mode) => invoke("window_migrate_colormc", {
					source,
					mode
				}),
				skipColormc: () => invoke("window_skip_colormc")
			},
			config: {
				closeNbt: (game, source, file, index) => invoke("config_close_nbt", {
					game,
					source,
					file,
					index
				}),
				deleteFile: (game, source, file) => invoke("config_delete_file", {
					game,
					source,
					file
				}),
				findChunk: (game, source, kind, x, z, name) => invoke("config_find_chunk", {
					game,
					source,
					kind,
					x,
					z,
					name
				}),
				getBytesSlice: (game, source, file, offset, length) => invoke("config_get_bytes_slice", {
					game,
					source,
					file,
					offset,
					length
				}),
				getFile: (game, source, file) => invoke("config_get_file", {
					game,
					source,
					file
				}),
				getFiles: (game, source) => invoke("config_get_files", {
					game,
					source
				}),
				getImage: (game, source, file) => invoke("config_get_image", {
					game,
					source,
					file
				}),
				getMca: (game, source, file) => invoke("config_get_mca", {
					game,
					source,
					file
				}),
				openFileDir: (game, source, file) => invoke("config_open_file_dir", {
					game,
					source,
					file
				}),
				openNbt: (game, source, file, index) => invoke("config_open_nbt", {
					game,
					source,
					file,
					index
				}),
				saveFile: (game, source, file, baseSha1, expectedLines, hunks) => invoke("config_save_file", {
					game,
					source,
					file,
					baseSha1,
					expectedLines,
					hunks
				}),
				setNbt: (game, source, file, index, baseSha1, edits) => invoke("config_set_nbt", {
					game,
					source,
					file,
					index,
					baseSha1,
					edits
				}),
				setTreeWidth: (width) => invoke("config_set_tree_width", { width }),
				treeWidth: () => invoke("config_tree_width")
			},
			customHome: {
				import: (path) => invoke("custom_home_import", { path }),
				openDir: () => invoke("custom_home_open_dir"),
				remove: () => invoke("custom_home_remove"),
				status: () => invoke("custom_home_status")
			},
			customUi: {
				importCss: (path) => invoke("custom_ui_import_css", { path }),
				importPage: (path) => invoke("custom_ui_import_page", { path }),
				openDir: () => invoke("custom_ui_open_dir"),
				removeCss: (id) => invoke("custom_ui_remove_css", { id }),
				removePage: () => invoke("custom_ui_remove_page"),
				status: () => invoke("custom_ui_status")
			},
			download: {
				cancelAll: () => invoke("download_cancel_all"),
				getStatus: () => invoke("download_get_status"),
				pauseAll: () => invoke("download_pause_all"),
				resumeAll: () => invoke("download_resume_all")
			},
			export: {
				getInfo: (uuid) => invoke("export_get_info", { uuid }),
				listDir: (uuid, dir) => invoke("export_list_dir", {
					uuid,
					dir
				}),
				run: (uuid, config) => invoke("export_run", {
					uuid,
					config
				})
			},
			javaDownload: {
				getList: (source, javaType, major, system, arch) => invoke("java_download_get_list", {
					source,
					javaType,
					major,
					system,
					arch
				}),
				getOptions: (source) => invoke("java_download_get_options", { source }),
				getTypes: () => invoke("java_download_get_types"),
				start: (uuid) => invoke("java_download_start", { uuid })
			},
			log: {
				getFiles: (uuid) => invoke("log_get_files", { uuid }),
				getRuntime: (uuid) => invoke("log_get_runtime", { uuid }),
				getSetting: (uuid) => invoke("log_get_setting", { uuid }),
				readFile: (uuid, path) => invoke("log_read_file", {
					uuid,
					path
				}),
				setSetting: (uuid, setting) => invoke("log_set_setting", {
					uuid,
					setting
				}),
				upload: (uuid, file) => invoke("log_upload", {
					uuid,
					file
				})
			},
			main: {
				addGroup: (name) => invoke("main_add_group", { name }),
				createInstance: (name, version, loader, loaderVersion, group, modpackType, source) => invoke("main_create_instance", {
					name,
					version,
					loader,
					loaderVersion,
					group,
					modpackType,
					source
				}),
				deleteInstance: (uuid) => invoke("main_delete_instance", { uuid }),
				getCustomJson: (uuid) => invoke("main_get_custom_json", { uuid }),
				getGroups: () => invoke("main_get_groups"),
				getInstanceArgs: (uuid) => invoke("main_get_instance_args", { uuid }),
				getInstanceLangs: (uuid) => invoke("main_get_instance_langs", { uuid }),
				getInstances: () => invoke("main_get_instances"),
				getJavaList: () => invoke("main_get_java_list"),
				getMotd: (address) => invoke("main_get_motd", { address }),
				getNews: (page) => invoke("main_get_news", { page }),
				getRunning: () => invoke("main_get_running"),
				getSystemMemory: () => invoke("main_get_system_memory"),
				getVersions: () => invoke("main_get_versions"),
				imageBaseUrl: () => invoke("main_image_base_url"),
				launchGame: (uuid, world) => invoke("main_launch_game", {
					uuid,
					world
				}),
				loadState: () => invoke("main_load_state"),
				moveGroup: (uuid, index) => invoke("main_move_group", {
					uuid,
					index
				}),
				moveInstance: (uuid, group, index) => invoke("main_move_instance", {
					uuid,
					group,
					index
				}),
				musicBaseUrl: () => invoke("main_music_base_url"),
				openInstanceDir: (uuid) => invoke("main_open_instance_dir", { uuid }),
				openUrl: (url) => invoke("main_open_url", { url }),
				refreshVersions: () => invoke("main_refresh_versions"),
				removeGroup: (uuid) => invoke("main_remove_group", { uuid }),
				renameInstance: (uuid, name) => invoke("main_rename_instance", {
					uuid,
					name
				}),
				stopGame: (uuid) => invoke("main_stop_game", { uuid }),
				takeStartArg: () => invoke("main_take_start_arg"),
				updateInstance: (uuid, patch) => invoke("main_update_instance", {
					uuid,
					patch
				}),
				updateInstanceArgs: (uuid, args) => invoke("main_update_instance_args", {
					uuid,
					args
				})
			},
			resource: {
				backupSave: (uuid, dir) => invoke("resource_backup_save", {
					uuid,
					dir
				}),
				clearScreenshots: (uuid) => invoke("resource_clear_screenshots", { uuid }),
				datapackDelete: (uuid, dir, name) => invoke("resource_datapack_delete", {
					uuid,
					dir,
					name
				}),
				datapackToggle: (uuid, dir, name) => invoke("resource_datapack_toggle", {
					uuid,
					dir,
					name
				}),
				deleteMod: (uuid, modUuid) => invoke("resource_delete_mod", {
					uuid,
					modUuid
				}),
				deleteResourcepack: (uuid, file) => invoke("resource_delete_resourcepack", {
					uuid,
					file
				}),
				deleteSave: (uuid, dir) => invoke("resource_delete_save", {
					uuid,
					dir
				}),
				deleteSchematic: (uuid, file) => invoke("resource_delete_schematic", {
					uuid,
					file
				}),
				deleteScreenshot: (uuid, name) => invoke("resource_delete_screenshot", {
					uuid,
					name
				}),
				deleteShaderpack: (uuid, file) => invoke("resource_delete_shaderpack", {
					uuid,
					file
				}),
				importFiles: (uuid, kind, paths, mode, parent) => invoke("resource_import_files", {
					uuid,
					kind,
					paths,
					mode,
					parent
				}),
				listDatapacks: (uuid, dir) => invoke("resource_list_datapacks", {
					uuid,
					dir
				}),
				listMods: (uuid) => invoke("resource_list_mods", { uuid }),
				listResourcepacks: (uuid, lang) => invoke("resource_list_resourcepacks", {
					uuid,
					lang
				}),
				listSaveBackups: (uuid, dir) => invoke("resource_list_save_backups", {
					uuid,
					dir
				}),
				listSaves: (uuid) => invoke("resource_list_saves", { uuid }),
				listSchematics: (uuid) => invoke("resource_list_schematics", { uuid }),
				listScreenshots: (uuid) => invoke("resource_list_screenshots", { uuid }),
				listServers: (uuid) => invoke("resource_list_servers", { uuid }),
				listShaderpacks: (uuid) => invoke("resource_list_shaderpacks", { uuid }),
				modDisable: (uuid, modUuid) => invoke("resource_mod_disable", {
					uuid,
					modUuid
				}),
				modEnable: (uuid, modUuid) => invoke("resource_mod_enable", {
					uuid,
					modUuid
				}),
				modGroupAdd: (uuid, name) => invoke("resource_mod_group_add", {
					uuid,
					name
				}),
				modGroupCollapsedSet: (uuid, collapsed) => invoke("resource_mod_group_collapsed_set", {
					uuid,
					collapsed
				}),
				modGroupOrder: (uuid) => invoke("resource_mod_group_order", { uuid }),
				modGroupOrderSet: (uuid, order) => invoke("resource_mod_group_order_set", {
					uuid,
					order
				}),
				modGroupRemove: (uuid, group) => invoke("resource_mod_group_remove", {
					uuid,
					group
				}),
				modGroupRename: (uuid, group, newName) => invoke("resource_mod_group_rename", {
					uuid,
					group,
					newName
				}),
				modGroupSet: (uuid, group, keys) => invoke("resource_mod_group_set", {
					uuid,
					group,
					keys
				}),
				modGroups: (uuid) => invoke("resource_mod_groups", { uuid }),
				modGroupsCollapsed: (uuid) => invoke("resource_mod_groups_collapsed", { uuid }),
				modNoteSet: (uuid, file, note) => invoke("resource_mod_note_set", {
					uuid,
					file,
					note
				}),
				modOnlineSet: (uuid, modUuid, source, projectId, fileId) => invoke("resource_mod_online_set", {
					uuid,
					modUuid,
					source,
					projectId,
					fileId
				}),
				modpackCheck: (uuid) => invoke("resource_modpack_check", { uuid }),
				modpackUpdate: (uuid, fileId) => invoke("resource_modpack_update", {
					uuid,
					fileId
				}),
				modsAutoMark: (uuid, overwrite) => invoke("resource_mods_auto_mark", {
					uuid,
					overwrite
				}),
				modsDepTest: (uuid) => invoke("resource_mods_dep_test", { uuid }),
				modsDepTree: (uuid, modUuid) => invoke("resource_mods_dep_tree", {
					uuid,
					modUuid
				}),
				modsRelatedMods: (uuid, mods, disable) => invoke("resource_mods_related_mods", {
					uuid,
					mods,
					disable
				}),
				modsUpdateApply: (uuid, items) => invoke("resource_mods_update_apply", {
					uuid,
					items
				}),
				modsUpdateCheck: (uuid) => invoke("resource_mods_update_check", { uuid }),
				modsUpdateLog: (uuid) => invoke("resource_mods_update_log", { uuid }),
				modsUpdateLogRemove: (uuid, projectId, index) => invoke("resource_mods_update_log_remove", {
					uuid,
					projectId,
					index
				}),
				openFolder: (uuid, kind, name, parent) => invoke("resource_open_folder", {
					uuid,
					kind,
					name,
					parent
				}),
				rename: (uuid, kind, parent, name, newName) => invoke("resource_rename", {
					uuid,
					kind,
					parent,
					name,
					newName
				}),
				resourcepackDisable: (uuid, file) => invoke("resource_resourcepack_disable", {
					uuid,
					file
				}),
				resourcepackEnable: (uuid, file) => invoke("resource_resourcepack_enable", {
					uuid,
					file
				}),
				restoreSaveBackup: (uuid, dir, file) => invoke("resource_restore_save_backup", {
					uuid,
					dir,
					file
				}),
				saveEdit: (uuid, dir, info) => invoke("resource_save_edit", {
					uuid,
					dir,
					info
				}),
				schematicDetail: (uuid, file) => invoke("resource_schematic_detail", {
					uuid,
					file
				}),
				serverAdd: (uuid, name, ip) => invoke("resource_server_add", {
					uuid,
					name,
					ip
				}),
				serverDelete: (uuid, name, ip) => invoke("resource_server_delete", {
					uuid,
					name,
					ip
				}),
				serverUpdate: (uuid, name, ip, newName, newIp, acceptTextures) => invoke("resource_server_update", {
					uuid,
					name,
					ip,
					newName,
					newIp,
					acceptTextures
				}),
				serversExport: (uuid, path) => invoke("resource_servers_export", {
					uuid,
					path
				}),
				serversImport: (uuid, path) => invoke("resource_servers_import", {
					uuid,
					path
				}),
				shaderSet: (uuid, file) => invoke("resource_shader_set", {
					uuid,
					file
				}),
				viewGet: (uuid) => invoke("resource_view_get", { uuid }),
				viewSet: (uuid, order, category, modView, modColWidths) => invoke("resource_view_set", {
					uuid,
					order,
					category,
					modView,
					modColWidths
				})
			},
			settings: {
				addJava: (name, path) => invoke("settings_add_java", {
					name,
					path
				}),
				clearBg: () => invoke("settings_clear_bg"),
				detectJava: (path) => invoke("settings_detect_java", { path }),
				getBg: () => invoke("settings_get_bg"),
				getCachePath: () => invoke("settings_get_cache_path"),
				getDefaults: () => invoke("settings_get_defaults"),
				getLaunch: () => invoke("settings_get_launch"),
				getNetwork: () => invoke("settings_get_network"),
				getSystemFonts: () => invoke("settings_get_system_fonts"),
				importJava: (name, archivePath) => invoke("settings_import_java", {
					name,
					archivePath
				}),
				removeAllJava: () => invoke("settings_remove_all_java"),
				removeJava: (name) => invoke("settings_remove_java", { name }),
				resizeBg: (percent) => invoke("settings_resize_bg", { percent }),
				saveLaunch: (run, window) => invoke("settings_save_launch", {
					run,
					window
				}),
				saveNetwork: (dto) => invoke("settings_save_network", { dto }),
				scanJava: () => invoke("settings_scan_java"),
				scanJavaDir: (path) => invoke("settings_scan_java_dir", { path }),
				setBg: (source, percent) => invoke("settings_set_bg", {
					source,
					percent
				})
			},
			stats: { data: () => invoke("stats_get_data") }
		};
	}
	//#endregion
	//#region ../mml-ipc/src/listens.ts
	var AccountChange = "account-change";
	var AccountOAuth = "account-oauth";
	var AccountOAuthState = "account-oauth-state";
	var AddLoaderProgress = "add-loader-progress";
	var AddModpackStatus = "add-modpack-status";
	var AddNameConflict = "add-name-conflict";
	var AddPackProgress = "add-pack-progress";
	var AddResourceStatus = "add-resource-status";
	var BgChange = "bg-change";
	var BlockRender = "block-render";
	var ClientConfigChange = "client-config-change";
	var CloseBlocked = "close-blocked";
	var CollectChange = "collect-change";
	var ColormcProgress = "colormc-progress";
	var CustomHomeChange = "custom-home-change";
	var CustomUiChange = "custom-ui-change";
	var DownloadItem = "download-item";
	var DownloadTask = "download-task";
	var ExportFocus = "export-focus";
	var ExportProgress = "export-progress";
	var FileDrop = "file-drop";
	var GameExit = "game-exit";
	var GameLog = "game-log";
	var GuiThemeChange = "gui-theme-change";
	var GuiViewModeChange = "gui-view-mode-change";
	var InstanceChange = "instance-change";
	var JavaChange = "java-change";
	var LaunchError = "launch-error";
	var LaunchState = "launch-state";
	var LoadDone = "load-done";
	var ResourceListModsProgress = "resource-list-mods-progress";
	var ResourceModsAutoMarkProgress = "resource-mods-auto-mark-progress";
	var ResourceModsDuplicate = "resource-mods-duplicate";
	var ResourceModsUpdateDone = "resource-mods-update-done";
	var ResourceModsUpdateProgress = "resource-mods-update-progress";
	var SettingsJavaProgress = "settings-java-progress";
	var SkinConfigChange = "skin-config-change";
	/**
	* 按事件创建订阅函数，每个事件一个带 payload 类型的 onXxx
	*
	* - on: 订阅函数；Tauri 页面传 listen 的同步包装，自定义主页传 window.mml.on
	*
	* # 返回值
	*
	* 各 onXxx(cb)；返回的取消订阅函数要在页面 / 组件卸载时调用
	*/
	function createEvents(on) {
		return {
			onAccountChange: (cb) => on(AccountChange, cb),
			onAccountOAuth: (cb) => on(AccountOAuth, cb),
			onAccountOAuthState: (cb) => on(AccountOAuthState, cb),
			onAddLoaderProgress: (cb) => on(AddLoaderProgress, cb),
			onAddModpackStatus: (cb) => on(AddModpackStatus, cb),
			onAddNameConflict: (cb) => on(AddNameConflict, cb),
			onAddPackProgress: (cb) => on(AddPackProgress, cb),
			onAddResourceStatus: (cb) => on(AddResourceStatus, cb),
			onBgChange: (cb) => on(BgChange, cb),
			onBlockRender: (cb) => on(BlockRender, cb),
			onClientConfigChange: (cb) => on(ClientConfigChange, cb),
			onCloseBlocked: (cb) => on(CloseBlocked, cb),
			onCollectChange: (cb) => on(CollectChange, cb),
			onColormcProgress: (cb) => on(ColormcProgress, cb),
			onCustomHomeChange: (cb) => on(CustomHomeChange, cb),
			onCustomUiChange: (cb) => on(CustomUiChange, cb),
			onDownloadItem: (cb) => on(DownloadItem, cb),
			onDownloadTask: (cb) => on(DownloadTask, cb),
			onExportFocus: (cb) => on(ExportFocus, cb),
			onExportProgress: (cb) => on(ExportProgress, cb),
			onFileDrop: (cb) => on(FileDrop, cb),
			onGameExit: (cb) => on(GameExit, cb),
			onGameLog: (cb) => on(GameLog, cb),
			onGuiThemeChange: (cb) => on(GuiThemeChange, cb),
			onGuiViewModeChange: (cb) => on(GuiViewModeChange, cb),
			onInstanceChange: (cb) => on(InstanceChange, cb),
			onJavaChange: (cb) => on(JavaChange, cb),
			onLaunchError: (cb) => on(LaunchError, cb),
			onLaunchState: (cb) => on(LaunchState, cb),
			onLoadDone: (cb) => on(LoadDone, cb),
			onResourceListModsProgress: (cb) => on(ResourceListModsProgress, cb),
			onResourceModsAutoMarkProgress: (cb) => on(ResourceModsAutoMarkProgress, cb),
			onResourceModsDuplicate: (cb) => on(ResourceModsDuplicate, cb),
			onResourceModsUpdateDone: (cb) => on(ResourceModsUpdateDone, cb),
			onResourceModsUpdateProgress: (cb) => on(ResourceModsUpdateProgress, cb),
			onSettingsJavaProgress: (cb) => on(SettingsJavaProgress, cb),
			onSkinConfigChange: (cb) => on(SkinConfigChange, cb)
		};
	}
	//#endregion
	//#region src/bridge.ts
	/** 拿桥接对象（不在启动器里时为 undefined） */
	function getMml() {
		return window.mml;
	}
	/** 带超时的 invoke 超时抛出的错误，用来区分「没人应答」与「命令自己报错」 */
	var TimeoutError = class extends Error {
		constructor(message) {
			super(message);
			this.name = "TimeoutError";
		}
	};
	/** 给 Promise 加超时（超时抛 TimeoutError） */
	function withTimeout(promise, ms, what) {
		return new Promise((resolve, reject) => {
			const timer = setTimeout(() => reject(new TimeoutError(`${what} 超时（${ms}ms 内没有应答）`)), ms);
			promise.then((value) => {
				clearTimeout(timer);
				resolve(value);
			}, (err) => {
				clearTimeout(timer);
				reject(err);
			});
		});
	}
	/** 调启动器命令（默认 10 秒超时，避免页面卡在「转圈」上） */
	function invoke(cmd, args, ms = 1e4) {
		const mml = getMml();
		if (!mml) return Promise.reject(/* @__PURE__ */ new Error("当前不在启动器中打开，window.mml 不可用"));
		return withTimeout(mml.invoke(cmd, args ?? null), ms, `命令 ${cmd}`);
	}
	var transport = (cmd, args) => invoke(cmd, args ?? void 0);
	/**
	* 类型化命令表，按启动器窗口分组
	*
	* 用法与启动器自己的前端一致，如 `commands.main.getInstances()`、
	* `commands.main.launchGame(uuid, null)`；传错参数或缺参数会在编译期报错。
	*/
	var commands = createCommands(transport);
	/** 探测宿主：页面是否真的跑在启动器里 */
	async function probeHost(ms = 1500) {
		const mml = getMml();
		if (!mml) return false;
		try {
			await withTimeout(mml.invoke("main_load_state"), ms, "握手");
			return true;
		} catch (err) {
			return !(err instanceof TimeoutError);
		}
	}
	/**
	* 启动阶段文案
	*
	* state 取值与启动器一致；表里没有的一律原样显示，方便后端加新阶段时不至于空着。
	*/
	var STAGE_LABELS = {
		launching: "正在启动",
		login: "登录账户",
		check: "检查游戏文件",
		readinfo: "读取版本信息",
		download: "下载缺失文件",
		jvm: "准备启动参数",
		pre: "执行启动前命令",
		post: "执行启动后命令",
		end: "启动完成",
		loadserverpack: "加载服务器包",
		checkserverpack: "检查服务器包",
		downloadserverpack: "下载服务器包"
	};
	/** 订阅启动阶段（launch-state）/ 游戏退出（game-exit）/ 启动失败（launch-error） */
	var { onLaunchState, onGameExit, onLaunchError } = createEvents((event, cb) => getMml()?.on(event, cb) ?? (() => {}));
	//#endregion
	//#region src/App.vue?vue&type=script&setup=true&lang.ts
	var _hoisted_1 = { class: "page" };
	var _hoisted_2 = { class: "hero" };
	var _hoisted_3 = { class: "hero-row" };
	var _hoisted_4 = { class: "slogan" };
	var _hoisted_5 = { class: "hero-actions" };
	var _hoisted_6 = ["disabled"];
	var _hoisted_7 = { class: "ticker" };
	var _hoisted_8 = {
		key: 0,
		class: "notice"
	};
	var _hoisted_9 = {
		key: 1,
		class: "notice warn"
	};
	var _hoisted_10 = { class: "grid" };
	var _hoisted_11 = { class: "col" };
	var _hoisted_12 = { class: "card" };
	var _hoisted_13 = { class: "events" };
	var _hoisted_14 = { class: "event-date" };
	var _hoisted_15 = { class: "event-body" };
	var _hoisted_16 = { class: "card" };
	var _hoisted_17 = { class: "rules" };
	var _hoisted_18 = { class: "card" };
	var _hoisted_19 = { class: "card-head" };
	var _hoisted_20 = ["disabled"];
	var _hoisted_21 = {
		key: 0,
		class: "empty"
	};
	var _hoisted_22 = { class: "inst-list" };
	var _hoisted_23 = { class: "inst-info" };
	var _hoisted_24 = { class: "inst-name" };
	var _hoisted_25 = {
		key: 0,
		class: "tag ok"
	};
	var _hoisted_26 = {
		key: 1,
		class: "tag"
	};
	var _hoisted_27 = { class: "inst-meta" };
	var _hoisted_28 = { class: "tag dim" };
	var _hoisted_29 = {
		key: 0,
		class: "tag dim"
	};
	var _hoisted_30 = {
		key: 1,
		class: "tag dim"
	};
	var _hoisted_31 = ["disabled", "onClick"];
	var _hoisted_32 = { class: "col side" };
	var _hoisted_33 = { class: "card" };
	var _hoisted_34 = { class: "card" };
	var _hoisted_35 = { class: "hint" };
	var _hoisted_36 = { class: "card" };
	var _hoisted_37 = { class: "sponsors" };
	var _hoisted_38 = { class: "card status" };
	var _hoisted_39 = { class: "card-head" };
	var _hoisted_40 = { class: "stage" };
	var _hoisted_41 = { class: "track" };
	var _hoisted_42 = {
		key: 1,
		class: "bar rolling"
	};
	var _hoisted_43 = {
		key: 0,
		class: "err"
	};
	var _hoisted_44 = {
		key: 1,
		class: "log"
	};
	var _hoisted_45 = { class: "foot" };
	/** 服务器名片：公告 / 群号 / 进服方式 / 赞助名单 */
	var App_vue_vue_type_script_setup_true_lang_default = /*@__PURE__*/ (0, vue.defineComponent)({
		__name: "App",
		setup(__props) {
			const SERVER = {
				name: "Coloryr的服务器",
				slogan: "1.20.1 原版生存 · 白名单 · 长期开放",
				/** 顶部滚动公告（一句话说清"现在该干什么"） */
				ticker: "本周活动：周六 20:00 全服寻宝，冠军队伍得限定称号",
				/** 进服前必读 */
				rules: [
					"禁止任何作弊模组与脚本，被举报即封禁",
					"公共区域建筑请勿破坏，私人领地可申请圈地",
					"服务器 1.20.1 Java 版，正版账号可直接进"
				],
				/** 进服方式：本整合包会把这些自动填好，玩家只需点启动 */
				join: [
					{
						k: "服务器地址",
						v: "api.coloryr.com"
					},
					{
						k: "群号",
						v: "571239090"
					},
					{
						k: "版本",
						v: "1.20.1 · Fabric"
					}
				],
				/** 开服时间（给玩家一个预期） */
				schedule: "每天 08:00 – 24:00（维护会提前在群里通知）",
				/** 赞助名单（留名 / 感谢） */
				sponsors: [
					"Coloryr",
					"热心玩家 A",
					"热心玩家 B",
					"匿名服主",
					"建筑组的小王"
				],
				/** 页面底部的联系人 */
				contact: "问题反馈请在本群 @ 管理员，或在游戏内 /mail send 管理员"
			};
			/** 活动日历（最近几场，玩家最关心这个） */
			const EVENTS = [
				{
					date: "本周六 20:00",
					title: "全服寻宝",
					desc: "地图各处埋了 30 个宝箱，先到先得"
				},
				{
					date: "下周三 19:30",
					title: "建筑大赛投票",
					desc: "作品提交截止到周二晚，群里投票"
				},
				{
					date: "每月 1 日",
					title: "服务器维护",
					desc: "换周目存档备份、插件更新，预计 1 小时"
				}
			];
			/** 宿主状态：probing = 正在握手，ready = 在启动器里，absent = 不在启动器里（浏览器预览） */
			const host = (0, vue.ref)("probing");
			const instances = (0, vue.ref)([]);
			const loading = (0, vue.ref)(false);
			const error = (0, vue.ref)("");
			/** 正在请求启动的实例 uuid（按钮转圈用） */
			const launching = (0, vue.ref)("");
			/** 当前启动阶段与进度（来自 launch-state 事件） */
			const stage = (0, vue.ref)("");
			const progress = (0, vue.ref)(null);
			/** 最近几条动态（启动阶段 / 退出 / 失败） */
			const activities = (0, vue.ref)([]);
			/** 事件取消订阅函数，组件卸载时统一调用 */
			const offs = [];
			/** 启动阶段文案 */
			const stageLabel = (0, vue.computed)(() => stage.value ? STAGE_LABELS[stage.value] ?? stage.value : "");
			/** 是否已经在跑（有实例处于运行中） */
			const anyRunning = (0, vue.computed)(() => instances.value.some((i) => i.running));
			/** 服务器推荐实例：跟这一页定位一致的整合包（没有就用第一个） */
			const serverInstance = (0, vue.computed)(() => {
				const list = instances.value;
				return list.find((i) => i.serverUrl === SERVER.join[0].v) ?? list.find((i) => /server|整合|暮色/i.test(i.name)) ?? list[0] ?? null;
			});
			/** 浏览器预览用的示例数据：只在没连上启动器时显示，页面上会明确标注 */
			const DEMO_INSTANCES = [{
				uuid: "demo-1",
				name: "暮色群岛 · 1.20.1",
				group: "服务器",
				version: "1.20.1",
				versionType: "release",
				quickPlay: false,
				loader: "fabric",
				loaderVersion: "0.15.11",
				dir: "（示例数据）",
				running: false,
				modpackType: null,
				pid: null,
				fid: null,
				serverUrl: "play.example.com",
				lang: null,
				logEncoding: null,
				source: null,
				order: 0
			}, {
				uuid: "demo-2",
				name: "暮色群岛 · 测试服",
				group: "服务器",
				version: "1.21.1",
				versionType: "release",
				quickPlay: false,
				loader: "",
				loaderVersion: null,
				dir: "（示例数据）",
				running: false,
				modpackType: null,
				pid: null,
				fid: null,
				serverUrl: null,
				lang: null,
				logEncoding: null,
				source: null,
				order: 1
			}];
			/** 页面上要显示的列表：连上启动器就是真实数据，否则是示例数据 */
			const shownInstances = (0, vue.computed)(() => host.value === "ready" ? instances.value : DEMO_INSTANCES);
			/** 主按钮指向的实例 */
			const mainTarget = (0, vue.computed)(() => host.value === "ready" ? serverInstance.value : DEMO_INSTANCES[0]);
			function nameOf(uuid) {
				return instances.value.find((i) => i.uuid === uuid)?.name ?? DEMO_INSTANCES.find((i) => i.uuid === uuid)?.name ?? uuid.slice(0, 8);
			}
			function messageOf(err) {
				return err instanceof Error ? err.message : String(err);
			}
			function log(text) {
				activities.value.unshift(text);
				if (activities.value.length > 5) activities.value.length = 5;
			}
			/** 拉实例列表（main_get_instances） */
			async function refreshInstances() {
				loading.value = true;
				error.value = "";
				try {
					instances.value = await commands.main.getInstances();
				} catch (err) {
					error.value = messageOf(err);
				} finally {
					loading.value = false;
				}
			}
			/** 启动游戏（main_launch_game，参数是实例 uuid） */
			async function launch(inst) {
				if (!inst || host.value !== "ready") return;
				launching.value = inst.uuid;
				error.value = "";
				try {
					await commands.main.launchGame(inst.uuid, null);
					inst.running = true;
					log(`已请求启动：${inst.name}`);
				} catch (err) {
					error.value = messageOf(err);
					log(`启动请求失败：${messageOf(err)}`);
				} finally {
					launching.value = "";
				}
			}
			(0, vue.onMounted)(async () => {
				if (!getMml()) {
					host.value = "absent";
					return;
				}
				offs.push(onLaunchState((e) => {
					stage.value = e.state;
					progress.value = e.progress;
					const pct = e.progress === null ? "" : ` ${e.progress}%`;
					log(`[${nameOf(e.uuid)}] ${STAGE_LABELS[e.state] ?? e.state}${pct}`);
				}));
				offs.push(onGameExit((e) => {
					const inst = instances.value.find((i) => i.uuid === e.uuid);
					if (inst) inst.running = false;
					stage.value = "";
					progress.value = null;
					log(`[${nameOf(e.uuid)}] 游戏已退出（退出码 ${e.code}）`);
				}));
				offs.push(onLaunchError((e) => {
					error.value = e.message;
					log(`启动失败：${e.message}`);
				}));
				const ok = await probeHost();
				host.value = ok ? "ready" : "absent";
				if (ok) {
					await refreshInstances();
					log("已连接启动器");
				}
			});
			(0, vue.onUnmounted)(() => {
				for (const off of offs) off();
				offs.length = 0;
			});
			return (_ctx, _cache) => {
				return (0, vue.openBlock)(), (0, vue.createElementBlock)("div", _hoisted_1, [
					(0, vue.createElementVNode)("header", _hoisted_2, [(0, vue.createElementVNode)("div", _hoisted_3, [(0, vue.createElementVNode)("div", null, [(0, vue.createElementVNode)("h1", null, (0, vue.toDisplayString)(SERVER.name), 1), (0, vue.createElementVNode)("p", _hoisted_4, (0, vue.toDisplayString)(SERVER.slogan), 1)]), (0, vue.createElementVNode)("div", _hoisted_5, [(0, vue.createElementVNode)("button", {
						class: "btn hero-btn",
						disabled: host.value !== "ready" || !mainTarget.value || anyRunning.value || launching.value !== "",
						onClick: _cache[0] || (_cache[0] = ($event) => launch(mainTarget.value))
					}, (0, vue.toDisplayString)(anyRunning.value ? "游戏运行中" : launching.value ? "正在启动…" : host.value === "ready" ? "一键进服" : "一键进服（需在启动器中打开）"), 9, _hoisted_6)])]), (0, vue.createElementVNode)("p", _hoisted_7, "📣 " + (0, vue.toDisplayString)(SERVER.ticker), 1)]),
					host.value === "probing" ? ((0, vue.openBlock)(), (0, vue.createElementBlock)("div", _hoisted_8, "正在连接启动器…")) : host.value === "absent" ? ((0, vue.openBlock)(), (0, vue.createElementBlock)("div", _hoisted_9, " 未检测到启动器（应该是用浏览器直接打开的）：下面是示例数据，「一键进服」不会真的启动游戏。 要看真实效果，把本工程构建出的 zip 导入启动器：设置 → 客户端设置 → 自定义主页面。 ")) : (0, vue.createCommentVNode)("", true),
					(0, vue.createElementVNode)("div", _hoisted_10, [(0, vue.createElementVNode)("div", _hoisted_11, [
						(0, vue.createElementVNode)("section", _hoisted_12, [_cache[1] || (_cache[1] = (0, vue.createElementVNode)("h2", null, "最近活动", -1)), (0, vue.createElementVNode)("ul", _hoisted_13, [((0, vue.openBlock)(), (0, vue.createElementBlock)(vue.Fragment, null, (0, vue.renderList)(EVENTS, (ev) => {
							return (0, vue.createElementVNode)("li", {
								key: ev.title,
								class: "event"
							}, [(0, vue.createElementVNode)("span", _hoisted_14, (0, vue.toDisplayString)(ev.date), 1), (0, vue.createElementVNode)("span", _hoisted_15, [(0, vue.createElementVNode)("b", null, (0, vue.toDisplayString)(ev.title), 1), (0, vue.createElementVNode)("em", null, (0, vue.toDisplayString)(ev.desc), 1)])]);
						}), 64))])]),
						(0, vue.createElementVNode)("section", _hoisted_16, [_cache[2] || (_cache[2] = (0, vue.createElementVNode)("h2", null, "进服必读", -1)), (0, vue.createElementVNode)("ul", _hoisted_17, [((0, vue.openBlock)(true), (0, vue.createElementBlock)(vue.Fragment, null, (0, vue.renderList)(SERVER.rules, (r) => {
							return (0, vue.openBlock)(), (0, vue.createElementBlock)("li", { key: r }, (0, vue.toDisplayString)(r), 1);
						}), 128))])]),
						(0, vue.createElementVNode)("section", _hoisted_18, [
							(0, vue.createElementVNode)("div", _hoisted_19, [_cache[3] || (_cache[3] = (0, vue.createElementVNode)("h2", null, "选择客户端", -1)), (0, vue.createElementVNode)("button", {
								class: "btn ghost",
								disabled: loading.value || host.value !== "ready",
								onClick: refreshInstances
							}, (0, vue.toDisplayString)(loading.value ? "读取中…" : "刷新列表"), 9, _hoisted_20)]),
							host.value === "ready" && !loading.value && instances.value.length === 0 ? ((0, vue.openBlock)(), (0, vue.createElementBlock)("p", _hoisted_21, " 启动器里还没有游戏实例：先添加一个（或导入本服整合包），再回来点「一键进服」。 ")) : (0, vue.createCommentVNode)("", true),
							(0, vue.createElementVNode)("ul", _hoisted_22, [((0, vue.openBlock)(true), (0, vue.createElementBlock)(vue.Fragment, null, (0, vue.renderList)(shownInstances.value, (inst) => {
								return (0, vue.openBlock)(), (0, vue.createElementBlock)("li", {
									key: inst.uuid,
									class: "inst"
								}, [(0, vue.createElementVNode)("div", _hoisted_23, [(0, vue.createElementVNode)("div", _hoisted_24, [(0, vue.createTextVNode)((0, vue.toDisplayString)(inst.name) + " ", 1), inst.running ? ((0, vue.openBlock)(), (0, vue.createElementBlock)("span", _hoisted_25, "运行中")) : mainTarget.value && inst.uuid === mainTarget.value.uuid ? ((0, vue.openBlock)(), (0, vue.createElementBlock)("span", _hoisted_26, " 推荐 ")) : (0, vue.createCommentVNode)("", true)]), (0, vue.createElementVNode)("div", _hoisted_27, [
									(0, vue.createElementVNode)("span", _hoisted_28, (0, vue.toDisplayString)(inst.version), 1),
									inst.loader ? ((0, vue.openBlock)(), (0, vue.createElementBlock)("span", _hoisted_29, (0, vue.toDisplayString)(inst.loader), 1)) : (0, vue.createCommentVNode)("", true),
									inst.group ? ((0, vue.openBlock)(), (0, vue.createElementBlock)("span", _hoisted_30, (0, vue.toDisplayString)(inst.group), 1)) : (0, vue.createCommentVNode)("", true)
								])]), (0, vue.createElementVNode)("button", {
									class: "btn primary",
									disabled: host.value !== "ready" || launching.value === inst.uuid || inst.running,
									onClick: ($event) => launch(inst)
								}, (0, vue.toDisplayString)(inst.running ? "运行中" : launching.value === inst.uuid ? "启动中…" : "启动"), 9, _hoisted_31)]);
							}), 128))])
						])
					]), (0, vue.createElementVNode)("aside", _hoisted_32, [
						(0, vue.createElementVNode)("section", _hoisted_33, [
							_cache[4] || (_cache[4] = (0, vue.createElementVNode)("h2", null, "怎么进服", -1)),
							((0, vue.openBlock)(true), (0, vue.createElementBlock)(vue.Fragment, null, (0, vue.renderList)(SERVER.join, (row) => {
								return (0, vue.openBlock)(), (0, vue.createElementBlock)("p", {
									key: row.k,
									class: "kv"
								}, [(0, vue.createElementVNode)("span", null, (0, vue.toDisplayString)(row.k), 1), (0, vue.createElementVNode)("b", null, (0, vue.toDisplayString)(row.v), 1)]);
							}), 128)),
							_cache[5] || (_cache[5] = (0, vue.createElementVNode)("p", { class: "hint" }, "点上面的「一键进服」即可；客户端会自动连上服务器。", -1))
						]),
						(0, vue.createElementVNode)("section", _hoisted_34, [_cache[6] || (_cache[6] = (0, vue.createElementVNode)("h2", null, "开服时间", -1)), (0, vue.createElementVNode)("p", _hoisted_35, (0, vue.toDisplayString)(SERVER.schedule), 1)]),
						(0, vue.createElementVNode)("section", _hoisted_36, [
							_cache[7] || (_cache[7] = (0, vue.createElementVNode)("h2", null, "赞助名单", -1)),
							(0, vue.createElementVNode)("ul", _hoisted_37, [((0, vue.openBlock)(true), (0, vue.createElementBlock)(vue.Fragment, null, (0, vue.renderList)(SERVER.sponsors, (s) => {
								return (0, vue.openBlock)(), (0, vue.createElementBlock)("li", { key: s }, (0, vue.toDisplayString)(s), 1);
							}), 128))]),
							_cache[8] || (_cache[8] = (0, vue.createElementVNode)("p", { class: "hint" }, "感谢以上玩家的支持，服务器才能一直开下去。", -1))
						])
					])]),
					(0, vue.createElementVNode)("section", _hoisted_38, [
						(0, vue.createElementVNode)("div", _hoisted_39, [_cache[9] || (_cache[9] = (0, vue.createElementVNode)("h2", null, "启动状态", -1)), (0, vue.createElementVNode)("span", _hoisted_40, (0, vue.toDisplayString)(stageLabel.value || "空闲"), 1)]),
						(0, vue.createElementVNode)("div", _hoisted_41, [progress.value !== null ? ((0, vue.openBlock)(), (0, vue.createElementBlock)("div", {
							key: 0,
							class: "bar",
							style: (0, vue.normalizeStyle)({ width: progress.value + "%" })
						}, null, 4)) : stage.value ? ((0, vue.openBlock)(), (0, vue.createElementBlock)("div", _hoisted_42)) : (0, vue.createCommentVNode)("", true)]),
						error.value ? ((0, vue.openBlock)(), (0, vue.createElementBlock)("p", _hoisted_43, (0, vue.toDisplayString)(error.value), 1)) : (0, vue.createCommentVNode)("", true),
						activities.value.length ? ((0, vue.openBlock)(), (0, vue.createElementBlock)("ul", _hoisted_44, [((0, vue.openBlock)(true), (0, vue.createElementBlock)(vue.Fragment, null, (0, vue.renderList)(activities.value, (line, i) => {
							return (0, vue.openBlock)(), (0, vue.createElementBlock)("li", { key: i }, (0, vue.toDisplayString)(line), 1);
						}), 128))])) : (0, vue.createCommentVNode)("", true)
					]),
					(0, vue.createElementVNode)("footer", _hoisted_45, [(0, vue.createElementVNode)("p", null, (0, vue.toDisplayString)(SERVER.contact), 1), _cache[10] || (_cache[10] = (0, vue.createElementVNode)("p", { class: "foot-dim" }, [
						(0, vue.createTextVNode)(" 本页由服主定制 · 改 "),
						(0, vue.createElementVNode)("code", null, "src/App.vue"),
						(0, vue.createTextVNode)(" 的 SERVER / EVENTS 即可 · 能力说明见 "),
						(0, vue.createElementVNode)("code", null, "README.md")
					], -1))])
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
	//#region src/App.vue
	var App_default = /*#__PURE__*/ _plugin_vue_export_helper_default(App_vue_vue_type_script_setup_true_lang_default, [["__scopeId", "data-v-1b4981b2"]]);
	//#endregion
	//#region src/entry.ts
	window.__mmlHomePages.push(() => App_default);
	//#endregion
})(__mmlVue);
