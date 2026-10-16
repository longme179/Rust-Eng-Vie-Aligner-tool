@echo off
rem start-labse-windows.bat — khởi động dịch vụ LaBSE (được gọi từ build-and-run-windows.bat)
rem Ghi log vào labse-service.log ở thư mục gốc của project.
cd /d "%~dp0.."
"%~dp0..\labse-venv\Scripts\python.exe" "%~dp0labse_service.py" >> labse-service.log 2>&1
