package entities

import (
	"testing"

	"github.com/metap-org/metap-go/metadata"
)

func TestOwnsExactlyItsOwnFourEntities(t *testing.T) {
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
	for _, want := range []string{"waf.security_events", "waf.incidents", "waf.alert_policies", "waf.alert_notifications"} {
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
	if len(names) != 4 {
		t.Errorf("unexpected entity count: %v", names)
	}
}
