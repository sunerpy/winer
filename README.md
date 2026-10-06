<div align="center">

<img src="./app/src-tauri/icons/128x128@2x.png" alt="winer" width="96" />

# winer

### 英雄联盟客户端助手：队友战绩、对局评级与自动化，直接读客户端自己的接口

[![CI](https://github.com/sunerpy/winer/actions/workflows/ci.yml/badge.svg)](https://github.com/sunerpy/winer/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/sunerpy/winer)](https://github.com/sunerpy/winer/releases)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue)](./LICENSE)

[安装](#安装) · [功能](#功能) · [评分与评级](#评分与评级) · [开发](#开发) · [致谢](#致谢) · [免责声明](#免责声明) · [使用文档](https://firlab.app/winer/)

English documentation: [firlab.app/winer/en](https://firlab.app/winer/en/)

</div>

---

一个直接与 LCU 通信的 Rust 内核，一个 Tauri 桌面窗口，以及一个经由 Pengu Loader 注入客户端页面的插件。
只做 Windows 版，在腾讯客户端（国服）上实测。

## 安装

需要 64 位 Windows 10 或 11（系统自带 WebView2）。安装只装到当前用户，不需要管理员权限。

**一行命令**（PowerShell）：下载最新版的安装包，按发布里的 `SHA256SUMS` 校验通过后静默安装。

```powershell
irm https://github.com/sunerpy/winer/releases/latest/download/install.ps1 | iex
```

指定版本时设置 `WINER_VERSION`，脚本也取同一个版本里的那份：

```powershell
$env:WINER_VERSION = "0.0.1"; irm https://github.com/sunerpy/winer/releases/download/v0.0.1/install.ps1 | iex
```

**安装包**：在 [Releases](https://github.com/sunerpy/winer/releases) 下载 `winer_<版本>_x64-setup.exe` 双击运行。
安装包没有代码签名证书，SmartScreen 会提示「已保护你的电脑」，点「更多信息 → 仍要运行」。原因和校验办法见
[`docs/accepted-tradeoffs.md`](docs/accepted-tradeoffs.md)；每个发布文件都可以对照 `SHA256SUMS`，或用
GitHub 的构建证明核对来源：

```bash
gh attestation verify winer_0.0.1_x64-setup.exe --repo sunerpy/winer \
  --signer-workflow sunerpy/winer/.github/workflows/release.yml
```

装好以后打开 winer 即可，其余它自己完成：国服客户端以管理员身份运行时，它请求以管理员身份重启（只有系统的确认
提示）；连上客户端后装好自带的 [Pengu Loader](https://github.com/PenguLoader/PenguLoader) 和客户端插件
（第一次激活要在客户端目录里创建链接，Windows 只允许管理员创建，winer 没有管理员权限时同样请求以管理员身份
重启一次），Pengu 自己的弹窗不会出现；启动后和运行期间自动检查更新，有新版本时标题栏出现按钮，一键升级。

## 功能

| 页面       | 能做什么                                                                                                                                                                                                                                                                                                                                                                                   |
| ---------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| 概览       | 召唤师、段位、近期状态（胜率、KDA、连胜、常用英雄）、自动化开关、动态、好友动态（正在选人或游戏中的好友，模式与已进行时间，一起玩的好友同色标出，点好友进入战绩）、最近对局                                                                                                                                                                                                                |
| 对局       | 对局分析：选人与对局中按队伍逐行列出每位玩家的段位、近期胜率与 KDA，以及最近 6–12 局（胜负、战绩、模式），点玩家进入战绩；我方/敌方切换与红蓝方；开黑标记（每组一种颜色并保留编号，选人时标出同一房间进来的队友）；战力评级、毒舌称号与评语；战力喊话（首行带红蓝方和 winer 署名，每行写选人位置 1L–5L 和名称）；大乱斗备选席一键换、重随；组队房间里列出每位成员的段位、近期战绩与战力分  |
| 战绩       | 按 Riot ID 查询任意玩家；分页浏览全部战绩（每页 10/15/25/50），头像上标出 MVP/SVP，时间后面是本局成就（多杀、超神、一血、各项最多、逃兵）；按模式筛选；海克斯大乱斗等模式显示每人的海克斯符文（稀有度与效果说明）；展开对局记分板：winer 评分与单局评级、MVP/SVP、成就小图标、单局称号、伤害与伤害占比、承伤、参团率、经济、补刀、装备，全场伤害/承伤/经济最高、死亡最少、评分最高单独标出 |
| 自动化     | 每项都可限定适用模式（排位、匹配、极地大乱斗、海克斯大乱斗、斗魂竞技场、其他），按模式查看各模式下生效的项目；自动接受（可设等待）；按分路的英雄顺序自动选择/禁用，规划阶段亮出意向；对局结束自动返回房间；进入选人自动喊话（开场白、每行内容可自定义，带实时预览）；大乱斗心愿英雄自动换                                                                                                  |
| 工具       | 在线状态（在线、离开、手机在线、隐身，和客户端里切换的效果相同），可记住并在客户端重置后改回；个性签名；重启客户端界面；生涯背景（全部皮肤可选，含未拥有的）；挑战徽章与称号；段位伪装（只改变好友看到的段位，默认关闭）；游戏设置与按键备份、恢复（最多 10 份，可导入）                                                                                                                   |
| 客户端增强 | 自带 Pengu Loader，连上客户端时自动激活并装好插件，可停用；选人界面队友战绩、评级与称号、自己所在的红蓝方和开黑编号；在客户端里直接点备选席英雄秒换（开关，不等冷却）；好友列表里好友对局的模式与已进行时间，一起玩的好友同色竖条；组队房间成员横幅上方的近期胜率、KDA 与战力分，点击在 winer 里打开战绩；隐藏首页推广                                                                     |
| 设置       | 五套主题（含海克斯）、强调色、密度、字号、减少动态、语言、关闭到托盘、唤起 winer 的全局快捷键（默认 Ctrl+Shift+W，游戏中唤出时置顶）、联网获取海克斯符文说明、开机自启、软件更新；评级方案、自定义档位、毒舌称号开关与评价依据                                                                                                                                                             |

### 评分与评级

两个数字都是 winer 自己的公式，写在 `crates/core/src/rating.rs`，不复刻客户端、WeGame 或其他工具的算法；对局评分的权重对照 WeGame 的 MVP、SVP 校准过。
完整规则见[评级说明](https://firlab.app/winer/rating)（源文件 [`docs/site/rating.md`](docs/site/rating.md)），窗口里在 **设置 › 评级**。

- **对局评分（0–10）**：把经济、存活、击杀、助攻、对英雄伤害、承伤，以及峡谷里的补刀和视野，分别与本局十人的
  平均值相比（每项最多计三倍），加权平均后经逻辑曲线映射。全场平均为 6.0，1.25 倍平均为 8.0，0.75 倍为 3.6。
  胜方最高分为 MVP，败方最高分为 SVP。WeGame 不公开它的评分方法，这组权重是拿 WeGame 打过分的对局拟合的：
  峡谷 126 局里 MVP、SVP 与 WeGame 一致的分别有 85% 和 83%，海克斯大乱斗 139 局里是 76% 和 70%。单局评级
  S+ 到 F，平均水平是 B；重开局不评分。
- **近期战力（0–10）**：近二十场的胜率与 KDA 各占一半，再按 `场次 / (场次 + 5)` 向 5.0 收缩，
  一两场的偶然好成绩排不到前面。默认的峡谷五档把一队五人从好到差排成峡谷通天代 / 人形防御塔 /
  峡谷公务员 / 移动眼位 / 纯正牛马，每档配一句毒舌评语；也可以换成按固定分段定级的峡谷八档（S+ 到 F）、
  马系宇宙、赛马五档、赛马三档、峡谷食物链，或者自己写 2–5 个档位名称。
- **毒舌称号**：按数据特征给出，比如三连胜是“版本答案”，输了但伤害占全队三成以上是“院长”；可以在设置里关掉。

### 战绩从哪里来

腾讯客户端自己的战绩接口（LCU）只给最近 20 场，而且不管请求第几页都返回同一批；分页靠的是所在大区的
战绩服务器（SGP），用客户端自己的登录令牌访问，可以翻完整个战绩（实测 160 场）。连不上 SGP 时退回
客户端的 20 场，并在列表下方说明。实测细节见 `docs/platform-notes.md`。

## 结构

| 路径              | 内容                                                                       |
| ----------------- | -------------------------------------------------------------------------- |
| `crates/lcu`      | LCU 传输：凭据发现（进程命令行，lockfile 兜底）、TLS 校验、REST、WAMP 事件 |
| `crates/core`     | 领域内核：快照与增量、选人/对局视图、自动化决策、评分、喊话、插件桥、设置  |
| `app/src-tauri`   | Tauri 外壳：命令、托盘、窗口、`lcu` 资源协议、提权重启、日志、更新         |
| `app/frontend`    | 桌面窗口（React 19 + Tailwind v4）                                         |
| `plugin`          | Pengu Loader 插件（纯 TypeScript DOM，适配客户端的 Chromium 108）          |
| `packages/shared` | `ts-rs` 生成的类型（`bindings.ts`）与共享格式化函数                        |
| `fixtures`        | 从真实客户端抓取的 LCU 响应，供测试使用                                    |
| `scripts/windows` | Windows 真机验收脚本                                                       |

设计规范见 `DESIGN.md`；实测的平台事实见 `docs/platform-notes.md`。

## 开发

```bash
pnpm install
pnpm dev                     # 浏览器预览，使用内置演示数据
cargo test --workspace       # Rust 全部测试（含 TypeScript 类型漂移检查）
pnpm test                    # 前端、插件、共享包测试
pnpm lint && pnpm typecheck && pnpm format:check
cargo clippy --workspace --all-targets -- -D warnings
```

Rust 侧的视图类型改动后，用 `UPDATE_BINDINGS=1 cargo test -p winer-core bindings` 重新生成
`packages/shared/src/bindings.ts`。

## 构建 Windows 版本

```bash
pnpm build
cargo xwin build --release --target x86_64-pc-windows-msvc -p winer --features custom-protocol
```

`custom-protocol` 不能省：没有它，前端资源不会嵌入，窗口会是空白的。安装包由发布流程在 Windows 上用
`pnpm tauri build` 和 `pnpm tauri bundle` 生成。

## 发布

版本号只写在根目录的 `package.json`（`tauri.conf.json` 和 `app/src-tauri/build.rs` 都读它），由
release-please 维护：改动经 squash 合并进 `main` 后，它开出 `chore: release X.Y.Z` 的发布 PR，合并后
`.github/workflows/release.yml` 在 Windows 上构建安装包，校验更新签名，生成 `latest.json`、`SHA256SUMS`
和构建证明，全部核对无误才公开发布。0.1.0 之前每次只增加最后一位版本号。规则写在 `AGENTS.md`。

## 真机验收

`scripts/windows/qa.sh` 在 Linux 上交叉编译带 `qa` 特性的版本，部署到 Windows 主机，在控制台会话中启动，
再经 WebView2 的调试端口截图、执行脚本，不注入任何键鼠输入：

```bash
scripts/windows/qa.sh build && scripts/windows/qa.sh deploy
scripts/windows/qa.sh shot overview          # target/qa/overview.png
scripts/windows/qa.sh log 40                 # 当天日志末尾
```

`qa` 特性会打开调试端口，任何本机进程都能借此操控窗口，因此只用于验收，不得进入发布版本。

## 致谢

- [Pengu Loader](https://github.com/PenguLoader/PenguLoader)（MIT）：把插件载入客户端界面。winer 内置了它的
  `core.dll`，许可原文见 [`THIRD_PARTY_NOTICES.md`](THIRD_PARTY_NOTICES.md)。
- [Tauri](https://tauri.app)、Rust 与 React 的开源生态：窗口与内核建立在它们之上，完整依赖见 `Cargo.lock` 与
  `pnpm-lock.yaml`。
- [ARAM.GG](https://aramgg.com)：海克斯大乱斗强化符文的效果说明。

## 免责声明

- winer 是非官方的第三方工具，与 Riot Games、腾讯及 WeGame 没有任何关联，未获它们认可、赞助或支持。英雄联盟
  （League of Legends）及相关名称、标志和图片是 Riot Games, Inc. 的商标或财产。
- winer 只使用客户端在本机开放的接口（LCU），不读取或修改游戏进程的内存，不修改游戏文件，也不在对局中替你操作。
  客户端增强经由 Pengu Loader 在客户端界面里运行插件，会改变客户端界面的显示。
- 第三方工具都可能违反游戏的服务条款，使用 winer 可能使账号受到限制，风险由使用者自行承担。
- 评分、评级、称号和喊话只是基于战绩数据的娱乐性参考，不代表任何人的真实水平；请勿用来辱骂或骚扰其他玩家。
- 段位伪装只改变好友在好友列表和资料卡里看到的段位，不改变真实段位、匹配和客户端里你自己的资料；好友看到的并不是
  你的真实段位，请勿用它误导他人。
- 软件按 MIT 许可「按原样」提供，不附带任何担保，作者不对使用造成的损失负责。

## 许可

MIT，见 `LICENSE`。随附的第三方组件及其许可见 [`THIRD_PARTY_NOTICES.md`](THIRD_PARTY_NOTICES.md)。
