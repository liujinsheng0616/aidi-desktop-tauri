#!/bin/bash
# 调试脚本 - 运行应用并查看日志

echo "启动 AIDI Desktop..."
echo "请在应用中点击'系统优化'按钮，然后查看下方的日志输出"
echo "========================================"

# 运行应用并捕获输出
/Users/liujinsheng/workspace/all-in-aidi/aidi-desktop-tauri/src-tauri/target/release/aidi-desktop-tauri 2>&1 | tee /tmp/aidi-debug.log &

APP_PID=$!
echo "应用 PID: $APP_PID"
echo "日志文件: /tmp/aidi-debug.log"
echo ""
echo "等待 5 秒后显示日志..."
sleep 5

echo ""
echo "最近日志:"
tail -50 /tmp/aidi-debug.log

echo ""
echo "应用正在运行，请测试'系统优化'按钮"
echo "按 Ctrl+C 停止查看日志（应用会继续运行）"

# 继续显示日志
tail -f /tmp/aidi-debug.log
