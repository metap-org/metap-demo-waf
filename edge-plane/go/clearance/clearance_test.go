package clearance

import (
	"strconv"
	"strings"
	"testing"
	"time"
)

// asCookieHeader: Issue() returns a full Set-Cookie header value; this pulls just the
// name=value pair out of it, i.e. what a browser would echo back on the next request's Cookie:
// header.
func asCookieHeader(setCookie string) string {
	parts := strings.Split(setCookie, ";")
	return parts[0]
}

func TestIssuedClearanceVerifiesForTheSameZoneAndIp(t *testing.T) {
	cookie := Issue("waf_clearance", "secret", "zone-1", "1.2.3.4", 60*time.Second)
	header := asCookieHeader(cookie)
	if !Verify(header, "waf_clearance", "secret", "zone-1", "1.2.3.4") {
		t.Error("expected the issued clearance to verify")
	}
}

func TestClearanceDoesNotTransferToADifferentZone(t *testing.T) {
	cookie := Issue("waf_clearance", "secret", "zone-1", "1.2.3.4", 60*time.Second)
	header := asCookieHeader(cookie)
	if Verify(header, "waf_clearance", "secret", "zone-2", "1.2.3.4") {
		t.Error("expected the clearance to not transfer to a different zone")
	}
}

func TestClearanceDoesNotTransferToADifferentIp(t *testing.T) {
	cookie := Issue("waf_clearance", "secret", "zone-1", "1.2.3.4", 60*time.Second)
	header := asCookieHeader(cookie)
	if Verify(header, "waf_clearance", "secret", "zone-1", "9.9.9.9") {
		t.Error("expected the clearance to not transfer to a different IP")
	}
}

func TestWrongSecretIsRejected(t *testing.T) {
	cookie := Issue("waf_clearance", "secret-a", "zone-1", "1.2.3.4", 60*time.Second)
	header := asCookieHeader(cookie)
	if Verify(header, "waf_clearance", "secret-b", "zone-1", "1.2.3.4") {
		t.Error("expected a wrong secret to be rejected")
	}
}

func TestTamperedExpiryIsRejectedBecauseExpiryIsInsideTheSignedInput(t *testing.T) {
	cookie := Issue("waf_clearance", "secret", "zone-1", "1.2.3.4", 60*time.Second)
	header := asCookieHeader(cookie)
	// Append a digit to the numeric expiry prefix (before the signature) to try to extend the
	// clearance's lifetime — the signature was computed over the original expiry, so this must
	// fail rather than being accepted with a longer expiry than was issued.
	lastDot := strings.LastIndex(header, ".")
	nameAndExpiry, signature := header[:lastDot], header[lastDot+1:]
	forged := nameAndExpiry + "9." + signature
	if Verify(forged, "waf_clearance", "secret", "zone-1", "1.2.3.4") {
		t.Error("expected a tampered expiry to be rejected")
	}
}

func TestExpiredClearanceIsRejected(t *testing.T) {
	// Built directly with sign() (same-package access) rather than sleeping past a real TTL —
	// an expiry timestamp already in the past, signed correctly, must still fail.
	past := nowSecs() - 10
	signature := sign("secret", "zone-1", "1.2.3.4", past)
	header := "waf_clearance=" + strconv.FormatUint(past, 10) + "." + signature
	if Verify(header, "waf_clearance", "secret", "zone-1", "1.2.3.4") {
		t.Error("expected an expired clearance to be rejected")
	}
}

func TestMissingCookieIsRejected(t *testing.T) {
	if Verify("other_cookie=xyz", "waf_clearance", "secret", "zone-1", "1.2.3.4") {
		t.Error("expected a missing cookie to be rejected")
	}
	if Verify("", "waf_clearance", "secret", "zone-1", "1.2.3.4") {
		t.Error("expected an empty header to be rejected")
	}
}

func TestMalformedCookieValueIsRejectedNotPanicking(t *testing.T) {
	if Verify("waf_clearance=not-a-valid-format", "waf_clearance", "secret", "zone-1", "1.2.3.4") {
		t.Error("expected a malformed value to be rejected")
	}
	if Verify("waf_clearance=123", "waf_clearance", "secret", "zone-1", "1.2.3.4") {
		t.Error("expected a value with no signature to be rejected")
	}
	if Verify("waf_clearance=abc.def", "waf_clearance", "secret", "zone-1", "1.2.3.4") {
		t.Error("expected a non-numeric expiry to be rejected")
	}
}

func TestCookieValueExtractsFromAMultiCookieHeader(t *testing.T) {
	header := "a=1; waf_clearance=999.abc; b=2"
	if v, ok := cookieValue(header, "waf_clearance"); !ok || v != "999.abc" {
		t.Errorf("expected 999.abc, got %q ok=%v", v, ok)
	}
	if _, ok := cookieValue(header, "missing"); ok {
		t.Error("expected a missing cookie name to report not-found")
	}
}

func TestIssuedCookieCarriesTheExpectedAttributes(t *testing.T) {
	cookie := Issue("waf_clearance", "secret", "zone-1", "1.2.3.4", 1800*time.Second)
	if !strings.Contains(cookie, "HttpOnly") {
		t.Error("expected HttpOnly")
	}
	if !strings.Contains(cookie, "SameSite=Lax") {
		t.Error("expected SameSite=Lax")
	}
	if !strings.Contains(cookie, "Path=/") {
		t.Error("expected Path=/")
	}
	if !strings.Contains(cookie, "Max-Age=1800") {
		t.Error("expected Max-Age=1800")
	}
}
