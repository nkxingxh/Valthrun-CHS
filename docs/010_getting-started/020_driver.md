# 加载 Valthrun 驱动程序 (`valthrun-driver.sys`)  
Loading or mapping a driver can be a complex and challenging task.  
Typically, drivers are signed with an "Extended Validation Code Sign Certificate."  
However, acquiring these certificates can be difficult, and they can be easily blocked by third-party software like VAC (Valve Anti-Cheat).  
As a result, loading the Valthrun driver requires unconventional methods.

加载 Valthrun 驱动程序加载或映射驱动程序可能是一项复杂且具有挑战性的任务。 
通常，驱动程序会使用「扩展验证代码签名证书」进行数字签名。 
然而，获取这些证书可能很困难，而且它们很容易被 VAC（Valve Anti-Cheat）等第三方软件阻止。 
因此，加载 Valthrun 驱动程序需要采用**非常规方法**。

## 支持的 Windows 版本 
Valthrun 旨在兼容各种 Windows 版本。 
由于函数和结构偏移量是动态解析的，因此预计将支持所有最新的 Windows 版本。 
最新的 Windows 10 和 Windows 11 版本（如 22H2）已成功通过测试。 
用户反馈还表明，最早的 Windows 版本（如 20H2）也能正常工作。 
如果遇到任何问题，请提交问题报告，其中包括 Windows 版本和遇到的错误描述。

## 加载方法概述
以下是可用于加载 Valthrun 驱动程序的各种方法的简要概述：

| 方法 | 操作难度 | 成功率 |
| --- | --- | --- |
| [Manual Mapping via KD-Mapper](010_getting-started/010_mapping-method/010_kdmapper.md) | Medium | High |
| [Manual Mapping via KDU](010_getting-started/010_mapping-method/020_kdu.md) | Medium | Medium |
| [Using Windows Test-Signing](010_getting-started/010_mapping-method/030_test-signing.md) | Low | Very High |
| [Manual Mapping via Other Mappers](010_getting-started/010_mapping-method/040_other-mappers.md) | Unknown | Unknown |

每种方法都有各自的复杂程度和成功率。 
选择使用哪种方法取决于您的具体需求和系统的兼容性。 
有关每种映射方法的详细信息，请参阅链接的资源。

## Mapping with Community-Made Scripts
In addition to the manual methods of mapping the Valthrun driver, there are community-made scripts available that simplify the process.  
These scripts are often developed and shared by members of the Valthrun community.  
While these scripts can be convenient, it's important to note that they are not officially provided by Valthrun, and their success rate may vary. 
  
- [Valthrunners' Script](010_getting-started/010_mapping-method/110_community_script_valthrunner.md)
  
TODO: Add the other scripts