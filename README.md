# net-stats-tui

`net-stats-tui` 是一个基于终端的实时网络速率监控工具。它会按配置读取 Linux 网卡在 `/sys/class/net/<iface>/statistics` 下的计数器，并以 TUI 表格形式展示每个接口的收发速率。

## 特性

- 读取多个网络接口并分别展示别名
- 按固定刷新间隔实时计算 RX/TX 速率
- 配置简单，使用 TOML 文件即可启动
- 纯终端界面，适合服务器和桌面环境

## 运行要求

- Linux
- Rust 1.85+（项目使用 Edition 2024）
- 可访问 `/sys/class/net`

## 从源码构建

```bash
cargo build --release
```

构建完成后二进制位于：`target/release/net-stats-tui`

## 配置

复制示例配置：

```bash
cp net-stats.toml.example net-stats.toml
```

示例配置：

```toml
refresh_ms = 1000

[[interfaces]]
alias = "wan"
device = "eth0"

[[interfaces]]
alias = "wifi"
device = "wlan0"
```

字段说明：

- `refresh_ms`: 刷新周期，单位毫秒，默认 `1000`
- `interfaces[].alias`: 界面显示名称
- `interfaces[].device`: Linux 网络设备名，例如 `eth0`、`wlan0`、`enp3s0`

## 使用方式

默认读取当前目录下的 `net-stats.toml`：

```bash
./target/release/net-stats-tui
```

也可以显式传入配置文件路径：

```bash
./target/release/net-stats-tui /path/to/net-stats.toml
```

运行后：

- 表格会显示每个接口的接收速率和发送速率
- 如果接口不存在或读取失败，会在对应行显示错误信息
- 按 `q` 退出程序

## GitHub Release 自动构建

仓库包含一个 GitHub Actions 工作流：`.github/workflows/release.yml`

行为如下：

- 当推送形如 `v*` 的标签时自动触发
- 使用 `cargo build --locked --release` 构建 Linux 版本
- 打包生成 `net-stats-tui-<tag>-x86_64-unknown-linux-gnu.tar.gz`
- 自动上传该压缩包到对应 GitHub Release

发布示例：

```bash
git tag v0.1.0
git push origin v0.1.0
```

如果该标签对应的 Release 尚未存在，工作流会自动创建并附带默认 Release Notes。

## Release 包内容

压缩包内包含：

- `net-stats-tui-<tag>-x86_64-unknown-linux-gnu` 二进制文件
- `net-stats.toml.example`
- `README.md`
