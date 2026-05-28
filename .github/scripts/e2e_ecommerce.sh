#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT_DIR"

COMPOSE_FILE=".docker/ecommerce/docker-compose.yml"

cleanup() {
  docker compose -f "$COMPOSE_FILE" down -v --remove-orphans >/dev/null 2>&1 || true
}
trap cleanup EXIT

echo "Starting e2e stack..."
docker compose -f "$COMPOSE_FILE" up -d --build

echo "Waiting for services..."
for i in {1..60}; do
  if curl -fsS "http://127.0.0.1:3000/product?limit=1&offset=0" >/dev/null 2>&1; then
    break
  fi
  sleep 2
done

echo "Generating traffic..."
curl -fsS -X POST "http://127.0.0.1:3000/product/create" -F 'name=Banana' -F 'price=1.23' >/dev/null
curl -fsS -X POST "http://127.0.0.1:3000/product/create" -F 'name=Apple' -F 'price=2.00' >/dev/null

echo "Waiting for Prometheus scrape/export..."
sleep 8

PROM="http://127.0.0.1:9090"
LOKI="http://127.0.0.1:3100"
TEMPO="http://127.0.0.1:3200"

echo "Checking Prometheus has graphium metrics..."
PROM_COUNT="$(curl -fsS "$PROM/api/v1/query" --data-urlencode 'query=sum(graphium_graph_count_total)' | jq -r '.data.result[0].value[1] // "0"')"
if [[ "$PROM_COUNT" == "0" ]]; then
  echo "Expected graphium metrics in Prometheus, got count=$PROM_COUNT"
  exit 1
fi

echo "Checking Loki has graph logs..."
END_NS="$(date +%s%N)"
START_NS="$((END_NS-30*60*1000000000))"
LQ='{service_name="ecommerce-prod"} | json | body="graph started"'
LOKI_STARTED="$(curl -fsS "$LOKI/loki/api/v1/query_range" \
  --data-urlencode "query=$LQ" \
  --data-urlencode 'limit=5' \
  --data-urlencode 'direction=backward' \
  --data-urlencode "start=$START_NS" \
  --data-urlencode "end=$END_NS" | jq '[.data.result[].values[]?] | length')"
if [[ "$LOKI_STARTED" -lt 1 ]]; then
  echo "Expected at least 1 graph started log line in Loki, got $LOKI_STARTED"
  exit 1
fi

echo "Checking Tempo has traces..."
NOW_S="$(date +%s)"
START_S="$((NOW_S-30*60))"
TQ='{ .service.name = "ecommerce-prod" && .graph = "CreateProductGraph" }'
TEMPO_TRACES="$(curl -fsS "$TEMPO/api/search" \
  --data-urlencode "q=$TQ" \
  --data-urlencode 'limit=5' \
  --data-urlencode "start=$START_S" \
  --data-urlencode "end=$NOW_S" | jq '.traces | length')"
if [[ "$TEMPO_TRACES" -lt 1 ]]; then
  echo "Expected at least 1 trace in Tempo search, got $TEMPO_TRACES"
  exit 1
fi

echo "E2E OK"

