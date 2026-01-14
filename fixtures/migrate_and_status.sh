#!/usr/bin/env bash
set -euo pipefail
APP_URL="${APP_URL:-http://127.0.0.1:8098}"
QW_URL="${QW_URL:-http://127.0.0.1:7280}"
SEED_IF_MISSING="${SEED_IF_MISSING:-1}"

# 如果源索引不存在，按需自动灌入模拟数据
SRC_STATUS=$(curl -s -o /dev/null -w "%{http_code}" "$QW_URL/api/v1/indexes/agent_logs" || echo 000)
if [ "$SRC_STATUS" != "200" ]; then
  if [ "$SEED_IF_MISSING" = "1" ]; then
    echo "源索引 agent_logs 不存在，自动执行模拟数据灌入..."
    bash "$(dirname "$0")/seed_agent_logs_v1.sh"
  else
    echo "源索引 agent_logs 不存在。请先运行 fixtures/seed_agent_logs_v1.sh 再迁移。" >&2
  fi
fi

# 创建新索引（已存在则忽略错误）
curl -sS "$APP_URL/api/agent/log/createV2Index" || true
sleep 1

# 触发迁移
curl -sS "$APP_URL/api/agent/log/migrateData"
sleep 1

# 查询迁移状态
curl -sS "$APP_URL/api/agent/log/migrationStatus"

# 统计新索引文档数
curl -sS -X POST "$QW_URL/api/v1/agent_logs_v2/search" -H "content-type: application/json" -d '{"query":"*","max_hits":0}'