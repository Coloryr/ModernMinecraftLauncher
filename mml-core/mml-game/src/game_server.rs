//! 游戏实例服务器相关

use std::sync::Arc;

use mml_names::i18_items::error_type::CoreResult;
use mml_nbt::{
    NbtType,
    nbt_file::{CompressType, NbtFile},
    nbt_types::{NbtByte, NbtCompound, NbtList, NbtString},
};
use mml_sys::path_helper;

use crate::launcher::instance_setting_obj::InstanceSettingObj;

/// 服务器列表里的单个条目，对应 `servers.dat` 里的一条记录
pub struct ServerInfoObj {
    /// 服务器地址，形如 `host:port`
    pub ip: String,
    /// 服务器显示名
    pub name: String,
    /// 服务器图标，Base64 编码；可缺省
    pub icon: Option<String>,
    /// 是否接受服务器资源包，缺省为 false
    pub accept_textures: bool,
    /// 所属实例
    pub instance: Arc<InstanceSettingObj>,
}

impl Default for ServerInfoObj {
    fn default() -> Self {
        Self {
            ip: Default::default(),
            name: Default::default(),
            icon: Default::default(),
            accept_textures: Default::default(),
            instance: Default::default(),
        }
    }
}

impl InstanceSettingObj {
    /// 读取实例的服务器列表
    ///
    /// # 返回值
    ///
    /// 服务器列表；文件不存在返回空列表，读取失败返回对应错误
    pub fn get_server_infos(&self) -> CoreResult<Vec<ServerInfoObj>> {
        let file = self.get_servers_file();
        let mut list = Vec::new();
        if file.exists() && file.is_file() {
            let stream = &mut path_helper::open_read(file)?;
            let nbt = NbtFile::read(stream)?;

            if let Some(map) = nbt.nbt.as_compound()
                && let Some(servers) = map.get_list("servers")
            {
                for item in servers.iter() {
                    if let Some(data) = item.as_compound() {
                        let mut server = ServerInfoObj::default();

                        if let Some(name) = data.get_string("name") {
                            server.name = name;
                        }
                        if let Some(ip) = data.get_string("ip") {
                            server.ip = ip;
                        }
                        if let Some(icon) = data.get_string("icon") {
                            server.icon = Some(icon);
                        }
                        if let Some(accept_textures) = data.get_byte("acceptTextures") {
                            server.accept_textures = accept_textures == 1;
                        }

                        list.push(server);
                    }
                }
            }
        }

        Ok(list)
    }

    /// 保存服务器列表，整体覆盖实例的 `servers.dat`
    ///
    /// - `list`: 待写入的服务器列表
    ///
    /// # 返回值
    ///
    /// 成功返回 `Ok(())`；写入失败返回对应错误
    pub fn save_servers(&self, list: &Vec<ServerInfoObj>) -> CoreResult<()> {
        let mut list1 = NbtList::new(NbtType::compound().get_num());
        for item in list.iter() {
            let mut tag = NbtCompound::new();
            tag.data.insert(
                "name".to_string(),
                NbtString::new(item.name.clone()).to_nbt(),
            );
            tag.data
                .insert("ip".to_string(), NbtString::new(item.ip.clone()).to_nbt());

            if let Some(icon) = item.icon.as_ref() {
                tag.data
                    .insert("icon".to_string(), NbtString::new(icon.clone()).to_nbt());
            }

            tag.data.insert(
                "acceptTextures".to_string(),
                NbtByte::new(if item.accept_textures { 1 } else { 0 }).to_nbt(),
            );

            list1.add_item(tag.to_nbt());
        }

        let mut nbt = NbtCompound::new();
        nbt.data.insert("servers".to_string(), list1.to_nbt());

        let file = self.get_servers_file();
        let nbt_file = NbtFile::new(nbt.to_nbt(), CompressType::GZip);
        let stream = &mut path_helper::open_write(file)?;
        nbt_file.write(stream)
    }

    /// 往列表末尾追加一个服务器
    ///
    /// - `name`: 服务器显示名
    /// - `ip`: 服务器地址，形如 `host:port`
    ///
    /// # 返回值
    ///
    /// 成功返回 `Ok(())`；写入失败返回对应错误
    pub fn add_server(&self, name: &str, ip: &str) -> CoreResult<()> {
        let mut list = self.get_server_infos()?;
        list.push(ServerInfoObj {
            ip: ip.to_string(),
            name: name.to_string(),
            ..Default::default()
        });

        self.save_servers(&list)
    }

    /// 删除名字与地址都匹配的服务器条目
    ///
    /// - `name`: 服务器显示名
    /// - `ip`: 服务器地址，形如 `host:port`
    ///
    /// # 返回值
    ///
    /// 成功返回 `Ok(())`；写入失败返回对应错误
    pub fn remove_server(&self, name: &str, ip: &str) -> CoreResult<()> {
        let mut list = self.get_server_infos()?;
        list.retain(|item| !(item.name == name && item.ip == ip));

        self.save_servers(&list)
    }
}
