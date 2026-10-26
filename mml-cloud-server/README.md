# M²L 存档云同步服务器

独立 HTTP 服务，使用 ColorMC 的 SHA1 文件清单、差异 ZIP 与
`cloud_delete.json` 删除清单。启动器目前提供手动存档同步。

## 运行

将 `cloud-server.example.json` 复制为自己的 `cloud-server.json`，替换两种密钥。
示例中的占位符不能用于实际部署。每个密钥至少 16 个字符，建议使用独立随机密钥。
`Clients` 的值是每个客户端账户的配额，单位字节。示例为 1 GiB。

在仓库根运行：

```sh
cargo run -p mml-cloud-server -- mml-cloud-server/cloud-server.json
```

也可构建独立可执行文件：

```sh
cargo build -p mml-cloud-server --release
./target/release/mml-cloud-server /path/to/cloud-server.json
```

Windows 可执行文件为 `target/release/mml-cloud-server.exe`。
唯一位置参数为配置文件；未传入时读取当前工作目录下的 `cloud-server.json`。
JSON 使用无 BOM 的 UTF-8；相对 `DataDir` 以配置文件所在目录为基准。
配置在启动时读取，改密钥和配额后重启生效。Ctrl+C 正常退出。

默认只监听本机。跨电脑使用时，在前面配置 HTTPS 反向代理，并将启动器连接地址设为
该 HTTPS 地址。代理须转发 POST、鉴权头和请求体，允许至少 256 MiB 的请求体以及
最长 300 秒的请求。不要把 `DataDir` 配成静态网站目录。

每个客户端密钥对应一个独立账户。多台电脑使用相同客户端密钥即可访问同一份数据，
不同密钥彼此隔离；`ServerKey` 是服务器共享凭据。配置文件包含密钥，应限制其读取权限。
同一数据目录只允许一个服务进程；备份整个目录前请停止服务。

## 启动器入口

1. 打开实例的“资源管理 → 存档 → 存档云同步”。
2. 填写服务器地址、服务器密钥和客户端密钥，保存并连接。
3. 多台电脑将“云端实例 UUID”设为相同值，然后启用同步。
4. 退出目标游戏后，对具体存档选择上传或下载。

上传使云端匹配本地，下载使本地匹配云端，源端没有的文件也会从目标移除。
操作前会显示确认；暂停同步会保留云端数据。上传使用修订号拒绝并发覆盖。
同步期间启动器阻止该实例启动，并阻止同一实例再次同步。
每次操作使用当前对话框显示的连接参数，其它窗口保存配置不会改变这次操作的目的地。

下载替换前，将本地旧存档保留在实例备份目录的 `cloud/<存档目录>/<备份 UUID>/`。
对话框中的“打开下载前备份”可打开其父目录。需要恢复时，先退出游戏，保留当前目录，
再把所选备份目录复制回该实例 `saves/` 下的原存档名。

单次压缩包最多 256 MiB、一个快照展开后最多 1 GiB、最多 100,000 个文件。
`session.lock` 不参与同步。路径穿越、软链接、Windows 保留名和包内大小写重复会被拒绝。
服务在隔离副本应用更新并检查配额，提交标记支持重启后恢复被中断的替换；
这不等于文件系统断电持久性保证，仍需保留独立备份。

## 协议与验证

支持的 ColorMC POST 端点：

```text
/check /getstate /getlist
/haveuuid /startuuid /stopuuid
/putconfig /pullconfig
/getworldlist /getworldfiles /putworld /pullworld /deleteworld
```

鉴权放在 `serverkey` / `clientkey` 请求头；实例为 `uuid`，存档为 URL 编码的 `name`。
成功 JSON 返回 `res:100`；停用 101、缺失 102、无变化 103、限额 104、
修订冲突 105、鉴权失败 300。下载成功返回 ZIP，无变化为 HTTP 400 + JSON，
保持 ColorMC 用 HTTP 状态区分 ZIP 和 JSON 的行为。

相关测试只使用 `target/temp` 中的临时数据与本机随机端口：

```sh
cargo test -p mml-cloud --test archive
cargo test -p mml-cloud-server --lib --test protocol
cargo test -p mml-gamepad --lib
```

`protocol` 通过真实 HTTP 覆盖同一修订号的并发更新、跨存档的账户配额竞争，
以及关闭后重新初始化服务的存档、启用状态与用量持久化。
配额竞争失败后还会检查临时副本清理，以及释放空间后的重试。
中断替换的恢复标记另由存储单元测试验证。

## English

This standalone service follows ColorMC's SHA1 manifest and delta ZIP protocol, including
`cloud_delete.json`. Copy the example config, replace both placeholder keys with separate random
secrets of at least 16 characters, and run the command above. Quotas are per client key in bytes.
Relative data paths resolve against the config directory. Configuration changes require a restart.

Use an HTTPS reverse proxy for access from other computers. Forward POST requests and custom
headers; allow bodies up to 256 MiB and requests lasting 300 seconds. Keep the data directory
private and stop the server before making a full directory backup.

In the launcher, open an instance's Resources → Saves → Save cloud sync. Save the connection,
enable sync, and use the same client key and remote instance UUID on each computer.
Sync is manual and requires the game to be stopped. Upload and download replace the destination
with the source contents, including deletions. Downloads retain the previous local save under
`cloud/<save-name>/<backup-uuid>/` in the instance backup directory. Stop the game before restoring
a backup into its original `saves/` directory.

Requests are limited to 256 MiB compressed, 1 GiB unpacked and 100,000 files per snapshot.
Different client keys are isolated. Quota failures leave the prior snapshot in place, and upload
revision checks reject concurrent changes. Interrupted replacements can be recovered on startup;
keep independent backups for disk failure or power loss.

The HTTP tests also cover concurrent updates from the same revision, quota competition between
different saves, staging cleanup and retry after freeing space. Restart tests close and recreate
the service against the same data directory, then verify saves, enabled states and storage usage.
Storage unit tests separately cover interrupted replacement recovery markers.
