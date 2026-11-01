// Boot sequence for alerting-service — 1 of 3 WAF Customer Portal backend services, ported from
// Rust's data-plane/services/alerting-service.
//
// Owns waf.security_events + waf.incidents + waf.alert_policies + waf.alert_notifications — kept
// together because Incident correlation logic calls straight into CrudService in-process;
// splitting SecurityEvent/Incident across services would turn that into a cross-service call for
// no real benefit. SecurityEvent.zoneId/Incident.zoneId/Incident.assignedTo are plain strings,
// not Reference — see entities/securityevent.go and entities/incident.go's own doc comments for
// why. All 3 services point at the SAME tenant database (Router.PoolFor) — split by
// compute/deploy/ownership, not by data layer, so Reference fields within this service's own
// entities (AlertNotification.alertPolicyId) keep a real Postgres FK.
//
// Same disclosed divergences as zones-service/scanning-service: no JWKS trust-root dual-dispatch,
// no RouteGroups toggle — see zones-service's own doc comment.
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

	"github.com/metap-org/metap-demo-waf-dataplane-go/alertingservice"
	"github.com/metap-org/metap-demo-waf-dataplane-go/alertingservice/entities"
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

	// Order is load-bearing for waf.alert_policies -> waf.alert_notifications
	// (AlertNotification.alertPolicyId builds a real FK at DDL time). waf.security_events and
	// waf.incidents have no Reference field to anything (zoneId/assignedTo etc. are plain
	// strings — see this package's own doc comment) — no FK dependency, so they reconcile
	// independently, order irrelevant. See zones-service's main.go for the PlatformTenantID
	// sentinel + BackfillScope.AllTenants reasoning — these 4 tables hold every real tenant's
	// rows in one physical table each.
	for _, entity := range []metadata.EntityDefinition{
		entities.AlertPolicyEntity(),
		entities.AlertNotificationEntity(),
		entities.SecurityEventEntity(),
		entities.IncidentEntity(),
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
	// General-purpose audit trail — every write on all 4 entities this service owns now lands a
	// row in metadata.audit_trail_entries (each opted in via its own EntityAuditConfig).
	state.Crud = crud.WithAudit(state.Router, state.Metadata, state.Permissions, audit.NewPostgresStore(state.Pool))
	// Dev binary serves plain http://localhost:3020 — a Secure session cookie is silently
	// dropped by the browser over non-HTTPS.
	state.CookieSecure = false

	rsaPubKey, ok := pubKey.(*rsa.PublicKey)
	if !ok {
		slog.Error("AUTH_JWT_PUBLIC_KEY_PATH did not parse as an RSA public key")
		os.Exit(1)
	}
	grpcServer, err := grpcapi.OptionalServe(config.Host, 3021, grpcapi.OptionalServeConfig{
		Backend: state.Crud, Router: state.Router, JWTPublicKey: rsaPubKey,
		AuthContextEntity: state.AuthContextEntity, Metadata: state.Metadata, ContextAttributesCache: state.ContextAttributesCache,
	})
	if err != nil {
		slog.Error("starting optional gRPC server", "error", err)
		os.Exit(1)
	}

	handler := httpserver.BuildRouter(state, config.CORSOrigins, func(r chi.Router) {
		alertingservice.MountRoutes(r, state)
	})

	addr := fmt.Sprintf("%s:%d", config.Host, config.Port)
	slog.Info("alerting-service listening", "addr", addr)
	if err := runtime.Serve(addr, handler); err != nil {
		slog.Error("alerting-service exited with an error", "error", err)
		os.Exit(1)
	}
	if grpcServer != nil {
		grpcServer.GracefulStop()
	}
}
