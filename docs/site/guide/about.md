# 致谢与免责声明

## 致谢

- [Pengu Loader](https://github.com/PenguLoader/PenguLoader)（MIT 许可）：把插件载入客户端界面。winer 内置了它的
  `core.dll`，许可原文随源码一起发布在
  [THIRD_PARTY_NOTICES.md](https://github.com/sunerpy/winer/blob/main/THIRD_PARTY_NOTICES.md)，也可以在
  **设置 › 关于** 里查看。
- [Tauri](https://tauri.app)、Rust 与 React 的开源生态：winer 的窗口和内核建立在它们之上。
- [ARAM.GG](https://aramgg.com)：海克斯大乱斗强化符文的效果说明，以及腾讯数据取不到时的海克斯大乱斗强化符文统计。
- 腾讯掌上英雄联盟的公开对局统计：召唤师峡谷和海克斯大乱斗的出装、符文、召唤师技能、技能加点、对位与强化符文推荐，
  数据来自国服对局。
- [OP.GG](https://www.op.gg)：极地大乱斗与斗魂竞技场的出装和强化符文统计，以及可选的召唤师峡谷数据来源。
- [WeGame](https://www.wegame.com.cn)：对局评分的权重拿 WeGame 公开的 MVP、SVP 结果校准过，没有使用它的算法。

## 免责声明

- winer 是非官方的第三方工具，与 Riot Games、腾讯及 WeGame 没有任何关联，未获它们认可、赞助或支持。英雄联盟
  （League of Legends）及相关名称、标志和图片是 Riot Games, Inc. 的商标或财产。
- winer 只使用客户端在本机开放的接口，不读取或修改游戏进程的内存，不修改游戏文件，也不在对局中替你操作。唯一的例外
  是默认关闭的 **游戏内发送**：打开后，你在游戏中按下发送喊话的快捷键时，winer 向游戏窗口模拟键盘输入，把喊话打进
  聊天，这属于第三方输入。客户端增强经由 Pengu Loader 在客户端界面里运行插件，会改变客户端界面的显示。
- 第三方工具都可能违反游戏的服务条款，使用 winer 可能使账号受到限制，风险由你自行承担。
- 评分、评级、称号和喊话只是基于战绩数据的娱乐性参考，不代表任何人的真实水平。请勿用它们辱骂或骚扰其他玩家。
- 段位伪装只改变好友在好友列表和资料卡里看到的段位，你的真实段位、匹配和客户端里你自己的资料都不会变。好友看到的
  并不是你的真实段位，请勿用它误导他人。
- 配装、符文、召唤师技能、技能加点和强化符文推荐来自上面列出的第三方公开统计，winer 不保证它们准确或及时。这些接口
  并不是为第三方工具提供的，随时可能变化或停用，那时相应的面板会显示取不到数据。
- 自动配置符文与召唤师技能、自动写入装备方案只在你打开对应规则后才会修改客户端里的内容，默认都是关闭的。
- winer 按 MIT 许可「按原样」提供，不附带任何担保，作者不对使用造成的损失负责。

## 许可

winer 的源码以 [MIT 许可](https://github.com/sunerpy/winer/blob/main/LICENSE)发布。它所依赖的开源组件各有自己的
许可，见 [THIRD_PARTY_NOTICES.md](https://github.com/sunerpy/winer/blob/main/THIRD_PARTY_NOTICES.md)。
