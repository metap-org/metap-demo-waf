package zonesservice

import "testing"

func TestApexDomainOfABareApexHostnameIsUnchanged(t *testing.T) {
	if got := ApexDomain("example.com"); got != "example.com" {
		t.Errorf("got %q", got)
	}
}

func TestApexDomainOfASubdomainReducesToItsApex(t *testing.T) {
	if got := ApexDomain("shop.example.com"); got != "example.com" {
		t.Errorf("got %q", got)
	}
	if got := ApexDomain("api.shop.example.com"); got != "example.com" {
		t.Errorf("got %q", got)
	}
}

func TestApexDomainOfASingleLabelHostnameIsUnchanged(t *testing.T) {
	if got := ApexDomain("localhost"); got != "localhost" {
		t.Errorf("got %q", got)
	}
}

func TestApexDomainOfACompoundTLDIsNotSpeciallyHandledAcceptedV1Limitation(t *testing.T) {
	// Groups on "co.uk", not the real apex "example.co.uk" — documented limitation, not a bug.
	if got := ApexDomain("shop.example.co.uk"); got != "co.uk" {
		t.Errorf("got %q", got)
	}
}
