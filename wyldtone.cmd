@echo off
rem Wyldtone Plugin SDK launcher for Windows CMD and PowerShell.
rem SPDX-License-Identifier: Apache-2.0
rem Only this PowerShell process bypasses script policy; system settings are unchanged.
powershell.exe -NoLogo -NoProfile -ExecutionPolicy Bypass -File "%~dp0wyldtone.ps1" %*
exit /b %ERRORLEVEL%
