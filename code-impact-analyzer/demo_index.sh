#!/bin/bash

# 代码影响分析工具 - 索引功能演示脚本

set -e

echo "========================================="
echo "代码影响分析工具 - 索引功能演示"
echo "========================================="
echo ""

# 检查是否提供了工作空间路径
if [ -z "$1" ]; then
    echo "用法: $0 <workspace_path> [patch_path]"
    echo ""
    echo "示例:"
    echo "  $0 ../examples/single-call ../examples/single-call/patches"
    exit 1
fi

WORKSPACE="$1"
PATCHES="${2:-$WORKSPACE/patches}"

# 检查路径是否存在
if [ ! -d "$WORKSPACE" ]; then
    echo "错误: 工作空间路径不存在: $WORKSPACE"
    exit 1
fi

if [ ! -d "$PATCHES" ]; then
    echo "错误: Patch 目录不存在: $PATCHES"
    exit 1
fi

echo "工作空间: $WORKSPACE"
echo "Patch 目录: $PATCHES"
echo ""

# 构建项目
echo "1. 构建项目..."
cargo build --release --quiet
echo "   ✓ 构建完成"
echo ""

ANALYZER="./target/release/code-impact-analyzer"

# 清除现有索引
echo "2. 清除现有索引..."
$ANALYZER --workspace "$WORKSPACE" --diff "$PATCHES" --clear-index 2>/dev/null || true
echo "   ✓ 索引已清除"
echo ""

# 首次运行（构建索引）
echo "3. 首次运行（构建索引）..."
echo "   开始时间: $(date '+%H:%M:%S')"
START_TIME=$(date +%s)

$ANALYZER --workspace "$WORKSPACE" --diff "$PATCHES" --output-format json > /tmp/result1.json 2>&1

END_TIME=$(date +%s)
DURATION=$((END_TIME - START_TIME))
echo "   结束时间: $(date '+%H:%M:%S')"
echo "   ✓ 首次运行完成，耗时: ${DURATION}秒"
echo ""

# 查看索引信息
echo "4. 查看索引信息..."
$ANALYZER --workspace "$WORKSPACE" --diff "$PATCHES" --index-info 2>/dev/null
echo ""

# 验证索引
echo "5. 验证索引..."
$ANALYZER --workspace "$WORKSPACE" --diff "$PATCHES" --verify-index 2>/dev/null
echo ""

# 第二次运行（使用缓存）
echo "6. 第二次运行（使用缓存）..."
echo "   开始时间: $(date '+%H:%M:%S')"
START_TIME=$(date +%s)

$ANALYZER --workspace "$WORKSPACE" --diff "$PATCHES" --output-format json > /tmp/result2.json 2>&1

END_TIME=$(date +%s)
DURATION=$((END_TIME - START_TIME))
echo "   结束时间: $(date '+%H:%M:%S')"
echo "   ✓ 第二次运行完成，耗时: ${DURATION}秒"
echo ""

# 比较结果
echo "7. 比较两次运行的结果..."
if diff /tmp/result1.json /tmp/result2.json > /dev/null 2>&1; then
    echo "   ✓ 两次运行结果一致"
else
    echo "   ⚠ 两次运行结果不同（这可能是正常的）"
fi
echo ""

# 强制重建索引
echo "8. 强制重建索引..."
echo "   开始时间: $(date '+%H:%M:%S')"
START_TIME=$(date +%s)

$ANALYZER --workspace "$WORKSPACE" --diff "$PATCHES" --rebuild-index --output-format json > /tmp/result3.json 2>&1

END_TIME=$(date +%s)
DURATION=$((END_TIME - START_TIME))
echo "   结束时间: $(date '+%H:%M:%S')"
echo "   ✓ 重建完成，耗时: ${DURATION}秒"
echo ""

# 清理
echo "9. 清理临时文件..."
rm -f /tmp/result1.json /tmp/result2.json /tmp/result3.json
echo "   ✓ 清理完成"
echo ""

echo "========================================="
echo "演示完成！"
echo "========================================="
echo ""
echo "索引文件位置: $WORKSPACE/.code-impact-analyzer/"
echo ""
echo "更多信息请参考:"
echo "  - INDEX_FORMAT.md  - 索引格式设计文档"
echo "  - INDEX_USAGE.md   - 索引功能使用指南"
echo ""
