package odc

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestDotEnv(t *testing.T) {
	for _, key := range []string{"ODC_TEST_A", "ODC_TEST_B", "ODC_TEST_C", "ODC_TEST_D", "ODC_TEST_E", "ODC_TEST_F"} {
		old, exists := os.LookupEnv(key)
		os.Unsetenv(key)
		t.Cleanup(func() {
			if exists {
				os.Setenv(key, old)
			} else {
				os.Unsetenv(key)
			}
		})
	}
	t.Setenv("ODC_TEST_A", "existing")
	path := filepath.Join(t.TempDir(), ".env")
	content := "export ODC_TEST_A=ignored\nODC_TEST_B='secret # and = signs'\nODC_TEST_C=plain # comment\nODC_TEST_D=\"line\\n${ODC_TEST_A}\"\nODC_TEST_E=${ODC_TEST_MISSING:-fallback}\nODC_TEST_F='two\nlines'\n"
	if e := os.WriteFile(path, []byte(content), 0600); e != nil {
		t.Fatal(e)
	}
	if e := loadDotEnv(path); e != nil {
		t.Fatal(e)
	}
	for key, want := range map[string]string{"ODC_TEST_A": "existing", "ODC_TEST_B": "secret # and = signs", "ODC_TEST_C": "plain", "ODC_TEST_D": "line\nexisting", "ODC_TEST_E": "fallback", "ODC_TEST_F": "two\nlines"} {
		if got := os.Getenv(key); got != want {
			t.Errorf("%s=%q, want %q", key, got, want)
		}
	}
}
func TestLoadSettings(t *testing.T) {
	t.Setenv("ODC_TENANT_URL", "https://tenant.example/")
	t.Setenv("ODC_CLIENT_ID", "id")
	t.Setenv("ODC_CLIENT_SECRET", "secret")
	s, e := LoadSettings()
	if e != nil || s.TenantOrigin() != "https://tenant.example" {
		t.Fatalf("settings=%v error=%v", s, e)
	}
	t.Setenv("ODC_CLIENT_SECRET", "")
	if _, e = LoadSettings(); e == nil || !strings.Contains(e.Error(), "ODC_CLIENT_SECRET") {
		t.Fatalf("missing credentials error: %v", e)
	}
}
func TestMermaidDeterministicAndDeduplicated(t *testing.T) {
	root := object{"key": "a", "name": `App "One"`, "revision": 1}
	child := object{"key": "b", "name": "Library", "revision": 2}
	g := RenderProducerGraph(root, []object{child, child})
	if strings.Count(g, " --> ") != 1 || !strings.Contains(g, `App \"One\"`) {
		t.Fatalf("graph: %s", g)
	}
	if g != RenderProducerGraph(root, []object{child}) {
		t.Fatal("duplicate changes graph")
	}
	if defaultMermaidPath("a/b", 3) != "producer-graph-a_b-rev-3.mmd" {
		t.Fatal("unsafe output path")
	}
}
