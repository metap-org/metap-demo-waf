// E2E tests against a real Postgres: reconciles both entities this service owns, then exercises
// the full run-scan-job -> submit-findings lifecycle over a real HTTP server with a real signed
// JWT, mirroring the rigor established for the rest of this migration.
package scanningservice_test

import (
	"bytes"
	"context"
	"crypto/rand"
	"crypto/rsa"
	"crypto/x509"
	"encoding/json"
	"encoding/pem"
	"net"
	"net/http"
	"os"
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

	"github.com/metap-org/metap-demo-waf-dataplane-go/scanningservice"
	"github.com/metap-org/metap-demo-waf-dataplane-go/scanningservice/entities"
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

func setupRegistry(t *testing.T, pool *pgxpool.Pool) *metadata.MetadataRegistry {
	ctx := context.Background()
	registry := metadata.NewMetadataRegistry()
	for _, def := range []metadata.EntityDefinition{entities.ScanJobEntity(), entities.ScanFindingEntity()} {
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
		for _, table := range []string{"waf.waf_scan_findings", "waf.waf_scan_jobs"} {
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

func jsonMarshal(t *testing.T, v any) json.RawMessage {
	t.Helper()
	b, err := json.Marshal(v)
	if err != nil {
		t.Fatal(err)
	}
	return b
}

func TestScanningServiceReconcilesBothEntities(t *testing.T) {
	pool := testPool(t)
	registry := setupRegistry(t, pool)
	for _, name := range []string{"waf.scan_jobs", "waf.scan_findings"} {
		if _, ok := registry.GetEntity(name); !ok {
			t.Errorf("expected %s to be registered", name)
		}
	}
}

func doPost(t *testing.T, url, token string, body []byte) *http.Response {
	t.Helper()
	var reader *bytes.Reader
	if body != nil {
		reader = bytes.NewReader(body)
	} else {
		reader = bytes.NewReader(nil)
	}
	req, err := http.NewRequest(http.MethodPost, url, reader)
	if err != nil {
		t.Fatal(err)
	}
	if body != nil {
		req.Header.Set("Content-Type", "application/json")
	}
	req.Header.Set("Authorization", "Bearer "+token)
	resp, err := http.DefaultClient.Do(req)
	if err != nil {
		t.Fatal(err)
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

// TestRunScanJobWithNoScannerConfiguredJustQueues confirms that with SCANNER_URL unset, running
// a job transitions it to "queued" and reports dispatched: false rather than pretending a scan
// happened.
func TestRunScanJobWithNoScannerConfiguredJustQueues(t *testing.T) {
	os.Unsetenv("SCANNER_URL")
	pool := testPool(t)
	registry := setupRegistry(t, pool)
	tenantID, userID := uuid.New(), uuid.New()
	seedAdmin(t, pool, tenantID, userID)
	cleanupEntityRows(t, pool, tenantID)

	state, privPEM := newTestState(t, pool, registry)
	rc := adminContext(tenantID, userID)
	ctx := context.Background()

	jobData := crud.JsonObject{"zoneId": jsonMarshal(t, "zone-"+uuid.New().String()), "scanType": jsonMarshal(t, "quickScan")}
	jobResult, err := state.Crud.Create(ctx, "waf.scan_jobs", jobData, rc, nil)
	if err != nil || !jobResult.IsOk {
		t.Fatalf("create job: %v / %+v", err, jobResult)
	}
	jobID := jobResult.Data.ID

	r := chi.NewRouter()
	scanningservice.MountRoutes(r, state)
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

	resp := doPost(t, baseURL+"/api/waf.scan_jobs/"+jobID.String()+"/run", token, nil)
	if resp.StatusCode != http.StatusOK {
		t.Fatalf("expected 200, got %d", resp.StatusCode)
	}
	body := decodeJSON(t, resp)
	data := body["data"].(map[string]any)
	if data["dispatched"] != false {
		t.Fatalf("expected dispatched=false with no scanner configured, got %+v", data)
	}
	job := data["job"].(map[string]any)
	if job["status"] != "queued" {
		t.Fatalf("expected status=queued, got %+v", job)
	}

	// Running it again while queued must be rejected with 409.
	resp2 := doPost(t, baseURL+"/api/waf.scan_jobs/"+jobID.String()+"/run", token, nil)
	if resp2.StatusCode != http.StatusConflict {
		t.Fatalf("expected 409 for a job already queued, got %d", resp2.StatusCode)
	}
	resp2.Body.Close()
}

// TestSubmitFindingsCompletesTheJobAndCreatesFindings exercises the scanner-callback endpoint
// directly (bypassing run-scan-job, since this endpoint accepts a job in any state the real
// scanner integration would reach it from) and confirms it creates ScanFinding rows, walks the
// job through start if still queued, and completes it with lastRunAt set.
func TestSubmitFindingsCompletesTheJobAndCreatesFindings(t *testing.T) {
	pool := testPool(t)
	registry := setupRegistry(t, pool)
	tenantID, userID := uuid.New(), uuid.New()
	seedAdmin(t, pool, tenantID, userID)
	cleanupEntityRows(t, pool, tenantID)

	state, privPEM := newTestState(t, pool, registry)
	rc := adminContext(tenantID, userID)
	ctx := context.Background()

	jobData := crud.JsonObject{"zoneId": jsonMarshal(t, "zone-"+uuid.New().String()), "scanType": jsonMarshal(t, "fullScan")}
	jobResult, err := state.Crud.Create(ctx, "waf.scan_jobs", jobData, rc, nil)
	if err != nil || !jobResult.IsOk {
		t.Fatalf("create job: %v / %+v", err, jobResult)
	}
	jobID := jobResult.Data.ID

	// Walk the job to "queued" first via a real transition, matching what run-scan-job would
	// have done before a scanner calls back.
	queuedResult, err := state.Crud.Transition(ctx, "waf.scan_jobs", jobID, "run", jobResult.Data.Version, nil, rc, nil)
	if err != nil || !queuedResult.IsOk {
		t.Fatalf("transition to queued: %v / %+v", err, queuedResult)
	}

	r := chi.NewRouter()
	scanningservice.MountRoutes(r, state)
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

	findingsBody, _ := json.Marshal(map[string]any{
		"findings": []map[string]any{
			{"severity": "high", "category": "sqli", "endpoint": "/login", "description": "SQL injection in login form"},
			{"severity": "low", "category": "headers", "endpoint": "/"},
		},
		"succeeded": true,
	})
	resp := doPost(t, baseURL+"/internal/scan-jobs/"+jobID.String()+"/findings", token, findingsBody)
	if resp.StatusCode != http.StatusOK {
		t.Fatalf("expected 200, got %d", resp.StatusCode)
	}
	result := decodeJSON(t, resp)
	data := result["data"].(map[string]any)
	created := data["createdFindings"].([]any)
	if len(created) != 2 {
		t.Fatalf("expected 2 created findings, got %+v", created)
	}
	job := data["job"].(map[string]any)
	if job["status"] != "completed" {
		t.Fatalf("expected the job to be completed, got %+v", job)
	}
	jobFields := job["data"].(map[string]any)
	if lastRunAt, ok := jobFields["lastRunAt"].(string); !ok || lastRunAt == "" {
		t.Fatalf("expected lastRunAt to be set, got %+v", jobFields)
	}

	// Both findings must actually exist, scoped to this job.
	findings, err := state.Crud.List(ctx, "waf.scan_findings",
		query.ListInput{Limit: 10, Filters: [][2]string{{"scanJobId", jobID.String()}}}, rc)
	if err != nil {
		t.Fatalf("list findings: %v", err)
	}
	if !findings.IsOk || len(findings.Data) != 2 {
		t.Fatalf("expected 2 findings for this job, got %+v", findings)
	}
}
