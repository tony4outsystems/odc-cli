package odc

import (
	"encoding/json"
	"errors"
	"os"
	"path/filepath"
	"testing"
)

func isolateSettings(t *testing.T) {
	t.Helper()
	t.Setenv("HOME", t.TempDir())
	for _, key := range []string{"ODC_TENANT_URL", "ODC_CLIENT_ID", "ODC_CLIENT_SECRET"} {
		t.Setenv(key, "")
		os.Unsetenv(key)
	}
	cwd, err := os.Getwd()
	if err != nil {
		t.Fatal(err)
	}
	dir := t.TempDir()
	if err = os.Chdir(dir); err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { os.Chdir(cwd) })
}

func TestLoginSaveAndFallback(t *testing.T) {
	isolateSettings(t)
	secret := "secret \" # ${literal} \\"
	if err := login("https://tenant.example", "client", func() (string, error) { return secret, nil }); err != nil {
		t.Fatal(err)
	}
	path, _ := configPath()
	info, err := os.Stat(path)
	if err != nil {
		t.Fatal(err)
	}
	if info.Mode().Perm() != 0600 {
		t.Fatalf("permissions: %v", info.Mode())
	}
	data, _ := os.ReadFile(path)
	var saved Settings
	if err = json.Unmarshal(data, &saved); err != nil {
		t.Fatal(err)
	}
	if saved.ClientSecret != secret {
		t.Fatal("secret changed")
	}
	t.Setenv("ODC_CLIENT_ID", "override")
	got, err := LoadSettings()
	if err != nil {
		t.Fatal(err)
	}
	if got.TenantURL != saved.TenantURL || got.ClientID != "override" || got.ClientSecret != secret {
		t.Fatal("incorrect configuration fallback")
	}
}

func TestDotEnvPreventsConfigFallback(t *testing.T) {
	isolateSettings(t)
	path, err := saveSettings(Settings{"https://saved.example", "saved", "secret"})
	if err != nil {
		t.Fatal(err)
	}
	// Even malformed home configuration must be ignored when a parent .env exists.
	if err = os.WriteFile(path, []byte("{invalid"), 0600); err != nil {
		t.Fatal(err)
	}
	if err = os.WriteFile(".env", []byte("ODC_TENANT_URL=https://env.example\nODC_CLIENT_ID=env\n"), 0600); err != nil {
		t.Fatal(err)
	}
	if err = os.Mkdir("child", 0700); err != nil {
		t.Fatal(err)
	}
	if err = os.Chdir("child"); err != nil {
		t.Fatal(err)
	}
	if _, err = LoadSettings(); err == nil {
		t.Fatal("incomplete .env should fail without fallback")
	}
	if os.Getenv("ODC_CLIENT_ID") != "env" {
		t.Fatal("parent .env not loaded")
	}
}

func TestLoginInvalidInputPreservesConfiguration(t *testing.T) {
	isolateSettings(t)
	path, err := saveSettings(Settings{"https://original.example", "original", "original"})
	if err != nil {
		t.Fatal(err)
	}
	before, _ := os.ReadFile(path)
	for _, tc := range []struct {
		url, id, secret string
		err             error
	}{
		{"invalid", "id", "secret", nil},
		{"https://tenant.example", "", "secret", nil},
		{"https://tenant.example", "id", "", nil},
		{"https://tenant.example", "id", "", errors.New("prompt failed")},
	} {
		if err = login(tc.url, tc.id, func() (string, error) { return tc.secret, tc.err }); err == nil {
			t.Fatal("expected error")
		}
	}
	after, _ := os.ReadFile(path)
	if string(before) != string(after) {
		t.Fatal("configuration modified on failure")
	}
	files, _ := filepath.Glob(filepath.Join(filepath.Dir(path), ".config-*"))
	if len(files) != 0 {
		t.Fatal("temporary files left behind")
	}
}

func TestLoginArgs(t *testing.T) {
	for _, args := range [][]string{{"login"}, {"login", "https://tenant.example"}, {"login", "url", "id", "extra"}} {
		if _, _, _, err := parseArgs(args); err == nil {
			t.Fatal("expected argument error")
		}
	}
	cmd, _, pos, err := parseArgs([]string{"login", "https://tenant.example", "id"})
	if err != nil || cmd != "login" || len(pos) != 2 {
		t.Fatalf("parse: %s %v %v", cmd, pos, err)
	}
}
