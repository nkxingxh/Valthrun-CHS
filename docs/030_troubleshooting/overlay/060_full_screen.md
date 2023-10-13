# Valthrun 和 CS2 全屏模式
Valthrun 叠加功能适用于 CS2 的 "窗口 "和 "全屏窗口 "模式。 
第三种模式“全屏”**不支持**，而且很可能永远不会支持。 
  
据我所知，目前还没有任何外部应用程序可以支持全屏模式。 
道理很简单： 外部应用程序必须创建第二个窗口，并将其置于目标应用程序之上。 
这就是“叠加层”的工作原理，而不会向目标进程本身注入任何东西 (例如，像 Discord 的覆盖) 。 
使用全屏模式时，CS2 没有窗口，而是直接使用 GPU 功能来呈现渲染的画面，因此不会覆盖任何其他窗口。

## Can I still play with my 4:3 resolution?
Personally I never tested this solution, but there is a script available to make Valthrun and CS2 work with
a 4:3 monitor resolution. The script can be found here:  
https://discord.com/channels/1135362291311849693/1157745655108874241/1157751376856764453