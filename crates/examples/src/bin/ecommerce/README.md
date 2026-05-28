# E-commerce demo (Graphium end-to-end)

This example is intended as a “360°” Graphium demo:

- A real HTTP API (Axum + Postgres)
- Graphium graphs powering request handlers
- OpenTelemetry telemetry (metrics, logs, traces)
- Graphium UI dashboard to visualize graphs and inspect telemetry

## What you get

- API server: `graphium-examples --bin ecommerce` (default `127.0.0.1:3000`)
- Admin/UI server: `graphium-examples --bin ecommerce_ui` (default `127.0.0.1:4000`)
- Telemetry stack via docker compose:
  - Prometheus (metrics query) on `127.0.0.1:9090`
  - Loki (logs query) on `127.0.0.1:3100`
  - Tempo (traces query) on `127.0.0.1:3200`
  - Grafana (optional UI) on `127.0.0.1:3002` (`admin`/`admin`)

## Run with Docker (recommended)

From the repo root:

```bash
docker compose -f .docker/ecommerce/docker-compose.yml up -d --build
```

Open:

- API: `http://127.0.0.1:3000`
- Graphium UI: `http://127.0.0.1:4000/dashboard`
- Grafana: `http://127.0.0.1:3002`

## Try the API

### Create product (multipart)

```bash
curl -sS -X POST http://127.0.0.1:3000/product/create \
  -F 'name=Banana' \
  -F 'price=1.23'
```

Price parsing:

- Accepts integer cents (`"123"`)
- Accepts decimal strings (`"1.23"` or `"1,23"`)

### List products

```bash
curl -sS 'http://127.0.0.1:3000/product?limit=50&offset=0'
```

### Get product

```bash
curl -sS 'http://127.0.0.1:3000/product/1'
```

### Update product

```bash
curl -sS -X PUT 'http://127.0.0.1:3000/product/1' \
  -H 'content-type: application/json' \
  -d '{"name":"New name","price":999}'
```

### Delete product

```bash
curl -sS -X DELETE 'http://127.0.0.1:3000/product/1'
```

## Verify telemetry in Graphium UI

After making a few API calls:

1. Open `http://127.0.0.1:4000/dashboard`
2. Select `CreateProductGraph` (or another graph)
3. You should see:
   - Prometheus metrics cards (count/success, p50/p95 after some traffic)
   - Loki logs
   - Tempo traces, with links to the trace API

## Service name matching (important)

The UI filters logs and traces by OpenTelemetry service name. In the docker setup:

- The API service exports with `GRAPHIUM_SERVICE_NAME=ecommerce-prod`
- The UI uses `GRAPHIUM_SERVICE_NAME=ecommerce-prod` to query Loki/Tempo

If you run things manually, make sure these match (or your logs/traces won’t show up).

