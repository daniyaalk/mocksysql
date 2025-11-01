# MocksySQL

MocksySQL is a lightweight MySQL wire-protocol proxy for local testing and experimentation. It can sit between a MySQL client and server, inspect packets, optionally intercept write queries, track state diffs for UPDATEs, and replay query responses from a cache/log.

Important: This project is a work-in-progress and intended for development/testing only. Do not use in production.

## Features

- Intercept writes: When enabled, returns immediate OK responses for INSERT/UPDATE/DELETE without touching the upstream.
  - INSERT responses include a synthetic last_insert_id starting at 100 and incrementing.
- Diff tracking: Extracts column assignments and WHERE predicates from UPDATE statements and stores them as a TTL cache for quick inspection/testing.
- Optional TLS: Switches to TLS mid-handshake when the client requests SSL (uses a self-signed cert to the client and no server verification). For local testing only.
- Optional replay via Kafka: Publish full server responses keyed by the last command, and/or serve responses directly from a Kafka-fed cache instead of forwarding to the real server.
- Packet debugging: Hex-dumped packet printing at debug log level.

## Limitations

- Prepared/parameterized statements (binary protocol) are not supported.
- Multiple-result-set queries are only partially supported.
- Client protocol < 4.1 is untested.
- Multifactor authentication is unsupported.
- Compression is unsupported.
- Packets larger than 16,777,215 bytes are not supported.

## Quick start

1) Prerequisites
- Rust (edition 2021 compatible toolchain)
- A reachable MySQL server

2) Start the proxy

```bash
export BIND_ADDRESS=127.0.0.1:6033            # where MocksySQL listens (default: 127.0.0.1:6033)
export TARGET_ADDRESS=127.0.0.1:3306          # your real MySQL server

# Optional: see packets and state transitions
export RUST_LOG=debug

cargo run
```

3) Connect your client to the proxy instead of MySQL

```bash
mysql -h 127.0.0.1 -P 6033 -u root -p
```

## Configuration

### Environment variables

- BIND_ADDRESS: Address the proxy listens on. Default: 127.0.0.1:6033
- TARGET_ADDRESS: Address of the upstream MySQL server. Default: 127.0.0.1:3307
- INTERCEPT_WRITES: If "true", intercepts INSERT/UPDATE/DELETE and returns an OK locally. Default: false
- DIFF_TTL: TTL in seconds for stored UPDATE diffs. 0 means effectively no expiration. Default: 0
- PANIC_ON_UNSUPPORTED_QUERY: If "true", unsupported constructs are logged as errors; otherwise they are logged and ignored. Default: false
- DELAY_<COMMAND>: Add artificial latency (milliseconds) before forwarding a client command to the server, e.g. DELAY_SELECT=500. Applies by the first keyword of the SQL statement.

### Logging

MocksySQL uses env_logger. Set `RUST_LOG=debug` to enable verbose logging and packet hex-dumps.

### Intercepting writes

```bash
export INTERCEPT_WRITES=true
cargo run

# Now INSERT/UPDATE/DELETE will return an immediate OK from the proxy.
# INSERT will report last_insert_id starting at 100.
```

### TLS (optional feature)

Build with the tls feature to allow STARTTLS-style switching when the client advertises CLIENT_SSL during handshake:

```bash
cargo run --features tls
```

Notes:
- The proxy presents a self-signed certificate to the client.
- Upstream server verification is disabled. Use only in controlled environments.

### Replay (optional feature)

Build with the replay feature to enable Kafka-based response logging and/or replay.

Flags and variables:
- kafka_replay_log_enable=true: Publish base64-encoded server responses keyed by the last command.
- kafka_replay_response_enable=true: Serve responses directly from the replay cache (fed by Kafka) instead of forwarding certain queries to the server.
- KAFKA_HOST: Comma-separated brokers, e.g. "localhost:9092,localhost:9093"
- KAFKA_TOPIC: Topic name for logs/cache

#### Examples

```bash
# Produce responses to Kafka
export KAFKA_HOST=localhost:9092
export KAFKA_TOPIC=mocksysql
export kafka_replay_log_enable=true
cargo run --features replay

# Consume and serve responses from Kafka-fed cache
export KAFKA_HOST=localhost:9092
export KAFKA_TOPIC=mocksysql
export kafka_replay_response_enable=true
cargo run --features replay
```

Behavior with replay enabled:
- Simple startup probes like `select @@version_comment limit 1` may still pass through to the server.
- Other SELECTs and non-SELECTs will prefer serving from the replay cache when available and may not be forwarded upstream.

## How it works (high level)

- The proxy decodes MySQL packets, tracks protocol state (handshake, auth, command, pending response), and rebuilds frames when forwarding.
- For UPDATEs, it parses SQL using sqlparser to extract WHERE predicates and assignments and stores them in a TTL cache as a lightweight "diff" log.
- When write interception is enabled, it synthesizes MySQL OK packets and sends them to the client.

## Development

```bash
cargo build
cargo test
```