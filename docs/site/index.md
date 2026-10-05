---
layout: home
title: winer：英雄联盟客户端助手
titleTemplate: false
description: 选人时看清每位队友的近期战绩和档位，翻完任何人的战绩，把接受对局、选用英雄和开局喊话交给它。只做 Windows 版，在国服客户端上实测。

hero:
  name: winer
  text: 英雄联盟客户端助手
  tagline: 选人时看清每位队友的近期战绩和档位，按名称翻完任何人的战绩，把接受对局、选用英雄和开局喊话交给它。只做 Windows 版，在国服客户端上实测。
  actions:
    - theme: brand
      text: 安装
      link: /guide/install
    - theme: alt
      text: 快速开始
      link: /guide/quick-start
    - theme: alt
      text: GitHub
      link: https://github.com/sunerpy/winer

home:
  facts:
    - term: 运行环境
      text: 64 位 Windows 10 或 11，腾讯客户端（国服）实测；安装到当前用户，不需要管理员权限。
    - term: 工作方式
      text: 只使用客户端在本机开放的接口，不读取游戏内存，不修改游戏文件。

  visual:
    desktop:
      light: /screens/live-light.webp
      dark: /screens/live-dark.webp
      width: 1440
      height: 900
      alt: 选人阶段的对局分析：每位队友一行，左边是档位、称号、段位、胜率和 KDA，右边是最近的每一局。

  index:
    title: winer 能做什么
    intro: 全部功能一览。每一项自动化都默认关闭，并且可以限定在哪些模式里生效。
    groups:
      - name: 对局
        items:
          - title: 对局分析
            body: 选人和游戏中逐行列出双方玩家的段位、近期胜率、KDA 和最近每一局，标出红蓝方和开黑的队友。
            status: available
            link: /guide/live
          - title: 评级与毒舌称号
            body: 默认把一队五人排成峡谷五档，每档配一句评语；也可以按固定分段评 S+ 到 F，或者用马系、自定义档位。
            status: available
            link: /rating
          - title: 战力喊话
            body: 按档位排好的聊天内容，一键发到队伍或只给自己看，也可以每局自动发送；第一行带上红蓝方。
            status: available
            link: /guide/live#战力喊话
          - title: 大乱斗备选席
            body: 点一下立刻换成备选席上的英雄，不等冷却；设好心愿英雄后自动换。
            status: available
            link: /guide/live#大乱斗的备选席
      - name: 战绩
        items:
          - title: 完整战绩
            body: 按名称查任何人，分页翻完整个历史，按排位、匹配、大乱斗筛选。
            status: available
            link: /guide/history
          - title: MVP 与成就徽章
            body: 每一局标出 MVP、SVP，以及多杀、超神、一血、各项最多和逃兵。
            status: available
            link: /guide/history#每一局
          - title: 记分板
            body: 单局评分与 S+ 到 F 的评级、伤害占比、承伤、参团率、经济和装备，全场最高的数值单独标出。
            status: available
            link: /guide/history#记分板
      - name: 自动化
        items:
          - title: 自动接受对局
            body: 匹配成功后自动接受，可以设置等待几秒。
            status: available
            link: /guide/automation#匹配成功
          - title: 自动选择与禁用
            body: 按分路的英雄顺序自动选择和禁用，跳过已被选走、禁用或队友亮出的英雄。
            status: available
            link: /guide/automation#英雄选择与禁用
          - title: 自动返回房间
            body: 对局结束后自动回到房间，继续排下一局。
            status: available
            link: /guide/automation#对局结束
      - name: 客户端与工具
        items:
          - title: 客户端插件
            body: 经由 Pengu Loader 把队友战绩、档位和称号写进选人界面，备选席英雄点击即换。
            status: available
            link: /guide/client
          - title: 在线状态与签名
            body: 切换在线、离开和隐身，修改个性签名，重启卡住的客户端界面。
            status: available
            link: /guide/settings#工具
          - title: 五套主题
            body: 明亮、暗黑、石墨、海克斯和跟随系统，八种强调色，托盘常驻与开机自启。
            status: available
            link: /guide/settings#设置

  steps:
    title: 从安装到第一次喊话
    items:
      - title: 安装
        command: irm https://github.com/sunerpy/winer/releases/latest/download/install.ps1 | iex
        body: 在 PowerShell 里运行，安装包按 SHA256SUMS 校验通过后才会安装；也可以下载安装包双击运行。
      - title: 连接客户端
        body: 登录客户端后打开 winer。国服客户端以管理员身份运行时，点「以管理员身份重启」。
      - title: 进入英雄选择
        body: 对局页列出每位队友的战绩和档位，点「发送到队伍」把战力喊话发给队友。

  shots:
    history:
      light: /screens/history-light.webp
      dark: /screens/history-dark.webp
      width: 1440
      height: 900
      alt: 战绩页：每一局的胜负、时长、成就徽章、KDA 和装备，头像上标着 MVP 或 SVP，展开的一局显示完整记分板。
    rating:
      light: /screens/rating-light.webp
      dark: /screens/rating-dark.webp
      width: 1440
      height: 900
      alt: 设置里的评级：七种评级方案、毒舌称号开关和评价依据。

  privacy:
    title: 数据去向
    intro: winer 没有账号，不收集使用数据，也没有自己的服务器。下面是它会连接的全部地址。
    sendsLabel: 发送
    modes:
      - name: 本机的客户端
        sends: 客户端自己生成的本地凭证
        detail: 读取召唤师、对局和战绩，执行你打开的自动化。
      - name: 所在大区的战绩服务器
        sends: 客户端登录时拿到的访问令牌
        detail: 分页读取战绩；连不上时退回客户端的最近 20 场。
      - name: ARAM.GG 与 GitHub
        sends: 普通的网页请求
        detail: 获取海克斯符文的效果说明（可以关闭），检查和下载更新。
---

<HomeIndex />

<HomeSteps />

<SplitBlock proof="screen" shot="history">

## 任何人的战绩，一页一页翻完

腾讯客户端自己的接口只给最近 20 场。winer 向所在大区的战绩服务器分页查询，所以能翻完整个历史，并按排位、匹配和
大乱斗筛选。

每一局都标出 MVP、SVP 和本局成就：双杀到五杀、超神、一血、各项最多，以及被系统判定挂机的逃兵。展开就是带单局
评分和评级的完整记分板。

[战绩与成就](/guide/history) · [评级说明](/rating)

</SplitBlock>

<SplitBlock proof="screen" shot="rating" flip>

## 评级有梗，依据写得清清楚楚

默认的峡谷五档把一队五人从峡谷通天代排到纯正牛马，每档配一句评语，三连胜的队友会被叫作「版本答案」。不想和队友
比，就换成按固定分段评 S+ 到 F 的峡谷八档。

近期战力、单局评分、每个称号的条件和每条成就的规则，全部写在设置和文档里，不靠猜。

[评级说明](/rating) · [对局分析与喊话](/guide/live)

</SplitBlock>

<HomePrivacy />

## 安装

::: code-group

```powershell [一行命令]
irm https://github.com/sunerpy/winer/releases/latest/download/install.ps1 | iex
```

```powershell [指定版本]
$env:WINER_VERSION = "0.0.1"
irm https://github.com/sunerpy/winer/releases/download/v0.0.1/install.ps1 | iex
```

:::

脚本下载安装包后先按同一版本的 `SHA256SUMS` 校验，通过后才静默安装。也可以在
[GitHub Releases](https://github.com/sunerpy/winer/releases) 下载 `winer_<版本>_x64-setup.exe` 双击安装。
[安装指南](/guide/install)介绍了校验、更新和卸载。

## 反馈

报告问题、提出建议：[GitHub Issues](https://github.com/sunerpy/winer/issues)。
