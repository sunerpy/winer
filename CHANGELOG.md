# Changelog

## [0.0.13](https://github.com/sunerpy/winer/compare/v0.0.12...v0.0.13) (2026-10-07)


### Bug Fixes

* keep callouts, bench swaps and history views reliable ([#27](https://github.com/sunerpy/winer/issues/27)) ([5ace97e](https://github.com/sunerpy/winer/commit/5ace97e321e42e72b6b9725616fc92781f5afa4e))

## [0.0.12](https://github.com/sunerpy/winer/compare/v0.0.11...v0.0.12) (2026-10-07)


### Bug Fixes

* accept matches reliably and keep callouts in seat order ([#25](https://github.com/sunerpy/winer/issues/25)) ([c61a003](https://github.com/sunerpy/winer/commit/c61a00361861a91cbc4170c253dc012864e09243))

## [0.0.11](https://github.com/sunerpy/winer/compare/v0.0.10...v0.0.11) (2026-10-07)


### Features

* diagnostics, player notes, premade hints and pick suggestions ([#20](https://github.com/sunerpy/winer/issues/20)) ([7d4e90f](https://github.com/sunerpy/winer/commit/7d4e90f66a81cb0999fffec82d4c4202c9d1d8e9))

## [0.0.10](https://github.com/sunerpy/winer/compare/v0.0.9...v0.0.10) (2026-10-07)


### Bug Fixes

* keep passive updater windows fully invisible ([#22](https://github.com/sunerpy/winer/issues/22)) ([cae5e68](https://github.com/sunerpy/winer/commit/cae5e68428d32555eb7f9547c898228d6493ea41))

## [0.0.9](https://github.com/sunerpy/winer/compare/v0.0.8...v0.0.9) (2026-10-07)


### Bug Fixes

* hide the updater installer before its first frame ([#19](https://github.com/sunerpy/winer/issues/19)) ([7cae57a](https://github.com/sunerpy/winer/commit/7cae57a2a85c3124b52fee32cfc6e7e77ab77af1))

## [0.0.8](https://github.com/sunerpy/winer/compare/v0.0.7...v0.0.8) (2026-10-07)


### Bug Fixes

* keep callouts readable and Windows updates silent ([#17](https://github.com/sunerpy/winer/issues/17)) ([584b0a9](https://github.com/sunerpy/winer/commit/584b0a9099039029edf6f6887b5f03b7a49f1b6c))

## [0.0.7](https://github.com/sunerpy/winer/compare/v0.0.6...v0.0.7) (2026-10-06)


### Bug Fixes

* read each rotating mode's own games for its form ([#15](https://github.com/sunerpy/winer/issues/15)) ([866ffe7](https://github.com/sunerpy/winer/commit/866ffe78a8077acb8f01d5e375647b0ad408cd46))

## [0.0.6](https://github.com/sunerpy/winer/compare/v0.0.5...v0.0.6) (2026-10-06)


### Bug Fixes

* wait for an idle client however long, read the mode's own games only ([#13](https://github.com/sunerpy/winer/issues/13)) ([9baa6e7](https://github.com/sunerpy/winer/commit/9baa6e7325f90d9b46f6cae0f85e2e1ee57bdbe6))

## [0.0.5](https://github.com/sunerpy/winer/compare/v0.0.4...v0.0.5) (2026-10-06)


### Features

* game-score strength, client restart once settled, safer callout ([#10](https://github.com/sunerpy/winer/issues/10)) ([16c4019](https://github.com/sunerpy/winer/commit/16c4019213d866988a293eceef3d0ed859062cb7))

## [0.0.4](https://github.com/sunerpy/winer/compare/v0.0.3...v0.0.4) (2026-10-06)


### Features

* builds, in-client history, callout shortcut and storage limits ([#6](https://github.com/sunerpy/winer/issues/6)) ([224b1a8](https://github.com/sunerpy/winer/commit/224b1a847449db4c9c94c9e87ef31cfe208ae686))

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
