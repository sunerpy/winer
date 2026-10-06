# Changelog

## [0.0.3](https://github.com/sunerpy/winer/compare/v0.0.2...v0.0.3) (2026-10-06)


### Bug Fixes

* ask for admin rights to link the loader, and name callout seats ([#4](https://github.com/sunerpy/winer/issues/4)) ([d8daafd](https://github.com/sunerpy/winer/commit/d8daafde4cb64b2960dd7b4b9afa5ad829415192))

## [0.0.2](https://github.com/sunerpy/winer/compare/v0.0.1...v0.0.2) (2026-10-05)


### Features

* set up the in-client features by itself and sign the callout ([#2](https://github.com/sunerpy/winer/issues/2)) ([f38adfa](https://github.com/sunerpy/winer/commit/f38adfa9b19648be3be7a3df097578f498f2dfaa))

## 0.0.1 (2026-10-05)

首个公开版本。

### Features

- 对局分析：选人与对局中逐行列出每位玩家的段位、近期战绩和最近对局，红蓝方、开黑标记，点玩家进入战绩。
- 评级：默认峡谷五档，另有峡谷八档（S+ 到 F）、马系宇宙、赛马与峡谷食物链和自定义档位；毒舌称号与评语；评价依据写在设置和文档里。
- 战绩：分页浏览全部战绩，按模式筛选，MVP/SVP 与成就徽章（多杀、超神、一血、各项最多、逃兵），记分板含单局评分与评级。
- 自动化：自动接受、按分路选用与禁用、开局喊话（首行带红蓝方）、对局结束返回房间、大乱斗心愿英雄自动换，每项可按模式限定。
- 客户端增强：Pengu Loader 插件把队友战绩、评级和称号写进选人界面，备选席英雄点击即换。
- 安装：PowerShell 一行命令或 NSIS 安装包，带 SHA256SUMS 和构建证明；应用内一键更新。
