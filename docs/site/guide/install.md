# 安装

需要 64 位 Windows 10 或 11。winer 依赖系统自带的 WebView2，Windows 10 和 11 一般已经装好；缺少时安装程序会
自动下载。安装只装到当前用户，不需要管理员权限。

## 一行命令

在 PowerShell 里运行：

```powershell
irm https://github.com/sunerpy/winer/releases/latest/download/install.ps1 | iex
```

脚本会下载最新版本的安装包，按同一个版本里的 `SHA256SUMS` 校验 SHA-256，校验通过后才静默安装；不通过就停下，
什么都不装。

固定一个版本时，先设置 `WINER_VERSION`，并使用那个版本自带的脚本：

```powershell
$env:WINER_VERSION = "0.0.1"
irm https://github.com/sunerpy/winer/releases/download/v0.0.1/install.ps1 | iex
```

## 安装包

在 [GitHub Releases](https://github.com/sunerpy/winer/releases) 下载 `winer_<版本>_x64-setup.exe`，双击运行。

安装包没有代码签名证书，所以 Windows 会显示「Windows 已保护你的电脑」：点 **更多信息**，再点 **仍要运行**。
部分杀毒软件可能误报，原因是 winer 会读取另一个程序（客户端）的本地接口。

## 校验下载的文件

每个版本都附带 `SHA256SUMS`，以及 GitHub 为每个文件生成的构建证明。手动下载时可以这样核对：

```powershell
Get-FileHash .\winer_0.0.1_x64-setup.exe -Algorithm SHA256
```

把结果和 `SHA256SUMS` 里同名文件的那一行对比。装有 GitHub CLI 时，还可以确认文件确实由 winer 的发布流程构建：

```bash
gh attestation verify winer_0.0.1_x64-setup.exe --repo sunerpy/winer \
  --signer-workflow sunerpy/winer/.github/workflows/release.yml
```

## 更新

在 **设置 › 关于** 里检查更新，有新版本时点 **立即更新**：下载完成后 winer 会自动安装并重启。每个更新包都有签名，
winer 只会安装签名能用内置公钥验证通过的版本。

## 卸载

在 Windows 的 **设置 › 应用 › 已安装的应用** 里卸载 winer。设置文件保存在
`%APPDATA%\app.winer.desktop\settings.json`，日志在 `%LOCALAPPDATA%\app.winer.desktop\logs`，卸载后不再需要
时可以手动删除。客户端插件请在卸载前从 **客户端增强** 页卸载，或者删除 Pengu Loader 的 `plugins\winer` 目录。
