package entities

import (
	"testing"

	"github.com/metap-org/metap-go/metadata"
)

func TestOwnsExactlyItsOwnTwoEntities(t *testing.T) {
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
	for _, want := range []string{"waf.scan_jobs", "waf.scan_findings"} {
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
	if len(names) != 2 {
		t.Errorf("unexpected entity count: %v", names)
	}
}
