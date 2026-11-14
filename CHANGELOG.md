# Changelog

## [0.0.21](https://github.com/sunerpy/winer/compare/v0.0.20...v0.0.21) (2026-11-14)


### 新功能

* 近期战力改为参考最近 50 场对局（原 20 场），越早的对局权重降得更慢，并按新抽样重新标定评分 ([#50](https://github.com/sunerpy/winer/issues/50)) ([aeddeab](https://github.com/sunerpy/winer/commit/aeddeab52c2ebebe1803a91706dbcc5f49e3bd94))

## [0.0.20](https://github.com/sunerpy/winer/compare/v0.0.19...v0.0.20) (2026-11-06)


### 新功能

* 在线语录可以查看和编辑：分档浏览、搜索，逐句修改、删除和添加；改过的语录标「已修改」，移除前会先确认 ([#46](https://github.com/sunerpy/winer/issues/46)) ([711dea6](https://github.com/sunerpy/winer/commit/711dea6140ef5bb6ccb5e3100f461fcca6f31526))
* 选人快捷语独立成「快捷语」页（Ctrl+8），与反击页分开；反击的两个快捷键改名为「反击发到队伍」「反击发到全体」 ([#46](https://github.com/sunerpy/winer/issues/46)) ([711dea6](https://github.com/sunerpy/winer/commit/711dea6140ef5bb6ccb5e3100f461fcca6f31526))
* 选人时交换英雄可自动发消息：向队友发起交换时自动发出第一条快捷语，交换成功后自动道谢；两个开关默认关闭，感谢语可自定义 ([#46](https://github.com/sunerpy/winer/issues/46)) ([711dea6](https://github.com/sunerpy/winer/commit/711dea6140ef5bb6ccb5e3100f461fcca6f31526))

## [0.0.19](https://github.com/sunerpy/winer/compare/v0.0.18...v0.0.19) (2026-11-03)


### 新功能

* 「反击」页（新菜单，Ctrl+7）：按快捷键随机发一句你自己写的反击语句，选人时发到选人聊天，打开「在游戏里输入」后可输入游戏的队伍或全体聊天，两次发送至少间隔 3 秒；还没有可发的语句时页面会提示 ([#43](https://github.com/sunerpy/winer/issues/43)) ([7186cdb](https://github.com/sunerpy/winer/commit/7186cdbcf578ccbc3e134616d0808bef3985e886))
* 反击可以使用开源的祖安语录（在线语录）：下载前要确认，下载后要自己打开才会使用，文件按 SHA-256 校验，可随时移除 ([#43](https://github.com/sunerpy/winer/issues/43)) ([7186cdb](https://github.com/sunerpy/winer/commit/7186cdbcf578ccbc3e134616d0808bef3985e886))
* 设置 › 关于新增 QQ 交流群「winer交流群」（群号 1109319256），可一键加入或复制群号 ([#43](https://github.com/sunerpy/winer/issues/43)) ([7186cdb](https://github.com/sunerpy/winer/commit/7186cdbcf578ccbc3e134616d0808bef3985e886))
* 选人快捷语：英雄选择中点对局页队友旁的菜单，把填好楼层和英雄的一句话发到选人聊天，例如「5L大佬，小弟想玩下龙王，给您磕一个了」；也可以先在客户端对队友点「交换」，再按「发给交换的队友」快捷键发出 ([#43](https://github.com/sunerpy/winer/issues/43)) ([7186cdb](https://github.com/sunerpy/winer/commit/7186cdbcf578ccbc3e134616d0808bef3985e886))


### 问题修复

* 从 WeGame 启动的游戏收不到 winer 输入的文字：游戏以管理员身份运行时会丢弃权限较低程序的按键；现在输入前先检查权限，不够时在动态里说明，并在连接客户端时请求一次以管理员身份重启 ([#43](https://github.com/sunerpy/winer/issues/43)) ([7186cdb](https://github.com/sunerpy/winer/commit/7186cdbcf578ccbc3e134616d0808bef3985e886))
* 游戏中按快捷键没有反应：游戏在前台时 Windows 的系统快捷键传不到 winer，现在游戏进行中改由键盘钩子接收你设置的组合键，只接手这几个组合键，其他按键原样交给游戏 ([#43](https://github.com/sunerpy/winer/issues/43)) ([7186cdb](https://github.com/sunerpy/winer/commit/7186cdbcf578ccbc3e134616d0808bef3985e886))

## [0.0.18](https://github.com/sunerpy/winer/compare/v0.0.17...v0.0.18) (2026-11-01)


### Features

* claim waiting rewards in one click, and reset the icon and border ([#40](https://github.com/sunerpy/winer/issues/40)) ([4854fad](https://github.com/sunerpy/winer/commit/4854fad89b23b5f5f3fef0caa8ab67e6dca2daa8))
* list the rewards the client holds for you on the Tools page, oldest first, each with what it contains and when it was granted ([#40](https://github.com/sunerpy/winer/issues/40)) ([4854fad](https://github.com/sunerpy/winer/commit/4854fad89b23b5f5f3fef0caa8ab67e6dca2daa8))
* only claim a reward that offers a choice as you tick it; winer never picks for you ([#40](https://github.com/sunerpy/winer/issues/40)) ([4854fad](https://github.com/sunerpy/winer/commit/4854fad89b23b5f5f3fef0caa8ab67e6dca2daa8))
* optionally claim when winer connects and after each game, behind a switch that is off by default, and never while queueing, in champ select or in a game ([#40](https://github.com/sunerpy/winer/issues/40)) ([4854fad](https://github.com/sunerpy/winer/commit/4854fad89b23b5f5f3fef0caa8ab67e6dca2daa8))
* put back the default profile icon or the plain border in one click, each with an undo ([#40](https://github.com/sunerpy/winer/issues/40)) ([4854fad](https://github.com/sunerpy/winer/commit/4854fad89b23b5f5f3fef0caa8ab67e6dca2daa8))


### Bug Fixes

* keep winer's item sets within what the client takes, so the automatic build import no longer fails with HTTP 413 (Content length is too large): at most 20 of winer's own sets and under 60 KB in all, its oldest dropped first and never yours ([#40](https://github.com/sunerpy/winer/issues/40)) ([4854fad](https://github.com/sunerpy/winer/commit/4854fad89b23b5f5f3fef0caa8ab67e6dca2daa8))
* say the item-set list is full when the client still refuses it, instead of showing the HTTP error ([#40](https://github.com/sunerpy/winer/issues/40)) ([4854fad](https://github.com/sunerpy/winer/commit/4854fad89b23b5f5f3fef0caa8ab67e6dca2daa8))

## [0.0.17](https://github.com/sunerpy/winer/compare/v0.0.16...v0.0.17) (2026-10-25)


### Features

* send the champ-select callout as one message by default ([#37](https://github.com/sunerpy/winer/issues/37)) ([9fc1f21](https://github.com/sunerpy/winer/commit/9fc1f210fbe58c3b51a6549235171757353a04ce))

## [0.0.16](https://github.com/sunerpy/winer/compare/v0.0.15...v0.0.16) (2026-10-16)


### Bug Fixes

* read history game by game while the match-history server is busy ([#34](https://github.com/sunerpy/winer/issues/34)) ([42da955](https://github.com/sunerpy/winer/commit/42da9558a36fd4e3f07284bc1b78bde38dde32da))

## [0.0.15](https://github.com/sunerpy/winer/compare/v0.0.14...v0.0.15) (2026-10-11)


### Bug Fixes

* read history past a busy server and delay the champ-select callout ([#31](https://github.com/sunerpy/winer/issues/31)) ([98035e5](https://github.com/sunerpy/winer/commit/98035e5423ac6856bf56119e056c6b1cd746d70f))

## [0.0.14](https://github.com/sunerpy/winer/compare/v0.0.13...v0.0.14) (2026-10-07)


### Bug Fixes

* remove external implementation references ([#29](https://github.com/sunerpy/winer/issues/29)) ([7cb6d4f](https://github.com/sunerpy/winer/commit/7cb6d4f6f9aa53eaf23b2f517db459c608eb515b))

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
