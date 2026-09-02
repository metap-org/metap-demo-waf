# Build context is the metap-org ROOT (`../../../../docker-compose.yml` builds this with
# `context: ../../`, not `.`/`data-plane/`) — needed to reach the `metap = { path =
# "../../../../metap/crates/metap" }` dependency in `Cargo.toml`, which points outside
# `data-plane/` and so outside any build context narrower than the org root. `../../../../metap`
# below (from this Dockerfile's own location, `data-plane/services/scanning-service/`) means the
# same `metap/` sibling repo that path dependency does — this Dockerfile just makes it visible to
# Docker by copying both trees into the image preserving their real relative nesting (`metap/` and
# `metap-demo-waf/data-plane/` as siblings), so `path = "../../../../metap/crates/metap"` resolves
# inside the image exactly like it does on disk. `../../../../.dockerignore` (root `metap-org`,
# not this repo's own) keeps that wide context from also shipping every sibling repo's
# `target/`/`node_modules/` — see that file's own comment.
#
# No secrets are baked in: DATABASE_URL/RABBITMQ_URL/AUTH_JWT_PUBLIC_KEY_PATH etc. are read
# from the environment at container start. The JWT public key must be mounted at runtime
# (e.g. `-v ./keys:/app/keys:ro`) — see the ENV note near the bottom.

# ---- build -----------------------------------------------------------------
FROM rust:1-slim-bookworm AS build
WORKDIR /build
COPY metap/Cargo.toml metap/Cargo.lock metap/
COPY metap/crates metap/crates
COPY metap-demo-waf/data-plane/Cargo.toml metap-demo-waf/data-plane/Cargo.lock metap-demo-waf/data-plane/
COPY metap-demo-waf/data-plane/services metap-demo-waf/data-plane/services
WORKDIR /build/metap-demo-waf/data-plane
RUN cargo build --release --locked -p scanning-service

# ---- runtime -----------------------------------------------------------------
FROM debian:bookworm-slim AS runtime
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --system --gid 10001 app \
    && useradd --system --uid 10001 --gid app --home-dir /app --shell /usr/sbin/nologin app

WORKDIR /app
COPY --from=build /build/metap-demo-waf/data-plane/target/release/scanning-service ./scanning-service
RUN chown -R app:app /app

ENV HOST=0.0.0.0
ENV PORT=3010
# No default for AUTH_JWT_PUBLIC_KEY_PATH on purpose — fail fast on a missing required env var
# rather than silently resolving to nothing. Mount the real key and point this at it, e.g.:
#   docker run -v ./keys:/app/keys:ro -e AUTH_JWT_PUBLIC_KEY_PATH=/app/keys/jwt-public.pem ...
EXPOSE 3010

USER app
ENTRYPOINT ["./scanning-service"]
