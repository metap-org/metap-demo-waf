// Package config is edge node configuration, read once at boot from the environment.
//
// Hand-rolled rather than reusing any shared helper: edge-plane deliberately does not depend on
// metap (or on control-plane/go) at all. Ten lines of os.Getenv is a much smaller cost than any
// dependency that would drag in.
package config

import (
	"fmt"
	"os"
	"strconv"
	"time"
)

type Config struct {
	ListenAddr string
	// RedisURL: where compiled rule-sets are read from. The only thing this process reads
	// config from — never data-plane.
	RedisURL string
	// RefreshInterval: how often the snapshot is refreshed. This value IS the
	// config-propagation SLA (10-30s in the architecture doc).
	RefreshInterval time.Duration
	// IngestURL: control-plane's ingest endpoint. Telemetry goes here, never to data-plane.
	IngestURL              string
	IngestToken            *string
	TelemetryBuffer        int
	TelemetryMaxBatch      int
	TelemetryFlushInterval time.Duration
	// ClientIPHeader: header carrying the client's real IP when this node sits behind another
	// proxy or an L4 load balancer. Unset by default, and that is the safe default: trusting a
	// client-supplied header would let anyone spoof their source IP past every IP rule and every
	// rate limit. Only set this when something in front is guaranteed to overwrite it.
	ClientIPHeader *string
	// GeoCountryHeader: header carrying an already-resolved ISO-3166 country code. There is no
	// GeoIP database in this build; country rules simply never match unless something upstream
	// provides this.
	GeoCountryHeader *string
	ClearanceCookie  string
	ClearanceTTL     time.Duration
	// ClearanceSecret: secret the clearance cookie is keyed with. A per-node random default
	// means a restart invalidates outstanding clearances (a re-challenge, not an outage); set it
	// explicitly to share clearances across a fleet.
	ClearanceSecret string
}

func stringVar(name string) *string {
	v, ok := os.LookupEnv(name)
	if !ok || v == "" {
		return nil
	}
	return &v
}

func stringOr(name, def string) string {
	if v := stringVar(name); v != nil {
		return *v
	}
	return def
}

func durationSecondsOr(name string, defSeconds uint64) time.Duration {
	if v := stringVar(name); v != nil {
		if n, err := strconv.ParseUint(*v, 10, 64); err == nil {
			return time.Duration(n) * time.Second
		}
	}
	return time.Duration(defSeconds) * time.Second
}

func durationMillisOr(name string, defMillis uint64) time.Duration {
	if v := stringVar(name); v != nil {
		if n, err := strconv.ParseUint(*v, 10, 64); err == nil {
			return time.Duration(n) * time.Millisecond
		}
	}
	return time.Duration(defMillis) * time.Millisecond
}

func uintOr(name string, def int) int {
	if v := stringVar(name); v != nil {
		if n, err := strconv.Atoi(*v); err == nil {
			return n
		}
	}
	return def
}

func Load() Config {
	return Config{
		ListenAddr:             stringOr("LISTEN_ADDR", "0.0.0.0:8080"),
		RedisURL:               stringOr("REDIS_URL", "redis://localhost:6379"),
		RefreshInterval:        durationSecondsOr("REFRESH_INTERVAL_SECONDS", 10),
		IngestURL:              stringOr("INGEST_URL", "http://localhost:4100/ingest/events"),
		IngestToken:            stringVar("INGEST_TOKEN"),
		TelemetryBuffer:        uintOr("TELEMETRY_BUFFER", 10_000),
		TelemetryMaxBatch:      uintOr("TELEMETRY_MAX_BATCH", 100),
		TelemetryFlushInterval: durationMillisOr("TELEMETRY_FLUSH_MS", 2_000),
		ClientIPHeader:         stringVar("CLIENT_IP_HEADER"),
		GeoCountryHeader:       stringVar("GEO_COUNTRY_HEADER"),
		ClearanceCookie:        stringOr("CLEARANCE_COOKIE", "waf_clearance"),
		ClearanceTTL:           durationSecondsOr("CLEARANCE_TTL_SECONDS", 1_800),
		ClearanceSecret:        stringOr("CLEARANCE_SECRET", randomSecret()),
	}
}

// randomSecret: node-local secret when none is configured. Not cryptographically strong entropy
// — it is a startup nonce, and its only job is that two nodes (or two runs) don't accept each
// other's clearance cookies by accident.
func randomSecret() string {
	return fmt.Sprintf("edge-%x", time.Now().UnixNano())
}
