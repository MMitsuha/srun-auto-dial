# srun-auto-dial

深澜（Srun）校园网认证自动拨号工具，支持 TUI 交互模式和 REST API 服务器模式。

支持三种 MAC 地址模式：本机网卡、自定义 MAC（macvlan）、随机 MAC 批量拨号。

> **Linux only** — 依赖 netlink 和 raw socket，需要 root 或 `CAP_NET_ADMIN` + `CAP_NET_RAW` 权限。

## 快速开始

### 编译

```bash
cargo build --release
```

### TUI 模式

```bash
sudo ./target/release/srun-auto-dial tui
```

交互式选择网卡、拨号模式（Local / Custom MAC / Random MAC）和操作（Login / Logout / Status）。

### API 服务器模式

```bash
sudo ./target/release/srun-auto-dial server
```

默认监听 `127.0.0.1:3000`，可通过配置文件或命令行参数覆盖：

```bash
sudo ./target/release/srun-auto-dial server --port 8080 --host 0.0.0.0
```

### Docker

```bash
docker run --rm --net=host --cap-add=NET_ADMIN --cap-add=NET_RAW \
  -v ./srun.toml:/etc/srun-auto-dial/srun.toml \
  ghcr.io/<owner>/srun-auto-dial
```

> Docker 镜像默认以 server 模式运行。需要 `--net=host` 以访问宿主机网络接口。

## 配置

复制 `srun.toml.example` 为 `srun.toml` 进行配置：

```toml
portal_url = "http://portal.hdu.edu.cn"
ac_id = "1"
userinfo_path = "userinfo.json"

[server]
host = "127.0.0.1"
port = 3000
# api_key = "your-secret-key"
```

用户凭据存放在 `userinfo.json`：

```json
[
    {
        "username": "your-username",
        "password": "your-password"
    }
]
```

可通过 `-c` 参数指定配置文件路径：

```bash
srun-auto-dial -c /path/to/srun.toml tui
```

## REST API

| 方法 | 路径 | 说明 |
|------|------|------|
| GET | `/api/health` | 健康检查 |
| GET | `/api/interfaces` | 列出可用网络接口 |
| GET | `/api/status?interface=eth0` | 查询在线状态 |
| POST | `/api/login/local` | 本机网卡登录 |
| POST | `/api/logout/local` | 本机网卡登出 |
| POST | `/api/login/macvlan` | 自定义 MAC 登录（macvlan） |
| POST | `/api/logout/macvlan` | macvlan 登出 |
| POST | `/api/login/random` | 随机 MAC 批量登录 |

### 请求示例

```bash
# 登录
curl -X POST http://127.0.0.1:3000/api/login/local \
  -H "Content-Type: application/json" \
  -d '{"interface":"eth0","username":"user","password":"pass"}'

# 查询状态
curl "http://127.0.0.1:3000/api/status?interface=eth0"

# 自定义 MAC 登录
curl -X POST http://127.0.0.1:3000/api/login/macvlan \
  -H "Content-Type: application/json" \
  -d '{"parent_interface":"eth0","mac_address":"aa:bb:cc:dd:ee:ff","username":"user","password":"pass"}'

# 随机 MAC 批量登录
curl -X POST http://127.0.0.1:3000/api/login/random \
  -H "Content-Type: application/json" \
  -d '{"parent_interface":"eth0","count":5}'
```

### API 认证

在 `srun.toml` 中设置 `api_key` 后，所有请求需要携带认证头：

```bash
curl -H "X-API-Key: your-secret-key" http://127.0.0.1:3000/api/status?interface=eth0
# 或
curl -H "Authorization: Bearer your-secret-key" http://127.0.0.1:3000/api/status?interface=eth0
```

## 日志

通过 `-v` 标志控制日志级别：

```bash
srun-auto-dial -v tui       # info
srun-auto-dial -vv server   # debug
srun-auto-dial -vvv tui     # trace
```

## 项目结构

```
src/
├── main.rs        # clap 入口
├── error.rs       # 错误类型（thiserror）
├── config.rs      # TOML 配置
├── service.rs     # 核心业务逻辑
├── srun/          # Srun 协议实现
├── net/           # 网络操作（netlink + DHCP）
├── tui/           # 交互式 TUI
└── api/           # REST API（axum）
```

## License

MIT
