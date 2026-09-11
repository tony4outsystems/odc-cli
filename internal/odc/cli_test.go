package odc

import (
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

const appKey = "11111111-1111-1111-1111-111111111111"
const envKey = "22222222-2222-2222-2222-222222222222"
const userKey = "33333333-3333-3333-3333-333333333333"
const roleKey = "44444444-4444-4444-4444-444444444444"

func TestAllCommands(t *testing.T) {
	dir := t.TempDir()
	apps := filepath.Join(dir, "apps.txt")
	if e := os.WriteFile(apps, []byte(appKey+"@4\n"), 0600); e != nil {
		t.Fatal(e)
	}
	cases := [][]string{
		{"list-deployed-apps", "--env", "Sandbox", "--search", "App"},
		{"list-revisions", "App"}, {"get-revision", "--app", "App", "--revision", "4"},
		{"analyze-deployment", "App", "--env", "Sandbox"}, {"analyze-deletion", "App"},
		{"discover"}, {"list-environments"}, {"list-apps", "--search", "App", "--type", "WebApplication"}, {"latest-revision", "--app", "App"}, {"get-app", "App"}, {"delete-app", "--app", "App"},
		{"get-user", "person@example.com"}, {"update-user", "person@example.com", "--name", "New Name", "--is-active", "false", "--photo-url", ""},
		{"grant-role", "person@example.com", "Creator"}, {"revoke-role", "person@example.com", "Creator", "--app", "App"},
		{"producer-graph", "App", "--env", "Sandbox", "--all-producers", "--output", filepath.Join(dir, "graphs", "app.mmd")},
		{"download-source-code", "App", "--revision", "4", "--output", filepath.Join(dir, "app.oml")},
		{"validate", "--app", "App", "--env", "Sandbox"}, {"deploy", "--app", "App", "--env", "Sandbox"},
		{"batch-deploy", apps, "--env", "Sandbox"}, {"batch-deploy", "--env", "Sandbox", apps, "--skip-dependencies"},
		{"batch-undeploy", apps, "--env", "Sandbox", "--skip-dependencies"},
		{"batch-delete", apps, "--skip-dependencies"},
		{"dangerous-batch-undeploy-all", "--env", "Sandbox"},
	}
	for _, cmd := range []string{"internal-build", "internal-publish", "internal-deploy", "undeploy"} {
		for _, noWait := range []bool{false, true} {
			args := []string{cmd, "--app", "App", "--env", "Sandbox"}
			if cmd == "internal-deploy" {
				args = append(args, "--build-key", "build")
			}
			if noWait {
				args = append(args, "--no-wait")
			}
			cases = append(cases, args)
		}
	}
	covered := map[string]bool{}
	for _, args := range cases {
		t.Run(strings.Join(args[:1], " "), func(t *testing.T) {
			covered[args[0]] = true
			mutations, polls := 0, 0
			c := testClient(func(r *http.Request) (*http.Response, error) {
				p := r.URL.Path
				switch {
				case p == "/identity/.well-known/openid-configuration":
					return jsonResponse(200, `{"issuer":"test","token_endpoint":"https://tenant.example/token","scopes_supported":[]}`), nil
				case p == "/api/asset-repository/v1/assets":
					return jsonResponse(200, `{"results":[{"name":"App","assetKey":"`+appKey+`","assetType":"WebApplication"}]}`), nil
				case p == "/api/portfolios/v2/environments":
					return jsonResponse(200, `{"results":[{"name":"Sandbox","key":"`+envKey+`","stage":"Development"}]}`), nil
				case p == "/api/portfolios/v2/deployed-assets":
					return jsonResponse(200, `{"results":[{"key":"`+appKey+`","deployments":[{"name":"App"}]}]}`), nil
				case p == "/api/identity/v1/users":
					return jsonResponse(200, `{"results":[{"key":"`+userKey+`","email":"person@example.com","name":"Person"}]}`), nil
				case p == "/api/identity/v1/users/"+userKey:
					if r.Method == "PATCH" {
						mutations++
						var d object
						if e := json.NewDecoder(r.Body).Decode(&d); e != nil {
							t.Error(e)
						}
						if d["isActive"] != false || d["name"] != "New Name" || d["photoUrl"] != "" {
							t.Errorf("update payload=%v", d)
						}
						return jsonResponse(204, ""), nil
					}
					return jsonResponse(200, `{"key":"`+userKey+`"}`), nil
				case strings.HasSuffix(p, "/revisions"):
					return jsonResponse(200, `{"results":[{"revision":4}]}`), nil
				case strings.HasSuffix(p, "/revisions/4"):
					return jsonResponse(200, `{"revision":4}`), nil
				case strings.HasSuffix(p, "/revisions/4/source-code"):
					return jsonResponse(200, `{"sourceCodeBinaryUrl":"https://blob.example/file.oml","sourceCodeDigest":"`+userKey+`"}`), nil
				case p == "/file.oml":
					return &http.Response{StatusCode: 200, Header: make(http.Header), Body: io.NopCloser(strings.NewReader("oml-bytes"))}, nil
				case p == "/api/identity/v1/application-roles":
					return jsonResponse(200, `{"results":[{"key":"`+roleKey+`","name":"Creator","assetKey":"`+appKey+`"}]}`), nil
				case p == "/api/identity/v1/users/"+userKey+"/application-roles/"+roleKey:
					mutations++
					if r.Method != "POST" && r.Method != "DELETE" {
						t.Errorf("unexpected method %s for role grant/revoke", r.Method)
					}
					return jsonResponse(201, ""), nil
				case strings.HasSuffix(p, "-analyses"):
					return jsonResponse(201, `{"analysisKey":"analysis"}`), nil
				case strings.HasSuffix(p, "-analyses/analysis"):
					return jsonResponse(200, `{"processStatus":"Finished","report":{}}`), nil
				case strings.HasSuffix(p, "/latest-revision"):
					return jsonResponse(200, `{"revision":4}`), nil
				case strings.HasSuffix(p, "/producer-graph"):
					return jsonResponse(200, `{"results":[]}`), nil
				case p == "/api/asset-repository/v1/assets/"+appKey:
					if r.Method == "DELETE" {
						mutations++
						return jsonResponse(204, ""), nil
					}
					return jsonResponse(200, `{"assetKey":"`+appKey+`","name":"App","revision":3,"assetType":"WebApplication"}`), nil
				case strings.HasSuffix(p, "-operations") && r.Method == "POST":
					mutations++
					return jsonResponse(200, `{"buildKey":"build","key":"operation"}`), nil
				case strings.Contains(p, "-operations/"):
					polls++
					return jsonResponse(200, `{"status":"Finished"}`), nil
				}
				return nil, fmt.Errorf("unexpected request %s %s", r.Method, r.URL)
			})
			c.token = "test-token"
			cmd, o, pos, e := parseArgs(args)
			if e != nil {
				t.Fatal(e)
			}
			if e = execute(c, cmd, o, pos); e != nil {
				t.Fatal(e)
			}
			if member(cmd, "deploy", "batch-deploy") && (mutations != 2 || polls != 2) {
				t.Errorf("deploy: mutations=%d polls=%d", mutations, polls)
			}
			if o.NoWait && (mutations != 1 || polls != 0) {
				t.Errorf("no-wait: mutations=%d polls=%d", mutations, polls)
			}
			if cmd == "producer-graph" {
				data, e := os.ReadFile(o.Output)
				if e != nil || !strings.Contains(string(data), "flowchart LR") {
					t.Fatalf("graph: %s %v", data, e)
				}
			}
			if cmd == "download-source-code" {
				data, e := os.ReadFile(o.Output)
				if e != nil || string(data) != "oml-bytes" {
					t.Fatalf("source code: %s %v", data, e)
				}
			}
		})
	}
	for _, cmd := range commands {
		if cmd == "login" {
			// Login saves local settings rather than calling execute; covered in login_test.go.
			continue
		}
		if !covered[cmd] {
			t.Errorf("command not covered: %s", cmd)
		}
	}
}
func TestCLIValidationAndHelp(t *testing.T) {
	for _, args := range [][]string{{}, {"unknown"}, {"deploy"}, {"internal-deploy", "--app", "a", "--env", "e"}, {"list-apps", "--type", "wrong"}, {"get-user"}, {"update-user", "person"}, {"update-user", "person", "--is-active", "wrong"}, {"deploy", "--app", "a", "--env", "e", "--timeout", "NaN"}, {"producer-graph", "app", "--revision", "x"}, {"discover", "extra"}, {"discover", "--unknown"}} {
		if _, _, _, e := parseArgs(args); e == nil {
			t.Errorf("accepted %v", args)
		}
	}
	for _, cmd := range commands {
		if e := Run([]string{cmd, "--help"}); e != nil {
			t.Errorf("%s help: %v", cmd, e)
		}
	}
	if e := Run([]string{"--help"}); e != nil {
		t.Fatal(e)
	}
	_, o, pos, e := parseArgs([]string{"update-user", "person", "--name", "", "--is-active=false"})
	if e != nil || len(pos) != 1 || o.Updates["name"] != "" || o.Updates["isActive"] != false {
		t.Fatalf("optional values lost: %v %v %v", o, pos, e)
	}
}
func TestResolution(t *testing.T) {
	c := testClient(func(r *http.Request) (*http.Response, error) {
		return jsonResponse(200, `{"results":[{"key":"sandbox","name":"Sandbox"},{"key":"production","name":"Production"}]}`), nil
	})
	c.token = "test-token"
	c.apps = []object{{"assetKey": "a", "name": "App"}, {"assetKey": "b", "name": "Apple"}}
	if key, e := c.Resolve("app", "app"); e != nil || key != "a" {
		t.Fatalf("exact: %q %v", key, e)
	}
	if _, e := c.Resolve("Ap", "app"); e == nil || !strings.Contains(e.Error(), "Did you mean") {
		t.Fatalf("partial: %v", e)
	}
	c.apps = append(c.apps, object{"assetKey": "c", "name": "APP"})
	if _, e := c.Resolve("app", "app"); e == nil || !strings.Contains(e.Error(), "ambiguous") {
		t.Fatalf("ambiguous: %v", e)
	}
	if key, e := c.Resolve("Sand", "environment"); e != nil || key != "sandbox" {
		t.Fatalf("environment partial: %q %v", key, e)
	}
	if key, e := c.Resolve(appKey, "app"); e != nil || key != appKey {
		t.Fatalf("UUID: %q %v", key, e)
	}
}

func TestCobraArgumentParsing(t *testing.T) {
	for _, args := range [][]string{
		{"--json", "--color", "never", "get-app", "App"},
		{"get-app", "--json", "App", "--color=never"},
		{"get-app", "App", "--color", "never", "--json"},
	} {
		cmd, o, pos, err := parseArgs(args)
		if err != nil || cmd != "get-app" || o.App != "App" || !o.JSON || o.Color != "never" || len(pos) != 1 {
			t.Fatalf("parseArgs(%v) = %q, %+v, %v, %v", args, cmd, o, pos, err)
		}
	}
	_, o, pos, err := parseArgs([]string{"get-app", "--", "--json"})
	if err != nil || o.App != "--json" || o.JSON || len(pos) != 1 {
		t.Fatalf("flag terminator: %+v, %v, %v", o, pos, err)
	}
	_, o, _, err = parseArgs([]string{"list-apps"})
	if err != nil || o.JSON || o.Color != "auto" {
		t.Fatalf("flags leaked between invocations: %+v, %v", o, err)
	}
	for _, args := range [][]string{
		{"get-app", "App", "--env", "Sandbox"},
		{"get-app", "App", "extra"},
		{"get-app", "App", "--app"},
	} {
		if _, _, _, err := parseArgs(args); err == nil {
			t.Errorf("accepted invalid args %v", args)
		}
	}
}

func TestCobraHelpAndCompletion(t *testing.T) {
	for _, args := range [][]string{
		{"help"},
		{"help", "deploy"},
		{"deploy", "-h"},
		{"completion", "--help"},
		{"completion", "bash"},
	} {
		// Help and completion must not select an API command or load credentials.
		cmd, _, _, err := parseArgs(args)
		if err != nil || cmd != "" {
			t.Errorf("parseArgs(%v): command=%q, error=%v", args, cmd, err)
		}
	}
}
