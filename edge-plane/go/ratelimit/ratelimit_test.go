package ratelimit

import (
	"testing"
	"time"
)

func TestStaysUnderBudgetUntilThresholdIsCrossed(t *testing.T) {
	limiter := New()
	window := 60 * time.Second
	// threshold 3: the 4th hit in the window is the first one over budget.
	if limiter.Check("k", 3, window) {
		t.Error("expected hit 1 to be under budget")
	}
	if limiter.Check("k", 3, window) {
		t.Error("expected hit 2 to be under budget")
	}
	if limiter.Check("k", 3, window) {
		t.Error("expected hit 3 to be under budget")
	}
	if !limiter.Check("k", 3, window) {
		t.Error("expected hit 4 to be over budget")
	}
	if !limiter.Check("k", 3, window) {
		t.Error("expected to stay over budget for further hits in the same window")
	}
}

func TestThresholdZeroTripsOnTheFirstHit(t *testing.T) {
	limiter := New()
	if !limiter.Check("k", 0, 60*time.Second) {
		t.Error("expected threshold 0 to trip immediately")
	}
}

func TestDifferentKeysAreIndependent(t *testing.T) {
	limiter := New()
	window := 60 * time.Second
	if limiter.Check("a", 1, window) {
		t.Error("expected key a's first hit to be under budget")
	}
	if limiter.Check("b", 1, window) {
		t.Error("expected key b's first hit to be under budget, independent of a")
	}
	if !limiter.Check("a", 1, window) {
		t.Error("expected key a's second hit to be over budget")
	}
}

func TestWindowResetsAfterItElapses(t *testing.T) {
	limiter := New()
	window := 30 * time.Millisecond
	if limiter.Check("k", 1, window) {
		t.Error("expected first hit to be under budget")
	}
	if !limiter.Check("k", 1, window) {
		t.Error("expected to still be inside the window, over budget")
	}
	time.Sleep(60 * time.Millisecond)
	if limiter.Check("k", 1, window) {
		t.Error("expected a new window to reset the counter")
	}
}

func TestTrackedKeysCountsDistinctKeys(t *testing.T) {
	limiter := New()
	window := 60 * time.Second
	limiter.Check("a", 10, window)
	limiter.Check("b", 10, window)
	limiter.Check("a", 10, window) // repeat — must not add a second entry for "a".
	if got := limiter.TrackedKeys(); got != 2 {
		t.Errorf("expected 2 tracked keys, got %d", got)
	}
}

func TestRuleKeyAndDdosKeyHaveDisjointPrefixes(t *testing.T) {
	// Same zone/client, different rule — the two key spaces must never collide, or a firewall
	// rule's budget could be silently shared with the zone's DDoS budget.
	rule := RuleKey("zone-1", "rule-1", "1.2.3.4")
	ddos := DdosKey("zone-1", "policy-1", "1.2.3.4")
	if rule == ddos {
		t.Error("expected rule and ddos keys to differ")
	}
	if rule[:2] != "r:" {
		t.Errorf("expected rule key to start with r:, got %q", rule)
	}
	if ddos[:2] != "d:" {
		t.Errorf("expected ddos key to start with d:, got %q", ddos)
	}
}
