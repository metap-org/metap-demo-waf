package regexcache

import "testing"

func TestCompilesAndCachesAValidPattern(t *testing.T) {
	first, ok := Compiled(`^/admin/\d+$`)
	if !ok {
		t.Fatal("expected a valid pattern to compile")
	}
	if !first.MatchString("/admin/42") {
		t.Error("expected /admin/42 to match")
	}
	if first.MatchString("/admin/abc") {
		t.Error("expected /admin/abc to not match")
	}
	// Second call must hit the cache (same *Regexp, not a fresh compile).
	second, ok := Compiled(`^/admin/\d+$`)
	if !ok || second != first {
		t.Error("expected the second call to return the cached *Regexp")
	}
}

func TestAnInvalidPatternReturnsFalseAndStaysFalse(t *testing.T) {
	if _, ok := Compiled("[invalid(regex"); ok {
		t.Error("expected an invalid pattern to not compile")
	}
	// Cached as a miss too — calling again must not panic or somehow succeed.
	if _, ok := Compiled("[invalid(regex"); ok {
		t.Error("expected the cached miss to still report not-ok")
	}
}
