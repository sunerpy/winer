# winer 选人房间自动查询和战绩喊话实施说明

> **日期** 2026-11-22 · **实现参考基线** `930946c` · **受众** winer 实现与测试人员

进入选人房间后，winer 自动读取当前己方队员的近期战绩。开启自动喊话时，每局按楼层发送一次战报。可读战绩使用现有评分和模板，数据不可见、读失败或仍在读取的队友保留楼层并显示对应状态。

## 目录

- [触发范围](#触发范围)
- [当前行为与修改位置](#当前行为与修改位置)
- [查询与发送时序](#查询与发送时序)
- [楼层与文案规则](#楼层与文案规则)
- [验收用例](#验收用例)

## 触发范围

「进房」指匹配成功后进入英雄选择阶段（`Phase::ChampSelect`）。以该局 `ChampSelectSession.myTeam` 为己方名单来源。普通组队大厅的 `on_lobby` 已经会为房内可查成员预取战绩。进入选人后继续复用这些记录，并查询刚加入己方名单的队友。自动喊话使用当前选人的 `championSelect` 聊天室。 [组队大厅入口][lobby] · [选人阶段入口][phase] · [聊天室定位][chat]

战报使用当前登录账号在本局有权读取的战绩。标记为 `HIDDEN` 或缺少可查身份的座位沿用现有可见性判断。缓存复用也以当前座位可读为前提。已知对当前账号不可读的战绩保留为「战绩不可见」。本基线没有可用的国服匿名座位授权统计入口。全队都有真实数字需要全队战绩均可读。 [选人身份判断][visibility] · [玩家隐私标记][privacy]

## 当前行为与修改位置

| 位置                                                 | 基线行为                                                                                  | 实现要求                                                                                                                                                                |
| ---------------------------------------------------- | ----------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `service.rs` 的 `on_champ_select` 和 `ensure_player` | 按可查身份加载战绩，用 `Focus` 判断记录是否适用于当前模式。每次选人会话更新都会重新检查。 | 复用现有加载入口和缓存去重。`champ_select_puuids` 服务选人视图，自动喊话在生成文案时从当前己方名单选取成员。 [选人事件][entry] · [加载入口][ensure] · [记录缓存][cache] |
| `live.rs` 的 `champ_select_view`                     | 按 `cellId` 排列客户端返回的座位。隐藏位生成 `PlayerStats::Hidden`。                      | 以该顺序确定楼层。已明确不可读的战绩在评分前按隐藏处理。 [座位视图][seats]                                                                                              |
| `callout.rs` 的 `ready`、`lines` 和 `rated_lines`    | 有人仍在读取时继续等。生成文案时只保留有评分的座位。`rated_lines` 也用于游戏内队友喊话。  | 在选人喊话入口为每个参与输出的楼层生成数据行或状态行。游戏内喊话继续使用其原有入口。 [选人就绪与文案][lines] · [共用评分文案][rated] · [游戏内队友喊话][ally]           |
| `service.rs` 的 `render`、`call_out`                 | 先取文案并占用本局发送标记，再等待配置延迟和聊天室。                                      | 把等待任务和实际发送分开。发送前核对会话并以最新名单及读取结果生成文案。 [渲染与自动发送][render] · [等待与发送][send]                                                  |

## 查询与发送时序

1. **首份选人会话启动取数。** 复用 `on_phase` 对已开始选人会话的读取和选人资源事件。记录本次 `champ_select_opened`，后续名单和换楼事件都更新当前会话，队友查询走 `ensure_player`。优先沿用仍在有效期且满足当前 `Focus` 的结果。普通组队大厅的名单可以用于预取，选人名单决定最终楼层和查询范围。 [阶段进入与补读][phase] · [会话记录][entry] · [缓存判断][cache]

   取数复用 `record_games`。它优先读取所在大区的战绩服务，在服务繁忙时尝试逐场读取，并可回退至客户端的战绩列表。自动喊话的等待上限只控制出稿时机，战绩加载仍按原有请求和缓存管理完成。 [战绩加载][record-games]

2. **每局只安排一次自动发送等待。** 从首份当前会话起固定预留 10 秒收集队友名单和战绩。该值是本方案的取舍，因为现有会话和队列模型尚无明确的名单完备标志。即使首批队友已读完，也用该窗口接收后续名单。现有「延迟发送」是最早发送约束，开始尝试发送的时间取收集窗口与配置延迟两者中的较晚值。默认设置下最早约 10 秒，客户端查询在进房时就启动。用独立定时唤醒到期检查，保证没有新事件时也会结束等待。 [选人会话字段][session] · [队列元数据][queue] · [延迟设置][settings]

3. **聊天室就绪时生成最终文案。** 到点后读取当前状态，确认客户端连接、登录账号、选人阶段以及 `gameId` 和 `champ_select_opened` 仍对应等待任务，自动发送仍打开且适用本模式。沿用现有的聊天室探测次数，同时把等待聊天室限制在 10 秒总预算。获取本局聊天室后再次核对会话与设置，并从最近一份己方名单和已到达的战绩重新生成文案。聊天室始终缺失或归属不明时，记录本次发送失败。参与输出的座位里至少有一人具备可展示的评分时才进入发送。 [当前连接校验][account] · [自动发送范围][render] · [聊天等待][send]

4. **同局发送去重，换局取消。** 等待与实际发送占用不同的状态。计时、名单更新、请求完成同时发生时，只允许一个任务进入发送。退出选人、换局、切账号或关闭自动喊话时取消本次等待。按行发送时沿用现有消息间隔，每次提交前复核当前会话和本次取得的聊天室。发送过程中出现失败时记录结果并结束本局自动尝试，避免重复提交已经发送的行。晚到的战绩只刷新本局视图。 [选人重置][reset] · [当前投递方式][delivery]

## 楼层与文案规则

按本局 `ChampSelectView.my_team` 的顺序编号，从 1 开始。先编号再应用「包含自己」，关闭时仅省略自己的那一行，其余楼层沿用原编号。已有座位的身份字段因隐私而为空时，仍使用其原楼层。楼层和人数取自本局选人视图，标签使用 `seat_label` 的 `1L` 或 `P1` 格式。 [座位顺序][seats] · [楼层格式][seat-label] · [现有模板][settings]

| 发送时的座位状态             | 文案                                                       |
| ---------------------------- | ---------------------------------------------------------- |
| `Ready` 且有可展示评分       | 用现有模板和档位输出，沿用实际计入的场次、胜率、KDA 与战力 |
| `Hidden` 或已明确战绩不可见  | `3L｜战绩不可见`                                           |
| `Ready` 但没有可展示评分     | `3L｜无可用战绩`                                           |
| `Failed`                     | `3L｜战绩读取失败`                                         |
| 取数窗口结束后仍为 `Loading` | `3L｜战绩读取未完成`                                       |

状态行提供中英文固定文案，仅含楼层和状态。`Ready` 无评分包含空列表或有效样本不足等情况，统一使用「无可用战绩」。对其他玩家，已明确私密的记录在队内评分前按隐藏处理。现有加载逻辑把部分读取错误归为空列表，这类结果沿用「无可用战绩」。正常战绩沿用 `recent_form` 的模式和样本规则，最多取最近 50 场，按实际有效场次计算。 [战绩状态][stats-view] · [空结果处理][empty] · [评分入口][rating] · [近期战绩口径][form]

选人实时预览和手动发送复用楼层及状态文案。取数窗口内的 `Loading` 在预览或手动发送时显示「战绩读取中」。状态行和数据行共用现有发送对象与投递设置。 [手动发送入口][manual] · [消息投递][delivery]

### 喊话示例

```text
winer 战绩鉴定
1L｜千里马｜胜率 58%｜KDA 4.2｜战力 7.4
2L｜中等马｜胜率 54%｜KDA 3.5｜战力 6.3
3L｜战绩不可见
4L｜战绩读取未完成
5L｜下等马｜胜率 47%｜KDA 2.8｜战力 5.4
```

## 验收用例

| 场景                                       | 应验证的结果                                                                                                                                             |
| ------------------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 五人可查且包含自己                         | 进房自动取数并发送一份战报，从 `1L` 到 `5L` 均有真实统计。                                                                                               |
| 普通房间已有两名车队成员，选人新增三名     | 普通大厅完成预取，自动发送由选人会话触发。同模式的有效缓存直接复用，其余成员自动查询。模拟第一份选人名单只含两人，后续列表在收集窗口内到达，仍输出五行。 |
| `3L` 匿名或已明确战绩不可见                | `3L` 显示「战绩不可见」，其余人原楼层保持，`3L` 无评分且不参与档位计算。客户端标记为 `HIDDEN` 时，该座位发起的战绩请求数为 0。                           |
| 队友有早先的可读缓存，本局标记 `HIDDEN`    | 使用本局的可见性判断，文案只输出对应楼层的「战绩不可见」。                                                                                               |
| 取数和聊天室等待的时间边界                 | 进房立即取数。默认 10 秒内持续接收名单和战绩，到期后聊天室最多再等 10 秒。聊天室超时后记录发送失败。                                                     |
| 一人失败，一人到期仍在读取                 | 分别输出失败与读取未完成，有数据的楼层正常输出。即使没有后续事件，定时器也能触发。晚到数据刷新界面，本局只自动发送一次。                                 |
| 所有人无可展示评分                         | 不发送只有状态行的自动喊话。                                                                                                                             |
| 关闭「包含自己」                           | 省略自己所在楼层，其余行仍按原编号。                                                                                                                     |
| 聊天室晚到，期间换楼或有战绩返回           | 就绪后的文案反映最新楼层和战绩，并且只发送到本次选人聊天室。                                                                                             |
| 重复事件、退出选人、换局、切账号或关闭自动 | 重复事件只触发一次发送。旧任务不会向后继会话发消息。逐行发送失败时不会重发已经提交的整段。                                                               |
| 模式范围、语言、模板和发送对象             | 选人自动发送遵守现有开关和模式范围。中英文状态文案、可评分玩家自定义模板及两种发送对象正常工作。                                                         |

自动测试使用模拟选人事件、可控时钟和聊天替身。先跑 `winer-core` 的选人视图、喊话及 Service 测试，再做格式和 lint 检查。用户说明同步更新「进入选人后自动发送」的等待规则及缺数据文案。 [项目验证命令][agents] · [现有用户说明][user-doc]

```bash
cargo test -p winer-core live::
cargo test -p winer-core callout::
cargo test -p winer-core service::
cargo fmt --all --check
cargo clippy -p winer-core --all-targets -- -D warnings
pnpm format:check
```

[lobby]: https://github.com/sunerpy/winer/blob/930946cb26a5aba2fdac978ba0dda9170ec3b5f9/crates/core/src/service.rs#L2808
[phase]: https://github.com/sunerpy/winer/blob/930946cb26a5aba2fdac978ba0dda9170ec3b5f9/crates/core/src/service.rs#L1314-L1375
[chat]: https://github.com/sunerpy/winer/blob/930946cb26a5aba2fdac978ba0dda9170ec3b5f9/crates/core/src/service.rs#L3032-L3039
[visibility]: https://github.com/sunerpy/winer/blob/930946cb26a5aba2fdac978ba0dda9170ec3b5f9/crates/core/src/live.rs#L35-L58
[privacy]: https://github.com/sunerpy/winer/blob/930946cb26a5aba2fdac978ba0dda9170ec3b5f9/crates/core/src/analysis.rs#L53-L63
[entry]: https://github.com/sunerpy/winer/blob/930946cb26a5aba2fdac978ba0dda9170ec3b5f9/crates/core/src/service.rs#L1471-L1514
[ensure]: https://github.com/sunerpy/winer/blob/930946cb26a5aba2fdac978ba0dda9170ec3b5f9/crates/core/src/service.rs#L1860-L1896
[cache]: https://github.com/sunerpy/winer/blob/930946cb26a5aba2fdac978ba0dda9170ec3b5f9/crates/core/src/service.rs#L373-L447
[record-games]: https://github.com/sunerpy/winer/blob/930946cb26a5aba2fdac978ba0dda9170ec3b5f9/crates/core/src/service.rs#L3529-L3599
[seats]: https://github.com/sunerpy/winer/blob/930946cb26a5aba2fdac978ba0dda9170ec3b5f9/crates/core/src/live.rs#L110-L157
[lines]: https://github.com/sunerpy/winer/blob/930946cb26a5aba2fdac978ba0dda9170ec3b5f9/crates/core/src/callout.rs#L436-L480
[rated]: https://github.com/sunerpy/winer/blob/930946cb26a5aba2fdac978ba0dda9170ec3b5f9/crates/core/src/callout.rs#L510-L541
[ally]: https://github.com/sunerpy/winer/blob/930946cb26a5aba2fdac978ba0dda9170ec3b5f9/crates/core/src/callout.rs#L1044-L1066
[render]: https://github.com/sunerpy/winer/blob/930946cb26a5aba2fdac978ba0dda9170ec3b5f9/crates/core/src/service.rs#L1580-L1619
[send]: https://github.com/sunerpy/winer/blob/930946cb26a5aba2fdac978ba0dda9170ec3b5f9/crates/core/src/service.rs#L1646-L1707
[session]: https://github.com/sunerpy/winer/blob/930946cb26a5aba2fdac978ba0dda9170ec3b5f9/crates/core/src/model.rs#L105-L124
[queue]: https://github.com/sunerpy/winer/blob/930946cb26a5aba2fdac978ba0dda9170ec3b5f9/crates/core/src/view.rs#L709-L719
[settings]: https://github.com/sunerpy/winer/blob/930946cb26a5aba2fdac978ba0dda9170ec3b5f9/crates/core/src/settings.rs#L449-L511
[account]: https://github.com/sunerpy/winer/blob/930946cb26a5aba2fdac978ba0dda9170ec3b5f9/crates/core/src/service.rs#L2990-L2996
[reset]: https://github.com/sunerpy/winer/blob/930946cb26a5aba2fdac978ba0dda9170ec3b5f9/crates/core/src/service.rs#L318-L329
[delivery]: https://github.com/sunerpy/winer/blob/930946cb26a5aba2fdac978ba0dda9170ec3b5f9/crates/core/src/service.rs#L3041-L3077
[seat-label]: https://github.com/sunerpy/winer/blob/930946cb26a5aba2fdac978ba0dda9170ec3b5f9/crates/core/src/callout.rs#L126-L131
[stats-view]: https://github.com/sunerpy/winer/blob/930946cb26a5aba2fdac978ba0dda9170ec3b5f9/crates/core/src/view.rs#L361-L382
[empty]: https://github.com/sunerpy/winer/blob/930946cb26a5aba2fdac978ba0dda9170ec3b5f9/crates/core/src/service.rs#L3577-L3599
[rating]: https://github.com/sunerpy/winer/blob/930946cb26a5aba2fdac978ba0dda9170ec3b5f9/crates/core/src/live.rs#L61-L81
[form]: https://github.com/sunerpy/winer/blob/930946cb26a5aba2fdac978ba0dda9170ec3b5f9/crates/core/src/analysis.rs#L151-L181
[manual]: https://github.com/sunerpy/winer/blob/930946cb26a5aba2fdac978ba0dda9170ec3b5f9/crates/core/src/service.rs#L774-L795
[agents]: https://github.com/sunerpy/winer/blob/930946cb26a5aba2fdac978ba0dda9170ec3b5f9/AGENTS.md
[user-doc]: https://github.com/sunerpy/winer/blob/930946cb26a5aba2fdac978ba0dda9170ec3b5f9/docs/site/guide/automation.md#L57-L72
