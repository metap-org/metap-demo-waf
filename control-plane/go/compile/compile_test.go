package compile

import (
	"encoding/json"
	"testing"

	"github.com/metap-org/metap-demo-waf-controlplane-go/dataplane"
	"github.com/metap-org/metap-demo-waf-controlplane-go/ruleset"
)

func rawJSON(t *testing.T, v any) json.RawMessage {
	t.Helper()
	b, err := json.Marshal(v)
	if err != nil {
		t.Fatal(err)
	}
	return b
}

func record(id string, status *string, data map[string]any) dataplane.Record {
	fields := make(map[string]json.RawMessage, len(data))
	for k, v := range data {
		b, _ := json.Marshal(v)
		fields[k] = b
	}
	return dataplane.Record{ID: id, Status: status, Data: fields}
}

func strPtr(s string) *string { return &s }

func zoneRecord(t *testing.T, status string, extra map[string]any) dataplane.Record {
	data := map[string]any{
		"hostname":       "shop.example.com",
		"originAddress":  "10.0.0.1",
		"status":         status,
		"protectionMode": "enforce",
		"configVersion":  3,
	}
	for k, v := range extra {
		data[k] = v
	}
	return record("zone-1", strPtr(status), data)
}

func TestPublishableAcceptsActiveAndPausedOnly(t *testing.T) {
	if !Publishable(zoneRecord(t, "active", nil)) {
		t.Error("expected active to be publishable")
	}
	if !Publishable(zoneRecord(t, "paused", nil)) {
		t.Error("expected paused to be publishable")
	}
	if Publishable(zoneRecord(t, "pending", nil)) {
		t.Error("expected pending to not be publishable")
	}
	if Publishable(zoneRecord(t, "suspended", nil)) {
		t.Error("expected suspended to not be publishable")
	}
}

func TestActionFromUnknownValueDefaultsToLogNotBlock(t *testing.T) {
	cases := map[string]ruleset.Action{
		"something-new": ruleset.ActionLog,
		"":              ruleset.ActionLog,
		"block":         ruleset.ActionBlock,
		"challenge":     ruleset.ActionChallenge,
		"allow":         ruleset.ActionAllow,
	}
	for input, want := range cases {
		if got := actionFrom(input); got != want {
			t.Errorf("actionFrom(%q) = %q, want %q", input, got, want)
		}
	}
}

func TestParseMatchNoneAndNullBothMeanAlways(t *testing.T) {
	expr, ok := ParseMatch(nil)
	if !ok || expr.Kind != ruleset.MatchAlways {
		t.Fatalf("expected Always for nil, got %+v ok=%v", expr, ok)
	}
	expr, ok = ParseMatch(json.RawMessage("null"))
	if !ok || expr.Kind != ruleset.MatchAlways {
		t.Fatalf("expected Always for null, got %+v ok=%v", expr, ok)
	}
}

func TestParseMatchSimplePredicate(t *testing.T) {
	raw := rawJSON(t, map[string]any{"field": "uri.path", "op": "contains", "value": "/admin"})
	expr, ok := ParseMatch(raw)
	if !ok || expr.Kind != ruleset.MatchPredicate {
		t.Fatalf("expected a Predicate, got %+v ok=%v", expr, ok)
	}
	p := expr.Predicate
	if p.Field != ruleset.FieldUriPath || p.Op != ruleset.OpContains || p.Value == nil || *p.Value != "/admin" {
		t.Errorf("unexpected predicate: %+v", p)
	}
}

func TestParseMatchHeaderFieldLowercasesTheHeaderName(t *testing.T) {
	raw := rawJSON(t, map[string]any{"field": "header", "op": "eq", "value": "1", "param": "X-Custom-Header"})
	expr, ok := ParseMatch(raw)
	if !ok {
		t.Fatal("expected a match")
	}
	if expr.Predicate.Param == nil || *expr.Predicate.Param != "x-custom-header" {
		t.Errorf("expected lowercased param, got %+v", expr.Predicate.Param)
	}
}

func TestParseMatchHeaderFieldWithoutParamIsUnrepresentable(t *testing.T) {
	raw := rawJSON(t, map[string]any{"field": "header", "op": "eq", "value": "1"})
	if _, ok := ParseMatch(raw); ok {
		t.Fatal("expected header field without param to be unrepresentable")
	}
}

func TestParseMatchAllAnyNotCompose(t *testing.T) {
	raw := rawJSON(t, map[string]any{
		"all": []any{
			map[string]any{"field": "method", "op": "eq", "value": "POST"},
			map[string]any{"not": map[string]any{"field": "sourceIp", "op": "eq", "value": "10.0.0.1"}},
		},
	})
	expr, ok := ParseMatch(raw)
	if !ok || expr.Kind != ruleset.MatchAll {
		t.Fatalf("expected All, got %+v ok=%v", expr, ok)
	}
	if len(expr.Children) != 2 {
		t.Fatalf("expected 2 children, got %d", len(expr.Children))
	}
	if expr.Children[1].Kind != ruleset.MatchNot {
		t.Errorf("expected second child to be Not, got %+v", expr.Children[1])
	}
}

func TestParseMatchInRequiresNonEmptyValues(t *testing.T) {
	empty := rawJSON(t, map[string]any{"field": "sourceIp", "op": "in", "values": []string{}})
	if _, ok := ParseMatch(empty); ok {
		t.Error("expected empty values with op=in to be unrepresentable")
	}

	filled := rawJSON(t, map[string]any{"field": "sourceIp", "op": "in", "values": []string{"1.2.3.4", "5.6.7.8"}})
	expr, ok := ParseMatch(filled)
	if !ok {
		t.Fatal("expected a match")
	}
	if len(expr.Predicate.Values) != 2 || expr.Predicate.Values[0] != "1.2.3.4" || expr.Predicate.Values[1] != "5.6.7.8" {
		t.Errorf("unexpected values: %+v", expr.Predicate.Values)
	}
}

func TestParseMatchRegexWithAValidPatternCompiles(t *testing.T) {
	raw := rawJSON(t, map[string]any{"field": "uri.path", "op": "regex", "value": `^/admin/\d+$`})
	expr, ok := ParseMatch(raw)
	if !ok || expr.Kind != ruleset.MatchPredicate {
		t.Fatalf("expected a Predicate, got %+v ok=%v", expr, ok)
	}
	if expr.Predicate.Field != ruleset.FieldUriPath || expr.Predicate.Op != ruleset.OpRegex || *expr.Predicate.Value != `^/admin/\d+$` {
		t.Errorf("unexpected predicate: %+v", expr.Predicate)
	}
}

func TestParseMatchRegexWithAnInvalidPatternIsUnrepresentable(t *testing.T) {
	raw := rawJSON(t, map[string]any{"field": "uri.path", "op": "regex", "value": "[invalid(regex"})
	if _, ok := ParseMatch(raw); ok {
		t.Fatal("expected an invalid regex to be unrepresentable")
	}
}

func TestParseMatchUnknownFieldOrOpIsUnrepresentable(t *testing.T) {
	if _, ok := ParseMatch(rawJSON(t, map[string]any{"field": "bogus", "op": "eq", "value": "x"})); ok {
		t.Error("expected an unknown field to be unrepresentable")
	}
	if _, ok := ParseMatch(rawJSON(t, map[string]any{"field": "method", "op": "bogus", "value": "x"})); ok {
		t.Error("expected an unknown op to be unrepresentable")
	}
}

func TestParseMatchMissingValueForANonInOpIsUnrepresentable(t *testing.T) {
	if _, ok := ParseMatch(rawJSON(t, map[string]any{"field": "method", "op": "eq"})); ok {
		t.Error("expected a missing value to be unrepresentable")
	}
}

func TestCompileRuleDropsDisabledRules(t *testing.T) {
	rule := record("rule-1", nil, map[string]any{
		"name": "r", "ruleType": "waf", "action": "block", "priority": 10, "enabled": false,
	})
	if _, ok := compileRule(rule); ok {
		t.Error("expected disabled rule to be dropped")
	}
}

func TestCompileRuleWithUnrepresentableConditionIsDropped(t *testing.T) {
	rule := record("rule-1", nil, map[string]any{
		"name": "r", "ruleType": "waf", "action": "block", "priority": 10, "enabled": true,
		"matchCondition": map[string]any{"field": "bogus", "op": "eq", "value": "x"},
	})
	if _, ok := compileRule(rule); ok {
		t.Error("expected unrepresentable condition to be dropped")
	}
}

func TestCompileRuleRateLimitTypeCarriesThresholdAndWindow(t *testing.T) {
	rule := record("rule-1", nil, map[string]any{
		"name": "login-rl", "ruleType": "rateLimit", "action": "challenge", "priority": 5,
		"enabled": true, "rateLimitThreshold": 20, "rateLimitWindow": 30,
	})
	compiled, ok := compileRule(rule)
	if !ok {
		t.Fatal("expected a compiled rule")
	}
	if compiled.ID != "rule-1" || compiled.Action != ruleset.ActionChallenge {
		t.Errorf("unexpected compiled rule: %+v", compiled)
	}
	if compiled.RateLimit == nil || compiled.RateLimit.Threshold != 20 || compiled.RateLimit.WindowSeconds != 30 {
		t.Errorf("expected rate limit 20/30, got %+v", compiled.RateLimit)
	}
	if compiled.MatchExpr.Kind != ruleset.MatchAlways {
		t.Errorf("expected Always with no condition given, got %+v", compiled.MatchExpr)
	}
}

func TestCompileRuleNonRateLimitTypeHasNoRateLimit(t *testing.T) {
	rule := record("rule-1", nil, map[string]any{
		"name": "r", "ruleType": "waf", "action": "block", "priority": 10, "enabled": true,
	})
	compiled, ok := compileRule(rule)
	if !ok || compiled.RateLimit != nil {
		t.Errorf("expected no rate limit, got %+v ok=%v", compiled, ok)
	}
}

func TestCompileDdosDropsWhenDisabled(t *testing.T) {
	policy := record("policy-1", nil, map[string]any{
		"sensitivity": "high", "action": "block", "requestRateThreshold": 500, "burstWindow": 60, "enabled": false,
	})
	if _, ok := compileDdos(policy); ok {
		t.Error("expected disabled policy to be dropped")
	}
}

func TestCompileDdosEnabledCarriesFieldsThrough(t *testing.T) {
	policy := record("policy-1", nil, map[string]any{
		"sensitivity": "high", "action": "block", "requestRateThreshold": 500, "burstWindow": 60, "enabled": true,
	})
	compiled, ok := compileDdos(policy)
	if !ok {
		t.Fatal("expected a compiled policy")
	}
	if compiled.Sensitivity != "high" || compiled.Action != ruleset.ActionBlock ||
		compiled.RequestRateThreshold != 500 || compiled.BurstWindowSeconds != 60 {
		t.Errorf("unexpected compiled policy: %+v", compiled)
	}
}

func TestCompileZoneSortsRulesByPriorityAndDropsUnrepresentableOnes(t *testing.T) {
	z := zoneRecord(t, "active", nil)
	rules := []dataplane.Record{
		record("r-high-priority-num", nil, map[string]any{"name": "second", "ruleType": "waf", "action": "log", "priority": 200, "enabled": true}),
		record("r-low-priority-num", nil, map[string]any{"name": "first", "ruleType": "waf", "action": "block", "priority": 10, "enabled": true}),
		record("r-disabled", nil, map[string]any{"name": "disabled", "ruleType": "waf", "action": "block", "priority": 1, "enabled": false}),
	}
	compiled, ok := CompileZone(z, "tenant-1", nil, rules, nil)
	if !ok {
		t.Fatal("expected a compiled zone")
	}
	if len(compiled.Rules) != 2 {
		t.Fatalf("expected the disabled rule to be dropped, got %d rules", len(compiled.Rules))
	}
	if compiled.Rules[0].Name != "first" || compiled.Rules[1].Name != "second" {
		t.Errorf("expected priority ascending order, got %+v", compiled.Rules)
	}
	if compiled.SchemaVersion != ruleset.RulesetSchemaVersion || compiled.TenantID != "tenant-1" || compiled.ZoneID != "zone-1" {
		t.Errorf("unexpected zone metadata: %+v", compiled)
	}
}

func TestCompileZoneWithoutHostnameFails(t *testing.T) {
	z := record("zone-1", strPtr("active"), map[string]any{"status": "active"})
	if _, ok := CompileZone(z, "tenant-1", nil, nil, nil); ok {
		t.Error("expected a zone with no hostname to fail to compile")
	}
}

func TestCompileZoneCarriesDdosOnlyWhenPresentAndEnabled(t *testing.T) {
	z := zoneRecord(t, "active", nil)
	ddos := record("ddos-1", nil, map[string]any{
		"sensitivity": "medium", "action": "challenge", "requestRateThreshold": 500, "burstWindow": 60, "enabled": true,
	})
	compiled, ok := CompileZone(z, "tenant-1", []dataplane.Record{ddos}, nil, nil)
	if !ok || len(compiled.Ddos) != 1 {
		t.Fatalf("expected 1 ddos policy, got %+v ok=%v", compiled.Ddos, ok)
	}

	compiledNoDdos, ok := CompileZone(z, "tenant-1", nil, nil, nil)
	if !ok || len(compiledNoDdos.Ddos) != 0 {
		t.Errorf("expected no ddos policies, got %+v", compiledNoDdos.Ddos)
	}
}

func TestCompileDdosPoliciesSortsByPriorityAndDropsDisabled(t *testing.T) {
	policies := []dataplane.Record{
		record("high-priority-num", nil, map[string]any{"sensitivity": "low", "action": "log", "requestRateThreshold": 100, "burstWindow": 60, "priority": 200, "enabled": true}),
		record("low-priority-num", nil, map[string]any{"sensitivity": "high", "action": "block", "requestRateThreshold": 10, "burstWindow": 60, "priority": 10, "enabled": true}),
		record("disabled", nil, map[string]any{"sensitivity": "high", "action": "block", "requestRateThreshold": 10, "burstWindow": 60, "priority": 1, "enabled": false}),
	}
	compiled := compileDdosPolicies(policies)
	if len(compiled) != 2 {
		t.Fatalf("expected the disabled policy to be dropped, got %d", len(compiled))
	}
	if compiled[0].ID != "low-priority-num" || compiled[1].ID != "high-priority-num" {
		t.Errorf("expected priority ascending order, got %+v", compiled)
	}
}

func TestCompileDdosReadsScopeFields(t *testing.T) {
	policy := record("policy-1", nil, map[string]any{
		"sensitivity": "high", "action": "block", "requestRateThreshold": 500, "burstWindow": 60,
		"priority": 50, "pathPrefix": "/login", "httpMethod": "POST", "enabled": true,
	})
	compiled, ok := compileDdos(policy)
	if !ok {
		t.Fatal("expected a compiled policy")
	}
	if compiled.PathPrefix == nil || *compiled.PathPrefix != "/login" {
		t.Errorf("expected pathPrefix /login, got %+v", compiled.PathPrefix)
	}
	if compiled.HttpMethod == nil || *compiled.HttpMethod != "POST" {
		t.Errorf("expected httpMethod POST, got %+v", compiled.HttpMethod)
	}
	if compiled.Priority != 50 {
		t.Errorf("expected priority 50, got %d", compiled.Priority)
	}
}

func TestCompileDdosEmptyScopeFieldsMeanUnscoped(t *testing.T) {
	policy := record("policy-1", nil, map[string]any{
		"sensitivity": "high", "action": "block", "requestRateThreshold": 500, "burstWindow": 60,
		"pathPrefix": "", "httpMethod": "", "enabled": true,
	})
	compiled, ok := compileDdos(policy)
	if !ok {
		t.Fatal("expected a compiled policy")
	}
	if compiled.PathPrefix != nil || compiled.HttpMethod != nil {
		t.Errorf("expected unscoped (nil) pathPrefix/httpMethod, got %+v / %+v", compiled.PathPrefix, compiled.HttpMethod)
	}
}

func accessListEntry(id, listType, value string, enabled bool) dataplane.Record {
	return record(id, nil, map[string]any{"type": listType, "value": value, "enabled": enabled})
}

func TestCompileIPAccessListWhitelistCompilesToAllow(t *testing.T) {
	entry := accessListEntry("wl-1", "whitelist", "10.0.0.5", true)
	compiled, ok := compileIPAccessList(entry)
	if !ok {
		t.Fatal("expected a compiled entry")
	}
	if compiled.Action != ruleset.ActionAllow || compiled.RuleType != "ipAccessList" {
		t.Errorf("unexpected compiled entry: %+v", compiled)
	}
	if compiled.MatchExpr.Kind != ruleset.MatchPredicate || compiled.MatchExpr.Predicate.Field != ruleset.FieldSourceIpCidr ||
		compiled.MatchExpr.Predicate.Op != ruleset.OpEq || *compiled.MatchExpr.Predicate.Value != "10.0.0.5" {
		t.Errorf("unexpected match expr: %+v", compiled.MatchExpr)
	}
}

func TestCompileIPAccessListBlacklistCompilesToBlock(t *testing.T) {
	entry := accessListEntry("bl-1", "blacklist", "10.0.0.0/24", true)
	compiled, ok := compileIPAccessList(entry)
	if !ok || compiled.Action != ruleset.ActionBlock {
		t.Errorf("expected block action, got %+v ok=%v", compiled, ok)
	}
}

func TestCompileIPAccessListDropsDisabledEntries(t *testing.T) {
	entry := accessListEntry("wl-1", "whitelist", "10.0.0.5", false)
	if _, ok := compileIPAccessList(entry); ok {
		t.Error("expected disabled entry to be dropped")
	}
}

func TestCompileZonePlacesIPAccessListsBeforeRegularRulesAllowBeforeBlock(t *testing.T) {
	z := zoneRecord(t, "active", nil)
	rules := []dataplane.Record{
		record("r-1", nil, map[string]any{"name": "regular", "ruleType": "waf", "action": "block", "priority": 1, "enabled": true}),
	}
	accessLists := []dataplane.Record{
		accessListEntry("bl-1", "blacklist", "10.0.0.1", true),
		accessListEntry("wl-1", "whitelist", "10.0.0.2", true),
	}
	compiled, ok := CompileZone(z, "tenant-1", nil, rules, accessLists)
	if !ok {
		t.Fatal("expected a compiled zone")
	}
	if len(compiled.Rules) != 3 {
		t.Fatalf("expected 3 rules, got %d", len(compiled.Rules))
	}
	// Whitelist (allow) first regardless of input order, then blacklist (block), then the
	// regular rule last regardless of its own (lower) priority number.
	if compiled.Rules[0].ID != "wl-1" || compiled.Rules[1].ID != "bl-1" || compiled.Rules[2].ID != "r-1" {
		t.Errorf("unexpected rule order: %+v", compiled.Rules)
	}
}
