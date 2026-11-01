// E2E tests against a real Postgres: reconciles all 5 entities this service owns via the real
// reconciler package, then exercises GuardedZonesBackend's guards and the custom HTTP routes
// through a real CrudService + PermissionService + a real signed JWT, mirroring the rigor
// established for the rest of this migration.
package zonesservice_test

import (
	"context"
	"crypto/rand"
	"crypto/rsa"
	"crypto/x509"
	"encoding/json"
	"encoding/pem"
	"net"
	"net/http"
	"os"
	"strings"
	"sync/atomic"
	"testing"

	"github.com/go-chi/chi/v5"
	"github.com/google/uuid"
	"github.com/jackc/pgx/v5/pgxpool"

	"github.com/metap-org/metap-go/control"
	"github.com/metap-org/metap-go/crud"
	"github.com/metap-org/metap-go/httpserver"
	"github.com/metap-org/metap-go/metadata"
	"github.com/metap-org/metap-go/peripherals"
	"github.com/metap-org/metap-go/permission"
	"github.com/metap-org/metap-go/query"
	"github.com/metap-org/metap-go/reconciler"

	"github.com/metap-org/metap-demo-waf-dataplane-go/zonesservice"
	"github.com/metap-org/metap-demo-waf-dataplane-go/zonesservice/entities"
)

func testPool(t *testing.T) *pgxpool.Pool {
	url := os.Getenv("DATABASE_URL")
	if url == "" {
		url = "postgres://metap:metap@localhost:5432/metap"
	}
	pool, err := pgxpool.New(context.Background(), url)
	if err != nil {
		t.Skipf("no Postgres available (%v) — skipping e2e test", err)
	}
	if err := pool.Ping(context.Background()); err != nil {
		t.Skipf("no Postgres available (%v) — skipping e2e test", err)
	}
	t.Cleanup(pool.Close)
	return pool
}

// setupRegistry reconciles all 5 of this service's entities against pool, same boot-time order
// main.go uses, and returns the resulting MetadataRegistry.
func setupRegistry(t *testing.T, pool *pgxpool.Pool) *metadata.MetadataRegistry {
	ctx := context.Background()
	registry := metadata.NewMetadataRegistry()
	defs := []metadata.EntityDefinition{
		entities.DomainEntity(), entities.ZoneEntity(), entities.DdosPolicyEntity(),
		entities.FirewallRuleEntity(), entities.IPAccessListEntity(),
	}
	for _, def := range defs {
		if err := registry.Register(def); err != nil {
			t.Fatalf("register %s: %v", def.Name, err)
		}
		if _, err := reconciler.ReconcileWithScope(ctx, pool, control.PlatformTenantID, def, nil, reconciler.ScopeAllTenants); err != nil {
			t.Fatalf("reconcile %s: %v", def.Name, err)
		}
	}
	return registry
}

func seedAdmin(t *testing.T, pool *pgxpool.Pool, tenantID, userID uuid.UUID) {
	ctx := context.Background()
	if _, err := pool.Exec(ctx, "INSERT INTO metadata.user_roles (tenant_id, user_id, role) VALUES ($1, $2, 'admin')", tenantID, userID); err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() {
		pool.Exec(context.Background(), "DELETE FROM metadata.user_roles WHERE tenant_id = $1", tenantID)
	})
}

func cleanupEntityRows(t *testing.T, pool *pgxpool.Pool, tenantID uuid.UUID) {
	t.Cleanup(func() {
		ctx := context.Background()
		for _, table := range []string{"waf.waf_ip_access_lists", "waf.waf_firewall_rules", "waf.waf_ddos_policies", "waf.waf_zones", "waf.waf_domains"} {
			pool.Exec(ctx, "DELETE FROM "+table+" WHERE tenant_id = $1", tenantID)
		}
	})
}

func genRSAKeyPair(t *testing.T) (privatePEM string, publicKey *rsa.PublicKey) {
	key, err := rsa.GenerateKey(rand.Reader, 2048)
	if err != nil {
		t.Fatal(err)
	}
	privBytes := x509.MarshalPKCS1PrivateKey(key)
	privPEM := pem.EncodeToMemory(&pem.Block{Type: "RSA PRIVATE KEY", Bytes: privBytes})
	return string(privPEM), &key.PublicKey
}

func newTestState(t *testing.T, pool *pgxpool.Pool, registry *metadata.MetadataRegistry) (*httpserver.AppState, string) {
	tenantRegistry := control.NewPostgresTenantRegistry(pool)
	router := control.NewRouter(pool, control.NewRegistryCache(tenantRegistry), control.EnvStore{})
	permissions := permission.NewPermissionService(control.NewPostgresPolicyStore(router))
	var mdPtr atomic.Pointer[metadata.MetadataRegistry]
	mdPtr.Store(registry)
	privPEM, pubKey := genRSAKeyPair(t)
	state := httpserver.NewAppState(pool, registry, &mdPtr, permissions, privPEM, pubKey, router)
	return state, privPEM
}

func adminContext(tenantID, userID uuid.UUID) *permission.RequestContext {
	return &permission.RequestContext{TenantID: tenantID.String(), UserID: userID.String(), Roles: []string{"admin"}, HasRoles: true}
}

// freshApex returns a random apex domain — waf.domains.apexDomain is globally unique (not
// tenant-scoped, same as the Rust original: the reconciler's unique index is a plain
// `WHERE deleted = false` partial index, no tenant_id in its key), so every test that touches
// it needs its own value rather than a shared literal like "example.com" to stay independent of
// both other tests in the same run and leftover rows from a previous run against this same
// persistent dev database.
func freshApex(t *testing.T) string {
	t.Helper()
	return strings.ReplaceAll(uuid.New().String(), "-", "") + ".test"
}

func TestZonesServiceReconcilesAllFiveEntities(t *testing.T) {
	pool := testPool(t)
	registry := setupRegistry(t, pool)
	for _, name := range []string{"waf.domains", "waf.zones", "waf.ddos_policies", "waf.firewall_rules", "waf.ip_access_lists"} {
		if _, ok := registry.GetEntity(name); !ok {
			t.Errorf("expected %s to be registered", name)
		}
	}
}

func TestGuardedZonesBackendCreateZoneAutoResolvesDomainID(t *testing.T) {
	pool := testPool(t)
	registry := setupRegistry(t, pool)
	tenantID, userID := uuid.New(), uuid.New()
	seedAdmin(t, pool, tenantID, userID)
	cleanupEntityRows(t, pool, tenantID)

	state, _ := newTestState(t, pool, registry)
	backend := zonesservice.NewGuardedZonesBackend(state.Crud, "", "localhost:1", "localhost:1")
	rc := adminContext(tenantID, userID)
	ctx := context.Background()
	apex := freshApex(t)
	t.Cleanup(func() {
		pool.Exec(context.Background(), "DELETE FROM waf.waf_domains WHERE data->>'apexDomain' = $1", apex)
	})

	data := crud.JsonObject{
		"hostname": jsonMarshal(t, "shop."+apex), "originAddress": jsonMarshal(t, "1.2.3.4"),
		"protectionMode": jsonMarshal(t, "monitor"),
	}
	result, err := backend.Create(ctx, "waf.zones", data, rc, nil)
	if err != nil {
		t.Fatalf("Create: %v", err)
	}
	if !result.IsOk {
		t.Fatalf("Create failed: status=%d code=%s msg=%v fieldErrors=%v", result.Status, result.ErrorCode, result.Message, result.FieldErrors)
	}
	domainIDRaw, ok := result.Data.Data["domainId"]
	if !ok || len(domainIDRaw) == 0 {
		t.Fatalf("expected domainId to be auto-filled, got %+v", result.Data.Data)
	}

	// A new Domain row with this apex must now exist.
	domains, err := state.Crud.List(ctx, "waf.domains", query.ListInput{Limit: 10, Filters: [][2]string{{"apexDomain", apex}}}, rc)
	if err != nil {
		t.Fatalf("List domains: %v", err)
	}
	if !domains.IsOk || len(domains.Data) != 1 {
		t.Fatalf("expected exactly 1 domain for %s, got %+v", apex, domains)
	}

	// Creating a second zone under the same apex must reuse the same domain, not create a
	// second one.
	data2 := crud.JsonObject{
		"hostname": jsonMarshal(t, "api."+apex), "originAddress": jsonMarshal(t, "1.2.3.5"),
		"protectionMode": jsonMarshal(t, "monitor"),
	}
	result2, err := backend.Create(ctx, "waf.zones", data2, rc, nil)
	if err != nil || !result2.IsOk {
		t.Fatalf("Create second zone: %v / %+v", err, result2)
	}
	domains2, err := state.Crud.List(ctx, "waf.domains", query.ListInput{Limit: 10, Filters: [][2]string{{"apexDomain", apex}}}, rc)
	if err != nil || !domains2.IsOk || len(domains2.Data) != 1 {
		t.Fatalf("expected the second zone to reuse the same domain, got %+v / %v", domains2, err)
	}
}

func TestGuardedZonesBackendRejectsInvalidMatchCondition(t *testing.T) {
	pool := testPool(t)
	registry := setupRegistry(t, pool)
	tenantID, userID := uuid.New(), uuid.New()
	seedAdmin(t, pool, tenantID, userID)
	cleanupEntityRows(t, pool, tenantID)

	state, _ := newTestState(t, pool, registry)
	backend := zonesservice.NewGuardedZonesBackend(state.Crud, "", "localhost:1", "localhost:1")
	rc := adminContext(tenantID, userID)

	data := crud.JsonObject{
		"name": jsonMarshal(t, "bad rule"), "priority": jsonMarshal(t, 1),
		"matchCondition": jsonMarshal(t, map[string]any{"field": "bogus", "op": "eq", "value": "x"}),
		"ruleType":       jsonMarshal(t, "waf"), "action": jsonMarshal(t, "block"),
	}
	result, err := backend.Create(context.Background(), "waf.firewall_rules", data, rc, nil)
	if err != nil {
		t.Fatalf("Create: %v", err)
	}
	if result.IsOk {
		t.Fatal("expected invalid matchCondition to be rejected")
	}
	if result.Status != 422 || result.FieldErrors["matchCondition"] == nil {
		t.Fatalf("expected a 422 with a matchCondition field error, got status=%d fieldErrors=%v", result.Status, result.FieldErrors)
	}
}

func TestGuardedZonesBackendRejectsInvalidIPAccessListValue(t *testing.T) {
	pool := testPool(t)
	registry := setupRegistry(t, pool)
	tenantID, userID := uuid.New(), uuid.New()
	seedAdmin(t, pool, tenantID, userID)
	cleanupEntityRows(t, pool, tenantID)

	state, _ := newTestState(t, pool, registry)
	backend := zonesservice.NewGuardedZonesBackend(state.Crud, "", "localhost:1", "localhost:1")
	rc := adminContext(tenantID, userID)

	data := crud.JsonObject{"type": jsonMarshal(t, "blacklist"), "value": jsonMarshal(t, "not-an-ip")}
	result, err := backend.Create(context.Background(), "waf.ip_access_lists", data, rc, nil)
	if err != nil {
		t.Fatalf("Create: %v", err)
	}
	if result.IsOk {
		t.Fatal("expected invalid IP/CIDR to be rejected")
	}
	if result.Status != 422 || result.FieldErrors["value"] == nil {
		t.Fatalf("expected a 422 with a value field error, got status=%d fieldErrors=%v", result.Status, result.FieldErrors)
	}
}

func mustCreateDomain(t *testing.T, state *httpserver.AppState, rc *permission.RequestContext, apex string) string {
	data := crud.JsonObject{
		"apexDomain": jsonMarshal(t, apex), "verificationToken": jsonMarshal(t, "tok"),
		"verificationMethod": jsonMarshal(t, "dnsTxt"), "verificationStatus": jsonMarshal(t, "unverified"),
	}
	result, err := state.Crud.Create(context.Background(), "waf.domains", data, rc, nil)
	if err != nil || !result.IsOk {
		t.Fatalf("create domain: %v / %+v", err, result)
	}
	return result.Data.ID.String()
}

// TestSyncConfigStateOverRealHTTP exercises the full chi routing + handler + CrudService stack
// over a real network socket with a real signed JWT, mirroring httpserver's own e2e pattern.
func TestSyncConfigStateOverRealHTTP(t *testing.T) {
	pool := testPool(t)
	registry := setupRegistry(t, pool)
	tenantID, userID := uuid.New(), uuid.New()
	seedAdmin(t, pool, tenantID, userID)
	cleanupEntityRows(t, pool, tenantID)

	state, privPEM := newTestState(t, pool, registry)
	rc := adminContext(tenantID, userID)
	ctx := context.Background()
	apex := freshApex(t)
	t.Cleanup(func() {
		pool.Exec(context.Background(), "DELETE FROM waf.waf_domains WHERE data->>'apexDomain' = $1", apex)
	})

	domainID := mustCreateDomain(t, state, rc, apex)
	zoneData := crud.JsonObject{
		"hostname": jsonMarshal(t, "sync."+apex), "originAddress": jsonMarshal(t, "1.2.3.4"),
		"protectionMode": jsonMarshal(t, "monitor"), "domainId": jsonMarshal(t, domainID),
	}
	zoneResult, err := state.Crud.Create(ctx, "waf.zones", zoneData, rc, nil)
	if err != nil || !zoneResult.IsOk {
		t.Fatalf("create zone: %v / %+v", err, zoneResult)
	}
	zoneID := zoneResult.Data.ID

	// hasConfig starts false; no ddos_policies/firewall_rules exist yet for this zone.
	r := chi.NewRouter()
	zonesservice.MountRoutes(r, state)
	listener, err := net.Listen("tcp", "127.0.0.1:0")
	if err != nil {
		t.Fatal(err)
	}
	server := &http.Server{Handler: r}
	go server.Serve(listener)
	t.Cleanup(func() { server.Close() })
	baseURL := "http://" + listener.Addr().String()

	token, err := peripherals.MintJWT(privPEM, tenantID, userID, 3600)
	if err != nil {
		t.Fatal(err)
	}

	// No related records yet — sync should report hasConfig: false, changed: false (it's
	// already correct on a freshly-created zone).
	resp := doPost(t, baseURL+"/api/waf.zones/"+zoneID.String()+"/sync-config-state", token)
	body := decodeJSON(t, resp)
	data := body["data"].(map[string]any)
	if data["hasConfig"] != false || data["changed"] != false {
		t.Fatalf("expected hasConfig=false changed=false on a fresh zone, got %+v", data)
	}

	// Add a DdosPolicy for this zone, then sync again — hasConfig should flip to true.
	ddosData := crud.JsonObject{
		"zoneId": jsonMarshal(t, zoneID.String()), "sensitivity": jsonMarshal(t, "medium"),
		"requestRateThreshold": jsonMarshal(t, 100), "burstWindow": jsonMarshal(t, 10),
		"action": jsonMarshal(t, "log"), "priority": jsonMarshal(t, 1),
	}
	if _, err := state.Crud.Create(ctx, "waf.ddos_policies", ddosData, rc, nil); err != nil {
		t.Fatalf("create ddos policy: %v", err)
	}

	resp = doPost(t, baseURL+"/api/waf.zones/"+zoneID.String()+"/sync-config-state", token)
	body = decodeJSON(t, resp)
	data = body["data"].(map[string]any)
	if data["hasConfig"] != true || data["changed"] != true {
		t.Fatalf("expected hasConfig=true changed=true after adding a DdosPolicy, got %+v", data)
	}

	// Calling it again is idempotent: still true, but changed=false now since it's already
	// correct.
	resp = doPost(t, baseURL+"/api/waf.zones/"+zoneID.String()+"/sync-config-state", token)
	body = decodeJSON(t, resp)
	data = body["data"].(map[string]any)
	if data["hasConfig"] != true || data["changed"] != false {
		t.Fatalf("expected a repeat sync to be a no-op, got %+v", data)
	}
}

func doPost(t *testing.T, url, token string) *http.Response {
	t.Helper()
	req, err := http.NewRequest(http.MethodPost, url, strings.NewReader(""))
	if err != nil {
		t.Fatal(err)
	}
	req.Header.Set("Authorization", "Bearer "+token)
	resp, err := http.DefaultClient.Do(req)
	if err != nil {
		t.Fatal(err)
	}
	if resp.StatusCode != http.StatusOK {
		t.Fatalf("expected 200, got %d", resp.StatusCode)
	}
	return resp
}

func decodeJSON(t *testing.T, resp *http.Response) map[string]any {
	t.Helper()
	defer resp.Body.Close()
	var v map[string]any
	if err := json.NewDecoder(resp.Body).Decode(&v); err != nil {
		t.Fatal(err)
	}
	return v
}

func jsonMarshal(t *testing.T, v any) json.RawMessage {
	t.Helper()
	b, err := json.Marshal(v)
	if err != nil {
		t.Fatal(err)
	}
	return b
}
