use super::*;

/// 这条 JVM 参数是不是在选择垃圾回收器
///
/// - `arg`: 单条 JVM 参数
///
/// # 返回值
///
/// `-XX:+Use…GC` 形式的选择器（UseZGC / UseG1GC / UseShenandoahGC 等）返回 `true`；
/// 带参数值的开关（`UseGCOverheadLimit`）不算
pub(super) fn is_gc_arg(arg: &str) -> bool {
    arg.starts_with("-XX:+Use") && arg.ends_with("GC")
}

impl InstanceSettingObj {
    /// 创建Jvm参数
    ///
    /// - `auth`: 登陆使用的账户
    /// - `java`: 使用的JAVA主版本号
    /// - `mixin`: ColorASM IPC 连接参数，`None` 表示不注入
    /// - `existing`: 版本 JSON / 加载器已经给出的 JVM 参数；自动档据此避让它们自带的 GC
    ///
    /// # 返回值
    ///
    /// 返回Jvm启动参数列表，刷新失败且用户拒绝离线模式返回对应错误。
    pub(super) async fn make_jvm_arg(
        &self,
        auth: &LoginObj,
        java: u8,
        mixin: Option<&MixinArg>,
        existing: &[String],
    ) -> CoreResult<Vec<String>> {
        let config = &mml_config::read_config().jvm_arg;

        let jvm_args = self
            .jvm_arg
            .as_ref()
            .and_then(|j| j.jvm_args.as_deref())
            .or(config.jvm_args.as_deref())
            .unwrap_or("")
            .to_string();
        let mut gc = self
            .jvm_arg
            .as_ref()
            .and_then(|j| j.gc_mode)
            .or(config.gc_mode)
            .unwrap_or(GCType::Auto);
        let max = self
            .jvm_arg
            .as_ref()
            .and_then(|j| j.max_memory)
            .or(config.max_memory)
            .unwrap_or(0);
        let min = self
            .jvm_arg
            .as_ref()
            .and_then(|j| j.min_memory)
            .or(config.min_memory)
            .unwrap_or(0);
        let colorasm = self
            .jvm_arg
            .as_ref()
            .and_then(|j| j.colorasm)
            .or(config.colorasm)
            .unwrap_or(false);

        let mut args = Vec::new();

        if self.isolate_home() {
            // Windows 上 JVM 的 user.home 不跟环境变量走，单独补一条属性
            args.push(format!(
                "-Duser.home={}",
                self.isolated_home().to_string_lossy()
            ));
        }

        if colorasm && let Some(mixin) = mixin {
            // 内置 jar 要先落到运行库目录：检查阶段会跳过没有下载地址的本地库，
            // 没人写它时 -javaagent 指向一个不存在的文件，JVM 直接起不来
            launcher_path::ready_colorasm()?;
            args.push(format!("-Dcoloryrasm.mixin.port={}", mixin.port));
            args.push(format!("-Dcoloryrasm.mixin.uuid={}", self.uuid));
            args.push(format!("-Dcoloryrasm.mixin.key={}", mixin.key));
            // 只走 -javaagent，不要把这个 jar 也塞进 classpath：
            // Forge 1.20.5+ 的 secure bootstrap 会把 classpath 上的 jar 变成自动模块，
            // agent 的类因此落进命名模块，被改写的 org.lwjgl.glfw 不读它，
            // 调用直接抛 IllegalAccessError（early display 起不来、游戏秒退）
            args.push(format!(
                "-javaagent:{}",
                launcher_path::get_colorasm().file.to_string_lossy()
            ));
        }

        if gc == GCType::Auto {
            // 版本 JSON / 加载器 / 自定义参数已经指定了 GC 就听它们的，自动档不再叠一套
            // （新版客户端的 jvm 参数自带 UseZGC / UseG1GC，重复追加会互相覆盖）
            let custom_has_gc = jvm_args.split('\n').any(|item| is_gc_arg(item.trim()));
            if !custom_has_gc && !existing.iter().any(|arg| is_gc_arg(arg)) {
                if java >= 21 {
                    gc = GCType::ZGC;
                } else {
                    gc = GCType::G1GC;
                }
            }
        }

        if gc == GCType::ZGC {
            args.extend(names::GCZGC.map(|item| item.to_string()));
        } else if gc == GCType::G1GC {
            args.extend(names::G1GC.map(|item| item.to_string()));
        }

        if min > 0 {
            args.push(format!("-Xms{min}m"));
        }

        if max > 0 {
            args.push(format!("-Xmx{max}m"));
        }

        // 空行必须滤掉：空参数会被 java 当成主类名，直接"找不到或无法加载主类"起不来。
        // 自定义参数是从文本框整段贴进来的，结尾带一个换行太常见了
        args.extend(
            jvm_args
                .split('\n')
                .map(|item| item.trim())
                .filter(|item| !item.is_empty())
                .map(|item| item.to_string()),
        );

        // 统一处理外置登录认证参数
        let auth_key: Option<String> = match &auth.auth_type {
            AuthType::Nide8 => {
                args.push(format!(
                    "-javaagent:{}={}",
                    libraries_path::get_nide8_file().unwrap().to_string_lossy(),
                    auth.text1.as_ref().unwrap()
                ));
                None
            }
            AuthType::AuthlibInjector => {
                let key = auth.get_authlib_key().await?;
                args.push(format!(
                    "-javaagent:{}={}",
                    libraries_path::get_authlib_file()
                        .unwrap()
                        .to_string_lossy(),
                    auth.text1.as_ref().unwrap(),
                ));
                Some(key)
            }
            AuthType::LittleSkin | AuthType::SelfLittleSkin => {
                let key = auth.get_littleskin_key().await?;
                let endpoint = if matches!(auth.auth_type, AuthType::LittleSkin) {
                    format!("{}api/yggdrasil", urls::LITTLE_SKIN_URL)
                } else {
                    auth.text1.as_ref().unwrap().clone()
                };
                args.push(format!(
                    "-javaagent:{}={}",
                    libraries_path::get_authlib_file()
                        .unwrap()
                        .to_string_lossy(),
                    endpoint,
                ));
                Some(key)
            }
            _ => None,
        };

        if let Some(key) = auth_key {
            args.push(format!(
                "-Dauthlibinjector.yggdrasil.prefetched={}",
                hash_helper::gen_base64(&key)
            ));
            args.push("-Dauthlibinjector.side=client".to_string());
        }

        // 启动器自身信息的系统属性，前缀统一 mml
        args.push(format!(
            "-Dmml.dir={}",
            mml_base::get_base_dir().to_string_lossy()
        ));
        args.push(format!("-Dmml.game.uuid={}", self.uuid.to_string()));
        args.push(format!("-Dmml.game.name={}", self.name));
        args.push(format!("-Dmml.game.version={}", self.version));
        args.push(format!(
            "-Dmml.game.dir={}",
            self.get_game_path().to_string_lossy()
        ));

        Ok(args)
    }
}
