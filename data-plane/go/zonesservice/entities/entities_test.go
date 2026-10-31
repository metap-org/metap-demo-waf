package entities

import (
	"testing"

	"github.com/metap-org/metap-go/metadata"
)

// TestOwnsExactlyItsOwnFiveEntities is a regression guard for RegisterAllSubmitted — catches an
// entities/*.go file that forgot its init() SubmitEntity call, a name collision, or another
// service's entity accidentally ending up registered here.
func TestOwnsExactlyItsOwnFiveEntities(t *testing.T) {
	registry := metadata.NewMetadataRegistry()
	if err := registry.RegisterAllSubmitted(); err != nil {
		t.Fatalf("RegisterAllSubmitted: %v", err)
	}
	if err := registry.ValidateReferences(); err != nil {
		t.Fatalf("ValidateReferences: %v", err)
	}

	var names []string
	for _, e := range registry.ListEntities() {
		names = append(names, e.Name)
	}
	expected := []string{"waf.domains", "waf.zones", "waf.ddos_policies", "waf.firewall_rules", "waf.ip_access_lists"}
	for _, want := range expected {
		found := false
		for _, n := range names {
			if n == want {
				found = true
				break
			}
		}
		if !found {
			t.Errorf("missing entity: %s", want)
		}
	}
	if len(names) != len(expected) {
		t.Errorf("unexpected entity count: %v", names)
	}
}
