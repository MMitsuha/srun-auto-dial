# srun-auto-dial

深澜（Srun）校园网认证自动拨号工具，支持 TUI 交互模式、REST API 服务器和 Web 管理界面。

支持三种 MAC 地址模式：本机网卡、自定义 MAC（macvlan）、随机 MAC 批量拨号。

> **Linux only** — 依赖 netlink 和 raw socket，需要 root 或 `CAP_NET_ADMIN` + `CAP_NET_RAW` 权限。

## 快速开始

### 编译后端

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

### Web 前端

```bash
cd web
bun install
bun run dev
```

访问 `http://localhost:3000` 打开管理界面。需要后端 API 服务器已在运行。

通过环境变量配置 API 地址：

```bash
NEXT_PUBLIC_API_URL=http://192.168.1.1:3000 bun run dev
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

## Web 管理界面

基于 Next.js + TypeScript + Tailwind CSS 构建，采用 nextjs.org 风格的暗色极简设计。

| 页面 | 路径 | 功能 |
|------|------|------|
| Dashboard | `/` | 网卡选择、在线状态查看、快速登录/登出 |
| Login | `/login` | Local / Macvlan 模式登录表单 |
| Random | `/random` | 随机 MAC 批量拨号，结果表格展示 |

## REST API

| 方法 | 路径 | 说明 |
|------|------|------|
| GET | `/api/health` | 健康检查 |
| GET | `/api/interfaces` | 列出可用网络接口 |
| GET | `/api/status?interface=eth0` | 查询在线状态 |
| POST | `/api/login/local` | 本机网卡登录（凭据可选） |
| POST | `/api/logout/local` | 本机网卡登出 |
| POST | `/api/login/macvlan` | 自定义 MAC 登录（凭据可选） |
| POST | `/api/logout/macvlan` | macvlan 登出 |
| POST | `/api/login/random` | 随机 MAC 批量登录 |

> `login/local` 和 `login/macvlan` 的 `username`/`password` 字段为可选。省略时自动从 `userinfo.json` 中随机选取一组凭据。

### 请求示例

```bash
# 登录（手动指定凭据）
curl -X POST http://127.0.0.1:3000/api/login/local \
  -H "Content-Type: application/json" \
  -d '{"interface":"eth0","username":"user","password":"pass"}'

# 登录（使用 userinfo.json 中的凭据）
curl -X POST http://127.0.0.1:3000/api/login/local \
  -H "Content-Type: application/json" \
  -d '{"interface":"eth0"}'

# 查询状态
curl "http://127.0.0.1:3000/api/status?interface=eth0"

# 自定义 MAC 登录（凭据可选）
curl -X POST http://127.0.0.1:3000/api/login/macvlan \
  -H "Content-Type: application/json" \
  -d '{"parent_interface":"eth0","mac_address":"aa:bb:cc:dd:ee:ff"}'

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
├── src/               # Rust 后端
│   ├── main.rs        # clap 入口
│   ├── error.rs       # 错误类型（thiserror）
│   ├── config.rs      # TOML 配置
│   ├── service.rs     # 核心业务逻辑
│   ├── srun/          # Srun 协议实现
│   ├── net/           # 网络操作（netlink + DHCP）
│   ├── tui/           # 交互式 TUI
│   └── api/           # REST API（axum）
└── web/               # Next.js 前端
    └── src/
        ├── app/       # 页面（Dashboard / Login / Random）
        ├── components/# UI 组件
        └── lib/       # API 客户端
```

## License

MIT
