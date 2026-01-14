#!/usr/bin/env bash
set -euo pipefail
QW_URL="${QW_URL:-http://127.0.0.1:7280}"
INDEX_ID="${INDEX_ID:-agent_logs}"
TENANT_ID="${TENANT_ID:-tenant_test}"
COUNT="${COUNT:-50}"
status=$(curl -s -o /dev/null -w "%{http_code}" "$QW_URL/api/v1/indexes/$INDEX_ID")
if [ "$status" != "200" ]; then
  cfg="/tmp/${INDEX_ID}_config.json"
  cat > "$cfg" <<'JSON'
{
  "version": "0.8",
  "index_id": "agent_logs",
  "doc_mapping": {
    "mode": "dynamic",
    "dynamic_mapping": {"indexed": true, "stored": true, "tokenizer": "default", "record": "basic"},
    "field_mappings": [
      {"name": "request_id", "type": "text", "tokenizer": "raw", "stored": true, "indexed": true, "fast": true},
      {"name": "tenant_id", "type": "text", "tokenizer": "raw", "stored": true, "indexed": true, "fast": true},
      {"name": "request_start_time", "type": "datetime", "fast": true, "stored": true},
      {"name": "user_input", "type": "text", "tokenizer": "raw", "stored": true, "indexed": true},
      {"name": "output", "type": "text", "tokenizer": "raw", "stored": true, "indexed": true}
    ],
    "tag_fields": ["tenant_id"],
    "timestamp_field": "request_start_time",
    "store_source": true
  },
  "search_settings": {"default_search_fields": ["user_input", "output"]},
  "indexing_settings": {"commit_timeout_secs": 3}
}
JSON
  curl -sS -X POST "$QW_URL/api/v1/indexes" -H "content-type: application/json" -d @"$cfg"
  sleep 1
fi
BASE_EPOCH=$(date -u +%s)
OUT="/tmp/${INDEX_ID}_seed.ndjson"
: > "$OUT"
for i in $(seq 1 "$COUNT"); do
  ts=$(date -u -r $((BASE_EPOCH - 3600 + i)) +"%Y-%m-%dT%H:%M:%SZ")
  rid=$(printf "seed_req_%03d" "$i")
  mid=$(printf "msg_%03d" "$i")
  echo "{\"request_id\":\"$rid\",\"tenant_id\":\"$TENANT_ID\",\"message_id\":\"$mid\",\"status\":\"success\",\"request_start_time\":\"$ts\",\"user_input\":\"模型测试$i\",\"output\":\"输出$i\"}" >> "$OUT"
done
curl -sS -X POST "$QW_URL/api/v1/$INDEX_ID/ingest?commit=force" -H "content-type: application/x-ndjson" --data-binary @"$OUT"
curl -sS -X POST "$QW_URL/api/v1/$INDEX_ID/search" -H "content-type: application/json" -d "{\"query\":\"tenant_id:$TENANT_ID\",\"max_hits\":0}"