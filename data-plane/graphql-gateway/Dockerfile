# Build context is the metap-org ROOT (`../../docker-compose.yml` builds this with
# `context: ../../`, not `.`/`data-plane/`) — needed to reach the `metap-graphql-gateway = { path
# = "../../../metap/crates/metap-graphql-gateway" }` dependency in `Cargo.toml`, which points outside
# `data-plane/` and so outside any build context narrower than the org root. Same pattern
# `../services/zones-service/Dockerfile` already uses (see that file's own comment for the full
# reasoning) — this crate just sits one level shallower (`data-plane/graphql-gateway/`, not
# `data-plane/services/zones-service/`).
#
# Builds `waf-graphql-gateway` — this repo's own custom GraphQL BFF (`docs/roadmap/
# 73-waf-graphql-protocol.md`), NOT `metap/crates/metap-graphql-gateway`'s generic binary. Building the
# generic one instead (even labeled/tagged "waf-graphql-gateway") silently drops the 7 custom
# mutations (`verifyZoneDns` etc.) and the `Query.aggregate` this binary forwards to REST — found
# live, 2026-09-04, when `docker-compose.dev.yml` had exactly this mix-up.
#
# No secrets are baked in: `UPSTREAM_<N>_SERVICE_EMAIL`/`UPSTREAM_<N>_SERVICE_PASSWORD`/
# `JWKS_URL`/`COOKIE_AUTH_ENABLED` etc. are read from the environment at container start.

# ---- build -----------------------------------------------------------------
FROM rust:1-slim-bookworm AS build
WORKDIR /build
COPY metap/Cargo.toml metap/Cargo.lock metap/
COPY metap/crates metap/crates
COPY metap-demo-waf/data-plane/Cargo.toml metap-demo-waf/data-plane/Cargo.lock metap-demo-waf/data-plane/
# `data-plane/Cargo.toml` is a workspace manifest listing all 4 members (3 services +
# graphql-gateway) — cargo needs every member's own `Cargo.toml` present to resolve the workspace
# graph even when building only one of them with `-p`, so `services/` is copied too despite this
# image never running any of those binaries.
COPY metap-demo-waf/data-plane/services metap-demo-waf/data-plane/services
COPY metap-demo-waf/data-plane/graphql-gateway metap-demo-waf/data-plane/graphql-gateway
WORKDIR /build/metap-demo-waf/data-plane
RUN cargo build --release --locked -p waf-graphql-gateway

# ---- runtime -----------------------------------------------------------------
FROM debian:bookworm-slim AS runtime
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --system --gid 10001 app \
    && useradd --system --uid 10001 --gid app --home-dir /app --shell /usr/sbin/nologin app

WORKDIR /app
COPY --from=build /build/metap-demo-waf/data-plane/target/release/waf-graphql-gateway ./waf-graphql-gateway
RUN chown -R app:app /app

ENV HOST=0.0.0.0
ENV PORT=4000
EXPOSE 4000

USER app
ENTRYPOINT ["./waf-graphql-gateway"]
