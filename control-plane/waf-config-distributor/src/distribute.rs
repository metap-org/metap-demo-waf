//! The "distribute" half: writing compiled rule-sets into Redis/DragonflyDB for the edge to read.
//!
//! **Why the plain `redis` crate and not `metap-cache`'s `RedisCache`**, which
//! `04-architecture-boundary.md` suggested reusing: that trait is a *cache* — one fixed TTL per
//! instance and keys scoped as `{tenant_id}:{key}` by the crate itself. Neither fits here. This
//! data is not a cache: if it expires, the edge loses its configuration and a zone silently stops
//! being protected; there is no origin to re-read it from on the hot path. And the key layout is
//! a **published contract** consumed by a process that deliberately does not depend on `metap`
//! (`ruleset.rs`) — it cannot be an internal detail of a `metap` crate. Tenancy is still in the
//! payload, and hostnames are globally unique in `data-plane` (`Zone.hostname` is `unique`), so
//! keying by hostname is safe.
//!
//! Writes are pipelined per zone so a reader never sees the index updated before the rule-set it
//! points at.

use anyhow::Context;
use redis::AsyncCommands;

use crate::ruleset::{zone_key, zone_version_key, CompiledZone, EPOCH_KEY, ZONE_INDEX_KEY};

#[derive(Clone)]
pub struct Distributor {
    client: redis::Client,
}

impl Distributor {
    pub fn connect(redis_url: &str) -> anyhow::Result<Self> {
        let client = redis::Client::open(redis_url).with_context(|| format!("opening redis at {redis_url}"))?;
        Ok(Self { client })
    }

    async fn conn(&self) -> anyhow::Result<redis::aio::MultiplexedConnection> {
        self.client
            .get_multiplexed_async_connection()
            .await
            .context("connecting to redis")
    }

    /// Publishes one zone. Rule-set and version are written before the index gains the hostname,
    /// so an edge that reads the index mid-write always finds a complete rule-set behind it.
    pub async fn publish(&self, zone: &CompiledZone) -> anyhow::Result<()> {
        let payload = serde_json::to_string(zone).context("serializing compiled zone")?;
        let mut conn = self.conn().await?;
        redis::pipe()
            .atomic()
            .set(zone_key(&zone.hostname), payload)
            .ignore()
            .set(zone_version_key(&zone.hostname), zone.config_version)
            .ignore()
            .sadd(ZONE_INDEX_KEY, &zone.hostname)
            .ignore()
            .query_async::<()>(&mut conn)
            .await
            .context("writing compiled zone to redis")?;
        tracing::info!(
            hostname = zone.hostname,
            config_version = zone.config_version,
            rules = zone.rules.len(),
            "published rule-set"
        );
        Ok(())
    }

    /// Removes a zone the edge must stop serving (deleted, suspended, or back to `pending`).
    /// Index membership goes first here — the mirror of `publish` — so a reader never finds a
    /// hostname in the index whose rule-set has already been deleted.
    pub async fn unpublish(&self, hostname: &str) -> anyhow::Result<()> {
        let mut conn = self.conn().await?;
        redis::pipe()
            .atomic()
            .srem(ZONE_INDEX_KEY, hostname)
            .ignore()
            .del(zone_key(hostname))
            .ignore()
            .del(zone_version_key(hostname))
            .ignore()
            .query_async::<()>(&mut conn)
            .await
            .context("removing zone from redis")?;
        tracing::info!(hostname, "unpublished rule-set");
        Ok(())
    }

    /// Hostnames currently published — what a full resync diffs against to find zones that have
    /// disappeared from `data-plane` without this worker ever seeing the delete event.
    pub async fn published_hostnames(&self) -> anyhow::Result<Vec<String>> {
        let mut conn = self.conn().await?;
        let members: Vec<String> = conn.smembers(ZONE_INDEX_KEY).await.context("reading zone index")?;
        Ok(members)
    }

    /// Bumped at the end of every successful full resync. An edge that has been disconnected long
    /// enough to distrust its incremental state compares this and reloads everything.
    pub async fn bump_epoch(&self) -> anyhow::Result<u64> {
        let mut conn = self.conn().await?;
        let epoch: u64 = conn.incr(EPOCH_KEY, 1).await.context("bumping epoch")?;
        Ok(epoch)
    }

    pub async fn ping(&self) -> anyhow::Result<()> {
        let mut conn = self.conn().await?;
        redis::cmd("PING").query_async::<()>(&mut conn).await.context("redis ping")
    }
}
