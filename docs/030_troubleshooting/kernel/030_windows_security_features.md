# 禁用某些 Windows 安全功能
默认情况下，Windows 启用了各种旨在保护内核不被篡改的功能。 
我强烈建议保持启用，但为了让 Valthrun 正常工作，必须禁用一些功能。 
- 内核隔离
- MSFT 驱动程序阻止列表
- 基于虚拟化的安全
- 禁用 HyperVisor

## 禁用核心隔离
详情请访问  
https://support.microsoft.com/en-us/windows/a-driver-can-t-load-on-this-device-8eea34e5-ff4b-16ec-870d-61a4a43b3dd5
  
TODO: Why?

## MSFT 驱动程序阻止列表和微软漏洞驱动程序阻止列表
KDMapper 输出: `NTSTATUS (0xC0000428): Windows cannot verify the digital signature for this file`
https://www.thewindowsclub.com/microsoft-vulnerable-driver-blocklist-in-windows
https://community.amd.com/t5/drivers-software/amd-driver-problem/m-p/474646#M144661

注意：之后必须重新启动电脑。
TODO: Why?

## 基于虚拟化的安全
https://www.makeuseof.com/windows-11-disable-vbs/

TODO: Why?

## 禁用 HyperVisor
在 cmd 中以管理员身份运行:
```
bcdedit /set hypervisorlaunchtype off
```