# srun-auto-dial

深澜（Srun）校园网认证工具，包含 Linux 命令行/TUI、REST API 后端和 Next.js 管理界面。支持本机网卡、自定义 MAC 的 macvlan，以及受控的随机 MAC 批量拨号。

> [!IMPORTANT]
> 后端仅支持 Linux。创建 macvlan、配置地址/路由和发送原始 DHCP 数据包需要 root，或等效的 `CAP_NET_ADMIN` 与 `CAP_NET_RAW` 能力。

## 服务与端口

| 组件 | 默认地址 | 用途 |
|---|---|---|
| Rust REST API | `http://127.0.0.1:3000` | 直接 API，路由前缀为 `/api/*` |
| Next.js Web | `http://127.0.0.1:3001` | 浏览器管理界面 |
| Web 后端代理 | Web 同源 `/api/backend/*` | 服务端转发到 Rust API，并注入可选 API key |

浏览器不会直接读取 `API_URL` 或 `API_KEY`。页面只请求同源的 `/api/backend/*`；Next.js Route Handler 在服务端把请求转发到 Rust 后端的 `/api/*`。因此 `API_KEY` 必须只配置在 Web 服务进程或容器中，不应使用 `NEXT_PUBLIC_*` 暴露给客户端。

## 前置要求

- Linux 后端主机；内核支持 macvlan 和 netlink。
- Rust stable 工具链（edition 2024）。
- Bun，用于 Web 开发、测试和构建。
- 运行 macvlan 模式时具备 `CAP_NET_ADMIN` 和 `CAP_NET_RAW`。

## 本地开发

### 1. 准备配置与凭据

```bash
cp srun.toml.example srun.toml
cp userinfo.json.example userinfo.json
```

按学校环境修改 `srun.toml` 和 `userinfo.json`。`srun.toml`、`userinfo.json` 以及 Web `.env*` 文件均可能包含敏感信息，已被 Git 忽略。

### 2. 启动后端

构建并运行 TUI：

```bash
cargo build --release --locked
sudo ./target/release/srun-auto-dial tui
```

或启动 REST API（默认 `127.0.0.1:3000`）：

```bash
sudo cargo run --locked -- server
```

命令行参数可覆盖监听地址；全局参数必须放在子命令之前：

```bash
sudo cargo run --locked -- -c srun.toml -vv server --host 0.0.0.0 --port 3000
```

日志级别由 `-v`、`-vv`、`-vvv` 控制。

### 3. 启动 Web

在另一个终端运行：

```bash
cd web
bun install --frozen-lockfile
API_URL=http://127.0.0.1:3000 bun run dev
```

打开 <http://127.0.0.1:3001>。如果后端启用了 API key：

```bash
API_URL=http://127.0.0.1:3000 \
API_KEY=your-secret-key \
bun run dev
```

Web 开发服务器固定使用端口 `3001`；Rust 后端继续使用 `3000`，不会发生端口冲突。

## 配置

默认读取当前工作目录中的 `srun.toml`。隐式配置文件不存在时使用默认值；其他读取、TOML 解析或字段校验错误会明确报告。也可以通过全局 `-c/--config` 指定路径。

```toml
portal_url = "https://portal.example.edu"
ac_id = "1"
userinfo_path = "userinfo.json"

[server]
host = "127.0.0.1"
port = 3000
# api_key = "replace-with-a-long-random-secret"
```

- `portal_url` 必须是绝对 `http://` 或 `https://` URL，且不能包含 query 或 fragment；HTTPS 使用 rustls。
- `ac_id` 不能为空。
- `userinfo_path` 是后端主机上的路径。REST/Web 请求中的同名字段也是服务器路径，不是浏览器本地文件。
- `server.api_key` 可选；设置后不能是空字符串，所有直接后端 API 请求都必须认证。
- 将后端暴露到非回环地址时，建议设置 API key 并配合防火墙或反向代理 TLS。

### 凭据文件格式与限制

凭据文件必须是 JSON 数组：

```json
[
  {
    "username": "campus-user-1",
    "password": "secret-1"
  },
  {
    "username": "campus-user-2",
    "password": "secret-2"
  }
]
```

后端会拒绝不符合以下规则的文件：

- 文件大小不超过 1 MiB（1,048,576 字节）。
- 数组包含 1 至 10,000 个用户。
- `username` 不能为空，且同一文件内必须唯一。
- `password` 不能为空。
- 每个数组元素都必须包含字符串 `username` 和 `password`。

不同运营商或线路建议使用不同 JSON 文件，不要混用。TUI 可以选择文件；REST 与 Web 可通过 `userinfo_path` 覆盖配置默认值。手动凭据必须同时提供 `username` 和 `password`，且不能与 `userinfo_path` 同时使用。

## Web 运行时环境变量

| 变量 | 默认值 | 说明 |
|---|---|---|
| `PORT` | `3001`（容器已设置） | Next.js standalone 服务监听端口 |
| `API_URL` | `http://127.0.0.1:3000` | Rust 后端基地址，仅在 Next.js 服务端读取 |
| `API_KEY` | 未设置 | 转发时写入 `X-API-Key`，仅在 Next.js 服务端读取 |
| `API_TIMEOUT_MS` | `75000` | 普通代理请求超时 |
| `API_BATCH_TIMEOUT_MS` | `7200000` | 随机批量代理请求超时（默认 2 小时，以覆盖 100 次顺序重试的最坏情况） |

浏览器侧请求还会做略长的保护性超时，以便代理先返回结构化的 504 错误。

## Docker

### 后端

后端镜像的工作目录是 `/etc/srun-auto-dial`，因此下面挂载的 `srun.toml` 会被默认配置发现逻辑直接读取。凭据文件也挂载到相同目录时，配置中的相对路径 `userinfo.json` 可以正常解析。

```bash
docker run --rm \
  --network host \
  --cap-add NET_ADMIN \
  --cap-add NET_RAW \
  -v "$PWD/srun.toml:/etc/srun-auto-dial/srun.toml:ro" \
  -v "$PWD/userinfo.json:/etc/srun-auto-dial/userinfo.json:ro" \
  ghcr.io/<owner>/srun-auto-dial:latest
```

`--network host` 是必需的：后端需要看到并操作宿主机真实网卡。镜像默认运行 `srun-auto-dial server`，后端端口为 `3000`。

### Web

同一 Linux 主机上可以让 Web 容器也使用 host 网络：

```bash
docker run --rm \
  --network host \
  -e PORT=3001 \
  -e API_URL=http://127.0.0.1:3000 \
  -e API_KEY=your-secret-key \
  ghcr.io/<owner>/srun-auto-dial-web:latest
```

打开 <http://127.0.0.1:3001>。Web 镜像设置并暴露 `PORT=3001`。

如果希望 Web 保持 bridge 网络，可将后端配置为可从 Docker host gateway 访问，然后运行：

```bash
docker run --rm \
  -p 3001:3001 \
  --add-host host.docker.internal:host-gateway \
  -e PORT=3001 \
  -e API_URL=http://host.docker.internal:3000 \
  -e API_KEY=your-secret-key \
  ghcr.io/<owner>/srun-auto-dial-web:latest
```

这种模式下，后端不能只绑定 `127.0.0.1`；请使用受防火墙保护的 `0.0.0.0:3000` 或合适的宿主机地址。

## REST API

直接调用 Rust 后端时使用 `/api/*`。Web 的 `/api/backend/*` 是同源代理路径，例如浏览器的 `/api/backend/status` 会转发到后端 `/api/status`。

| 方法 | 后端路径 | 说明 |
|---|---|---|
| GET | `/api/health` | 健康检查 |
| GET | `/api/interfaces` | 列出可选网络接口 |
| GET | `/api/status?interface=eth0` | 查询本机接口状态 |
| POST | `/api/login/local` | 本机接口登录 |
| POST | `/api/logout/local` | 本机接口登出 |
| POST | `/api/status/macvlan` | 使用指定 MAC 查询状态 |
| POST | `/api/login/macvlan` | 使用指定 MAC 登录 |
| POST | `/api/logout/macvlan` | 使用指定 MAC 登出 |
| POST | `/api/login/random` | 随机 MAC 批量登录，`count` 为 1–100 |

### 认证

配置 `server.api_key` 后，可使用任一种请求头：

```bash
curl -H 'X-API-Key: your-secret-key' \
  http://127.0.0.1:3000/api/status?interface=eth0

curl -H 'Authorization: Bearer your-secret-key' \
  http://127.0.0.1:3000/api/status?interface=eth0
```

Web 用户不需要在浏览器中设置请求头；Next.js 代理会从服务端 `API_KEY` 注入认证信息。

### 请求示例

```bash
# 手动凭据登录
curl -X POST http://127.0.0.1:3000/api/login/local \
  -H 'Content-Type: application/json' \
  -H 'X-API-Key: your-secret-key' \
  -d '{"interface":"eth0","username":"user","password":"pass"}'

# 使用后端配置的凭据文件
curl -X POST http://127.0.0.1:3000/api/login/local \
  -H 'Content-Type: application/json' \
  -H 'X-API-Key: your-secret-key' \
  -d '{"interface":"eth0"}'

# 使用指定服务器端凭据文件执行随机批量登录
curl -X POST http://127.0.0.1:3000/api/login/random \
  -H 'Content-Type: application/json' \
  -H 'X-API-Key: your-secret-key' \
  -d '{"parent_interface":"eth0","count":5,"userinfo_path":"line-a.json"}'
```

### 统一响应与错误

成功响应：

```json
{
  "success": true,
  "data": {
    "ip": "10.0.0.8",
    "username": "campus-user",
    "mac": null
  }
}
```

失败响应始终使用结构化错误；`field` 仅在错误关联具体输入字段时出现：

```json
{
  "success": false,
  "error": {
    "code": "validation_failed",
    "message": "count must be between 1 and 100.",
    "field": "count"
  }
}
```

未知路由、错误 HTTP 方法、无效 JSON、认证失败、Portal/DHCP 错误也使用同一 JSON envelope。内部诊断写入后端日志，不直接泄露给客户端。

### 随机批量响应

批量接口返回摘要和逐次结果，而不是 Rust `Result` 的 `{Ok|Err}` 编码：

```json
{
  "success": true,
  "data": {
    "requested": 2,
    "attempted": 2,
    "succeeded": 1,
    "failed": 1,
    "results": [
      {
        "mac": "02:11:22:33:44:55",
        "success": true,
        "data": {
          "ip": "10.0.0.8",
          "username": "campus-user-1",
          "mac": "02:11:22:33:44:55"
        }
      },
      {
        "mac": "02:66:77:88:99:aa",
        "success": false,
        "error": {
          "code": "authentication_failed",
          "message": "The portal rejected the login request."
        }
      }
    ]
  }
}
```

完整数据形状为：

```text
{
  requested,
  attempted,
  succeeded,
  failed,
  stopped_reason?,
  results: [
    { mac, success, data?, error?: { code, message, field? } }
  ]
}
```

每个账号在单个批次内最多尝试 3 次。当账号池不足以满足请求数量时，`attempted` 可能小于 `requested`，并返回 `stopped_reason`。

## 网络操作安全模型

- macvlan 与本机登录/登出操作通过 network-namespace 级抽象 Unix socket 串行执行；同一网络命名空间中的其他进程或 host-network 容器也会收到 `409 operation_in_progress`，避免接口和路由互相干扰。
- 该 namespace 锁是同一主机/网络命名空间内受信进程之间的协作锁；若允许不受信本地进程或容器进入相同 network namespace，应通过宿主机隔离策略阻止其抢占抽象 socket 名称造成拒绝服务。
- 每次操作创建唯一的 `srnxxxxxxxxxxxx` 临时接口，不会复用或预先删除固定名称；持有 namespace 锁的下一次操作会安全清理同格式残留接口。
- 正常路径会显式清理临时接口；请求被取消时也会安排尽力清理。
- 清理重试仍失败时不会静默返回成功：API 使用 `cleanup_failed_after_success` 或 `operation_failed_cleanup_incomplete` 明确报告部分完成状态，随机批次会立即停止。
- DHCP 租约 IP 会与 Portal 看到的 IP 比较，不一致时停止认证。
- 本机登录/登出修改操作也使用同一跨进程锁；只读状态查询不受影响。

## 质量检查

后端检查必须在 Linux 上运行：

```bash
cargo fmt --all -- --check
cargo check --locked --all-targets
cargo test --locked --all-targets
cargo clippy --locked --all-targets -- -D warnings
```

Web 检查：

```bash
cd web
bun install --frozen-lockfile
bun run lint
bun run typecheck
bun test
bun run build
```

Web lint 使用仓库中的 ESLint flat config，并将 warning 视为 CI 失败。

## Web 页面

| 路径 | 功能 |
|---|---|
| `/` | Dashboard：接口选择和在线状态 |
| `/login` | Connect：本机、自定义 macvlan、随机批量登录 |
| `/logout` | Disconnect：本机或指定 macvlan 登出 |

## 项目结构

```text
.
├── src/                         # Rust TUI、服务层、Srun、DHCP/netlink、REST API
├── web/
│   ├── src/app/                 # Next.js 页面与同源后端代理
│   ├── src/components/          # UI 组件
│   ├── src/lib/                 # API 客户端与验证
│   └── tests/                   # Bun 单元测试
├── .github/workflows/ci.yml     # Rust、Web 和容器 CI
├── Dockerfile                   # Rust API 镜像
└── srun.toml.example            # 配置模板
```

## License

MIT
