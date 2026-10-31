// Boot sequence for zones-service — 1 of 3 WAF Customer Portal backend services (zones-service/
// scanning-service/alerting-service), ported from Rust's data-plane/services/zones-service.
//
// Owns waf.domains + waf.zones + waf.ddos_policies + waf.firewall_rules + waf.ip_access_lists —
// kept together because Zone's workflow guard "activate" depends on the technical field
// hasConfig, updated by app logic whenever a DdosPolicy/FirewallRule is created/deleted for that
// zone; keeping that hook in-process (not a cross-service call) matters most at exactly this
// sensitive a spot (a workflow guard). All 3 services point at the SAME tenant database
// (Router.PoolFor) — split by compute/deploy/ownership, not by data layer, so Reference fields
// within this service's own entities keep a real Postgres FK.
//
// Divergences from the Rust original, both disclosed in their own files: no JWKS trust-root
// dual-dispatch (this Go port's httpserver.AppState only supports the static RSA keypair path —
// a pre-existing metap-go limitation, not introduced here; all 3 WAF Go services share one RSA
// keypair instead, the same topology this app used before it adopted JWKS), and no RouteGroups
// toggle (httpserver.BuildRouter always mounts every route group — attachments degrades to a
// harmless 503 with no ObjectStore configured, same practical effect as disabling it).
package main

import (
	"context"
	"crypto/rsa"
	"crypto/x509"
	"encoding/pem"
	"fmt"
	"log/slog"
	"os"
	"sync/atomic"

	"github.com/go-chi/chi/v5"
	"github.com/golang-jwt/jwt/v5"

	"github.com/metap-org/metap-go/audit"
	"github.com/metap-org/metap-go/control"
	"github.com/metap-org/metap-go/crud"
	"github.com/metap-org/metap-go/grpcapi"
	"github.com/metap-org/metap-go/httpserver"
	"github.com/metap-org/metap-go/infra"
	"github.com/metap-org/metap-go/metadata"
	"github.com/metap-org/metap-go/peripherals"
	"github.com/metap-org/metap-go/permission"
	"github.com/metap-org/metap-go/reconciler"
	"github.com/metap-org/metap-go/runtime"

	"github.com/metap-org/metap-demo-waf-dataplane-go/zonesservice"
	"github.com/metap-org/metap-demo-waf-dataplane-go/zonesservice/entities"
)

func readRSAPublicKey(path string) (any, error) {
	data, err := os.ReadFile(path)
	if err != nil {
		return nil, err
	}
	block, _ := pem.Decode(data)
	if block == nil {
		return nil, jwt.ErrKeyMustBePEMEncoded
	}
	if pub, err := x509.ParsePKIXPublicKey(block.Bytes); err == nil {
		return pub, nil
	}
	return jwt.ParseRSAPublicKeyFromPEM(data)
}

func main() {
	infra.InitTracing()
	config, err := infra.LoadConfig()
	if err != nil {
		slog.Error("loading config", "error", err)
		os.Exit(1)
	}

	ctx := context.Background()
	pool, err := infra.Connect(ctx, config.DatabaseURL)
	if err != nil {
		slog.Error("connecting to postgres", "error", err)
		os.Exit(1)
	}
	defer pool.Close()

	secretStore, err := control.BuildSecretStore(ctx, control.SecretStoreConfigFromAppConfig(config))
	if err != nil {
		slog.Error("building secret store", "error", err)
		os.Exit(1)
	}
	tenantRegistry := control.NewPostgresTenantRegistry(pool)
	router := control.NewRouter(pool, control.NewRegistryCache(tenantRegistry), secretStore)
	permissions := permission.NewPermissionService(control.NewPostgresPolicyStore(router))

	privPEM, err := os.ReadFile(config.AuthJWTPrivateKeyPath)
	if err != nil {
		slog.Error("reading AUTH_JWT_PRIVATE_KEY_PATH", "error", err)
		os.Exit(1)
	}
	pubKey, err := readRSAPublicKey(config.AuthJWTPublicKeyPath)
	if err != nil {
		slog.Error("reading AUTH_JWT_PUBLIC_KEY_PATH", "error", err)
		os.Exit(1)
	}

	registry := metadata.NewMetadataRegistry()
	if err := registry.RegisterAllSubmitted(); err != nil {
		slog.Error("registering entities", "error", err)
		os.Exit(1)
	}
	if err := registry.ValidateReferences(); err != nil {
		slog.Error("validating entity references", "error", err)
		os.Exit(1)
	}

	// Order is load-bearing: Zone.domainId builds a real FK into waf.domains, and
	// DdosPolicy.zoneId/FirewallRule.zoneId/IpAccessList.zoneId each build one into waf.zones —
	// so waf.domains must be reconciled before waf.zones, which must be reconciled before the
	// other 3 (which don't reference each other, so their own order doesn't matter).
	for _, entity := range []metadata.EntityDefinition{
		entities.DomainEntity(),
		entities.ZoneEntity(),
		entities.DdosPolicyEntity(),
		entities.FirewallRuleEntity(),
		entities.IPAccessListEntity(),
	} {
		outcome, err := reconciler.ReconcileWithScope(ctx, pool, control.PlatformTenantID, entity, nil, reconciler.ScopeAllTenants)
		if err != nil {
			slog.Error("reconciling dedicated table", "entity", entity.Name, "error", err)
			os.Exit(1)
		}
		slog.Info("reconciled dedicated table", "entity", entity.Name, "table", outcome.Table, "ops_applied", outcome.OpsApplied)
	}

	entitySummaries := registry.ListEntities()
	peripherals.Check(ctx, pool, entitySummaries)
	peripherals.Reconcile(ctx, pool, entitySummaries)

	var mdPtr atomic.Pointer[metadata.MetadataRegistry]
	mdPtr.Store(registry)

	state := httpserver.NewAppState(pool, registry, &mdPtr, permissions, string(privPEM), pubKey, router)
	// General-purpose audit trail — every write on all 5 entities this service owns now lands a
	// row in metadata.audit_trail_entries (each opted in via its own EntityAuditConfig).
	state.Crud = crud.WithAudit(state.Router, state.Metadata, state.Permissions, audit.NewPostgresStore(state.Pool))
	// Dev binary serves plain http://localhost:3000 — a Secure session cookie is silently
	// dropped by the browser over non-HTTPS, which looks exactly like "login succeeds but
	// nothing stays logged in".
	state.CookieSecure = false

	scanningGRPCAddr, _ := runtime.Optional("SCANNING_GRPC_ADDR")
	if scanningGRPCAddr == "" {
		scanningGRPCAddr = "localhost:3011"
	}
	alertingGRPCAddr, _ := runtime.Optional("ALERTING_GRPC_ADDR")
	if alertingGRPCAddr == "" {
		alertingGRPCAddr = "localhost:3021"
	}
	guardedCrud := zonesservice.NewGuardedZonesBackend(state.Crud, state.JWTPrivateKeyPEM, scanningGRPCAddr, alertingGRPCAddr)

	rsaPubKey, ok := pubKey.(*rsa.PublicKey)
	if !ok {
		slog.Error("AUTH_JWT_PUBLIC_KEY_PATH did not parse as an RSA public key")
		os.Exit(1)
	}
	grpcServer, err := grpcapi.OptionalServe(config.Host, 3001, grpcapi.OptionalServeConfig{
		Backend: guardedCrud, Router: state.Router, JWTPublicKey: rsaPubKey,
		AuthContextEntity: state.AuthContextEntity, Metadata: state.Metadata, ContextAttributesCache: state.ContextAttributesCache,
	})
	if err != nil {
		slog.Error("starting optional gRPC server", "error", err)
		os.Exit(1)
	}

	handler := httpserver.BuildRouter(state, config.CORSOrigins, func(r chi.Router) {
		zonesservice.MountRoutes(r, state)
	})

	addr := fmt.Sprintf("%s:%d", config.Host, config.Port)
	slog.Info("zones-service listening", "addr", addr)
	if err := runtime.Serve(addr, handler); err != nil {
		slog.Error("zones-service exited with an error", "error", err)
		os.Exit(1)
	}
	if grpcServer != nil {
		grpcServer.GracefulStop()
	}
}
