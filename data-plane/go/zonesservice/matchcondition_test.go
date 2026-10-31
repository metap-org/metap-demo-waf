package zonesservice

import (
	"encoding/json"
	"testing"
)

func mustJSON(t *testing.T, v string) json.RawMessage {
	t.Helper()
	return json.RawMessage(v)
}

func TestNoneAndNullAreAlwaysValid(t *testing.T) {
	if !IsValidMatchCondition(nil) {
		t.Error("nil should be valid")
	}
	if !IsValidMatchCondition(mustJSON(t, "null")) {
		t.Error("null should be valid")
	}
}

func TestASimplePredicateIsValid(t *testing.T) {
	raw := mustJSON(t, `{"field":"uri.path","op":"contains","value":"/admin"}`)
	if !IsValidMatchCondition(raw) {
		t.Error("expected valid")
	}
}

func TestAnUnknownFieldOrOpIsInvalid(t *testing.T) {
	if IsValidMatchCondition(mustJSON(t, `{"field":"bogus","op":"eq","value":"x"}`)) {
		t.Error("expected invalid field to be rejected")
	}
	if IsValidMatchCondition(mustJSON(t, `{"field":"method","op":"bogus","value":"x"}`)) {
		t.Error("expected invalid op to be rejected")
	}
}

func TestHeaderFieldRequiresAParamOrHeaderName(t *testing.T) {
	if IsValidMatchCondition(mustJSON(t, `{"field":"header","op":"eq","value":"1"}`)) {
		t.Error("expected missing param/header to be rejected")
	}
	if !IsValidMatchCondition(mustJSON(t, `{"field":"header","op":"eq","value":"1","param":"x-api-key"}`)) {
		t.Error("expected valid with param")
	}
}

func TestInAndNotInRequireNonEmptyValues(t *testing.T) {
	if IsValidMatchCondition(mustJSON(t, `{"field":"sourceIp","op":"in","values":[]}`)) {
		t.Error("expected empty values to be rejected")
	}
	if !IsValidMatchCondition(mustJSON(t, `{"field":"sourceIp","op":"in","values":["1.2.3.4"]}`)) {
		t.Error("expected non-empty values to be valid")
	}
}

func TestRegexOpRequiresAPatternThatActuallyCompiles(t *testing.T) {
	if !IsValidMatchCondition(mustJSON(t, `{"field":"uri.path","op":"regex","value":"^/admin/[0-9]+$"}`)) {
		t.Error("expected a compiling regex to be valid")
	}
	if IsValidMatchCondition(mustJSON(t, `{"field":"uri.path","op":"regex","value":"[invalid(regex"}`)) {
		t.Error("expected a non-compiling regex to be rejected")
	}
}

func TestAllAnyNotRecurseAndRequireEveryChildValid(t *testing.T) {
	valid := mustJSON(t, `{"all":[{"field":"method","op":"eq","value":"POST"},{"not":{"field":"sourceIp","op":"eq","value":"10.0.0.1"}}]}`)
	if !IsValidMatchCondition(valid) {
		t.Error("expected valid nested condition")
	}

	invalid := mustJSON(t, `{"any":[{"field":"method","op":"eq","value":"POST"},{"field":"bogus","op":"eq","value":"x"}]}`)
	if IsValidMatchCondition(invalid) {
		t.Error("expected invalid nested condition to be rejected")
	}
}
