@echo off
chcp 936
set "scriptVersion=1.0"
title 源什启动器 v%scriptVersion%
setlocal EnableDelayedExpansion
cls

:: BatchGotAdmin
:-------------------------------------
REM  --> Check for permissions
>nul 2>&1 "%SYSTEMROOT%\system32\cacls.exe" "%SYSTEMROOT%\system32\config\system"

REM --> If error flag set, we do not have admin.
if '%errorlevel%' NEQ '0' (
    color 6F
    echo.
    echo  正在请求管理员权限...
    goto UACPrompt
) else ( goto gotAdmin )

:UACPrompt
    echo Set UAC = CreateObject^("Shell.Application"^) > "%temp%\getadmin.vbs"
    set params = %*:"=""
    echo UAC.ShellExecute "cmd.exe", "/c %~s0 %params%", "", "runas", 1 >> "%temp%\getadmin.vbs"

    "%temp%\getadmin.vbs"
    del "%temp%\getadmin.vbs"
    exit /B

:gotAdmin
    pushd "%CD%"
    CD /D "%~dp0"
:--------------------------------------
color 07
if not exist "nightly-chs" (
    mkdir "nightly-chs"
)
cd nightly-chs

@REM 相关文件名
set "valthrunExe=ctrl.exe"
set "valthrunDrv=driver.sys"
set "mapperExe=mapper.exe"
set "mapperDrv=drv.sys"
set "buildNumTxt=nightly.txt"

call :stepEula
call :stepDownload "%~f0"
timeout /T 4
call :stepDriver
timeout /T 4
call :stepGame
call :stepController INFO

color 8F
echo.
echo 按任意键退出。
pause > nul
goto :eof


:stepEula
set "acceptInput=我愿意承担使用本程序造成的一切后果"
if exist "answered" (
    findstr /I "/C:%acceptInput%" "answered" > nul
    if "!errorlevel!" equ "0" (
        set eulaAccepted=1
    ) else (
        del "answered"
        color CF
    )
) else (
    color CF
)
echo ################################################################
echo                            _       ___       __ 
echo                \    / /\  ^|_) ^|\ ^|  ^|  ^|\ ^| /__ 
echo                 \/\/ /--\ ^| \ ^| \^| _^|_ ^| \^| \_^| 
echo.
echo ################################################################
echo.
echo  Valthrun-CHS 是一个开源软件 (GPL-2.0 许可证) 。
echo  请在使用前阅读文档: https://wiki.valth.run/#/zh-cn/
echo.
echo  使用本工具造成的一切后果由用户自行承担！
echo  使用本工具造成的一切后果由用户自行承担！
echo  使用本工具造成的一切后果由用户自行承担！
echo.
if "%eulaAccepted%" neq "1" (
    echo  如果你已阅读文档并愿意承担使用本程序造成的一切后果，
    echo  请输入“%acceptInput%”；
    echo  如果你不同意上述内容，请退出本程序。
    echo.
    set /p userInput=输入: 
    echo.
    if not "!userInput!" equ "!acceptInput!" (
        color 8F
        echo  程序将退出！
        timeout /T 4 /NOBREAK > nul
        exit
    )
    color 9F
    echo  你已同意上述内容，程序将继续运行，你可随时退出！
    echo !userInput! > answered
    timeout /T 4 /NOBREAK > nul
) else (
    color 9F
    echo  你已同意过上述内容，程序将继续运行，你可随时退出！
    echo  删除 “answered” 文件即可撤回同意。
    echo.
    pause
)
@REM color 07
goto :eof


:stepDownload
color 6F
cls
echo ################################
echo             检查更新
echo ################################
echo.

@REM 检查脚本更新
call :updateScript "%~1"

echo  正在获取最新构建信息...
curl -s https://ci.stdio.run/job/Valthrun-CHS/api/json > nightly.json
call :failedCheck "检查更新失败！"

@REM 获取最新构建号
for /f "delims=" %%i in ('powershell -Command "Get-Content nightly.json | ConvertFrom-Json | Select-Object -ExpandProperty lastSuccessfulBuild | Select-Object -ExpandProperty number"') do set "latestBuild=%%i"
del nightly.json

@REM 检查并下载最新控制器
if exist %valthrunExe% (
    @REM 检查本地文件构建号
    if exist "!buildNumTxt!" (
        set /p currentBuild=<!buildNumTxt!
    ) else (
        set currentBuild=0
    )
    if !currentBuild! LSS !latestBuild! (
        echo  正在下载最新构建版本...
        call :downloadController
        call :downloadDriver
    ) else (
        echo  本地版本已是最新构建。
    )
) else (
    echo  未找到控制器二进制文件，开始下载...
    call :downloadController
)

@REM 检查驱动程序
if not exist %valthrunDrv% (
    echo  未找到驱动程序文件，开始下载...
    call :downloadDriver
)
call :checkFileSha256AndPrompt "!valthrunDrv!"

@REM 检查映射器
if not exist %mapperExe% (
    echo  未找到驱动加载器，开始下载...
    call :downloadMapperExe
)
call :checkFileSha256AndPrompt "!mapperExe!"

if not exist %mapperDrv% (
    echo  未找到必要驱动，开始下载...
    call :downloadMapperDrv
)
call :checkFileSha256AndPrompt "!mapperDrv!"
color 2F
goto :eof


:stepDriver
color 1F
cls
echo ################################
echo             加载驱动
echo ################################
echo.
echo  正在加载驱动...

"%mapperExe%" "%mapperDrv%" "%valthrunDrv%"
call :failedCheck 驱动加载失败！请参阅文档进行错误排查
color 2F
goto :eof


:stepGame
color 6F
cls
echo ################################
echo             启动游戏
echo ################################
echo.

tasklist /FI "IMAGENAME eq cs2.exe" 2>NUL | find /I /N "cs2.exe">NUL
if "%ERRORLEVEL%"=="0" (
    color 2F
    echo  CS2 正在运行。即将启动控制器...
) else (
    echo  CS2 未运行。尝试自动启动...
    start steam://run/730
    echo  等待 CS2 启动...

    :waitloop
    tasklist /FI "IMAGENAME eq cs2.exe" 2>NUL | find /I /N "cs2.exe">NUL
    if "%ERRORLEVEL%"=="1" (
        timeout /t 1 /nobreak > NUL
        goto waitloop
    )
    color 2F
    echo  检测到游戏运行！
    timeout /t 15
    echo  正在启动控制器...
)
goto :eof


:stepController
color 07
cls
if not "%~1"=="" (
    set RUST_LOG=%~1
)
%valthrunExe%
color 6F
"%mapperExe%" "%valthrunDrv%"
goto :eof


:updateScript
@REM set "newScriptFile=%~1.update"
set "newScriptFile=launcher.bat"
set "scriptSha256File=%newScriptFile%.sha256"
@REM 如果刚更新完成，则跳过
if exist "%scriptSha256File%" (
    del "%scriptSha256File%"
    goto :eof
)
echo  正在检查脚本更新...
curl -s "https://cdn.jsdelivr.net/gh/nkxingxh/Valthrun-CHS@release/nightly/launcher.sh.sha256" > "%scriptSha256File%"
if "!errorlevel!" neq "0" (
    del "%scriptSha256File%"
    echo.
    echo 检查脚本更新失败！
    call :failedPause
    color 6F
    echo.
    goto :eof
)
@REM call :checkFileSha256 "%~1" "%scriptSha256File%"
call :checkFileSha256 "%newScriptFile%" "%scriptSha256File%"
if "%result%" equ "0" (
    del "%scriptSha256File%"
) else (
    @REM 更新脚本
    echo  正在下载最新脚本...
    call :downloadFile "!newScriptFile!" "https://cdn.jsdelivr.net/gh/nkxingxh/Valthrun-CHS@release/nightly/launcher.sh"
    if "!failed!" equ "0" (
        call :checkFileSha256AndPrompt "!newScriptFile!"
        @REM UTF-8 编码转为 系统编码 (GBK)
        PowerShell -Command "get-content '!newScriptFile!' -encoding utf8 | set-content '%~1' -encoding Oem" && start "" cmd.exe /C "%~1" && exit
        @REM move /y "!newScriptFile!" "%~1" && start "" cmd.exe /C "%~1" && exit
    ) else (
        color 6F
        echo.
    )
)
goto :eof


:downloadController
call :downloadFile "!valthrunExe!" "https://ci.stdio.run/job/Valthrun-CHS/lastSuccessfulBuild/artifact/target/release/controller.exe"
echo %latestBuild% > %buildNumTxt%
goto :eof


:downloadDriver
call :downloadFile "!valthrunDrv!" "https://cdn.jsdelivr.net/gh/nkxingxh/Valthrun-CHS@release/nightly/valthrun-driver.sys"
call :downloadFile "!valthrunDrv!.sha256" "https://cdn.jsdelivr.net/gh/nkxingxh/Valthrun-CHS@release/nightly/valthrun-driver.sys.sha256"
goto :eof


:downloadMapperExe
call :downloadFile "!mapperExe!" "https://cdn.jsdelivr.net/gh/nkxingxh/Valthrun-CHS@release/utils/driver.bin"
call :downloadFile "!mapperExe!.sha256" "https://cdn.jsdelivr.net/gh/nkxingxh/Valthrun-CHS@release/utils/driver.bin.sha256"
goto :eof


:downloadMapperDrv
call :downloadFile "!mapperDrv!" "https://cdn.jsdelivr.net/gh/nkxingxh/Valthrun-CHS@release/utils/drv.sys"
call :downloadFile "!mapperDrv!.sha256" "https://cdn.jsdelivr.net/gh/nkxingxh/Valthrun-CHS@release/utils/drv.sys.sha256"
goto :eof


:downloadFile
curl -L -o "%~1.tmp" "%~2" %~3
@REM call :failedCheck "下载文件失败！"
if "%errorlevel%" equ "0" (
    set failed=0
    if exist "%~1" (
        del "%~1"
    )
    rename "%~1.tmp" "%~1"
) else (
    set failed=1
    del "%~1.tmp"
    echo.
    echo 下载文件失败！
    call :failedPause
)
goto :eof


:checkFileSha256AndPrompt
echo  正在校验文件 %~1
call :checkFileSha256 "%~1"
if "%result%" equ "0" (
    echo  文件 %~1 校验通过！
    goto :eof
)
if "%result%" equ "1" (
    del "%~1" "%~1.sha256"
    echo.
    echo 文件 %~1 校验和不一致！请重新运行以下载该文件
    call :failedPause
    exit
)
if "%result%" equ "2" (
    del "%~1"
    echo.
    echo 校验值文件不存在！请重新运行以下载该文件
    call :failedPause
    exit
)
goto :eof


:checkFileSha256
if "%~2" equ "" (
    set "sha256File=%~1.sha256"
) else (
    set "sha256File=%~2"
)
if not exist "%sha256File%" (
    set result=2
    goto :eof
)
if not exist "%~1" (
    set result=3
    goto :eof
)
call :sha256 "%~1"
@REM echo [debug] 文件 %~1 的 SHA256 校验和为 %hash%
findstr /I /C:%hash% "%sha256File%" > nul
if "%errorlevel%" equ "0" (
    set result=0
) else (
    set result=1
)
goto :eof


:sha256
for /f "skip=1 tokens=*" %%a in ('certutil -hashfile "%~1" SHA256') do set hash=%%a & goto :eof
goto :eof


:failedCheck
if "%errorlevel%" equ "0" (
    set failed=0
) else (
    set failed=1
    echo.
    echo %~1
    call :failedPause
)
goto :eof


:failedPause
color CF
echo 如要继续运行请按任意键，退出请直接关闭窗口。
pause > nul
goto :eof
