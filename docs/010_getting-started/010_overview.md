# 如何使用 Valthrun
要使用 Valthrun，你需要遵循几个步骤来设置所需的组件并运行 Valthrun overlay。
以下是使用 Valthrun 的指南：

## 所需文件
在使用 Valthrun 之前，您必须获得两个基本组件：

1. **内核驱动 (`vulthrun-driver.sys`)**  
内核驱动程序是 Valthrun 的关键部分。
它支持对《反恐精英 2》进程进行未检测到的任意读取操作，并防止其他软件（如 VAC）检测到这些操作。
你可以从 GitHub 发布页面取得最新版本的核心驱动程序。

2. **控制器 (`controller.exe`)**  
以 `controller.exe` 提供的 Valthrun 叠加层是 Valthrun 的用户界面。 
它允许你与 Valthrun 软件交互并控制它。

您可以从以下页面下载最新版本的内核驱动程序和 CS2 叠加程序:
 - [GitHub release page](https://github.com/Valthrun/Valthrun/releases).

## 启动 Valthrun
获得上述必要文件后，请按照以下步骤以启动 Valthrun:

1. **加载内核驱动 (`vulthrun-driver.sys`)**  
加载内核驱动程序有多种方法。
有关如何加载内核驱动程序的详细说明，请参阅[此处](010_getting-started/020_driver.md)的文档。

1. **确保 CS2 正在运行**  
启动 Valthrun 叠加之前，请确保《反恐精英 2》（CS2）正在运行。
如果 CS2 尚未运行，请启动游戏，因为如果 CS2 未运行，控制器将无法运行。

1. **启动控制器 (`controller.exe`)**  
一旦成功加载内核驱动程序，且 CS2 启动并运行后，
你可以运行 `controller.exe`，启动 Valthrun 控制器。
如果一切设置正确，你应该会看到一个控制台窗口。

Here's an example of what the overlay's interface might look like:
![Screenshot of Success](../../_media/screenshot_controller_success.png)

完成这些步骤后，您现在就可以使用 Valthrun 并利用其在《反恐精英 2》中的游戏增强功能了。 
有关其功能和使用的更多详细信息，请务必查阅项目文档和支持资源。