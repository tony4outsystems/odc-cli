package odc

import (
	"encoding/json"
	"fmt"
	"net/http"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

const assetKey = "11111111-1111-1111-1111-111111111111"
const envKey = "22222222-2222-2222-2222-222222222222"
const userKey = "33333333-3333-3333-3333-333333333333"

func TestAllCommands(t *testing.T) {
	dir := t.TempDir()
	apps := filepath.Join(dir, "apps.txt")
	if e := os.WriteFile(apps, []byte(assetKey+"@4\n"), 0600); e != nil {
		t.Fatal(e)
	}
	cases := [][]string{
		{"discover"}, {"list-environments"}, {"list-apps", "--search", "App", "--type", "WebApplication"}, {"latest-revision", "--asset", "App"}, {"get-app", "App"}, {"delete-app", "--asset", "App"},
		{"get-user", "person@example.com"}, {"update-user", "person@example.com", "--name", "New Name", "--is-active", "false", "--photo-url", ""},
		{"producer-graph", "App", "--env", "Sandbox", "--all-producers", "--output", filepath.Join(dir, "graphs", "app.mmd")},
		{"validate", "--asset", "App", "--env", "Sandbox"}, {"deploy", "--asset", "App", "--env", "Sandbox"},
		{"batch-deploy", apps, "--env", "Sandbox"}, {"batch-deploy", "--env", "Sandbox", apps, "--skip-dependencies"},
		{"dangerous-batch-undeploy-all", "--env", "Sandbox"},
	}
	for _, cmd := range []string{"internal-build", "internal-publish", "internal-deploy", "undeploy"} {
		for _, noWait := range []bool{false, true} {
			args := []string{cmd, "--asset", "App", "--env", "Sandbox"}
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
					return jsonResponse(200, `{"results":[{"name":"App","assetKey":"`+assetKey+`","assetType":"WebApplication"}]}`), nil
				case p == "/api/portfolios/v2/environments":
					return jsonResponse(200, `{"results":[{"name":"Sandbox","key":"`+envKey+`","stage":"Development"}]}`), nil
				case p == "/api/portfolios/v2/deployed-assets":
					return jsonResponse(200, `{"results":[{"key":"`+assetKey+`","deployments":[{"name":"App"}]}]}`), nil
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
				case strings.HasSuffix(p, "/latest-revision"):
					return jsonResponse(200, `{"revision":4}`), nil
				case strings.HasSuffix(p, "/producer-graph"):
					return jsonResponse(200, `{"results":[]}`), nil
				case p == "/api/asset-repository/v1/assets/"+assetKey:
					if r.Method == "DELETE" {
						mutations++
						return jsonResponse(204, ""), nil
					}
					return jsonResponse(200, `{"assetKey":"`+assetKey+`","name":"App","revision":3,"assetType":"WebApplication"}`), nil
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
		})
	}
	for _, cmd := range commands {
		if !covered[cmd] {
			t.Errorf("command not covered: %s", cmd)
		}
	}
}
func TestCLIValidationAndHelp(t *testing.T) {
	for _, args := range [][]string{{}, {"unknown"}, {"deploy"}, {"internal-deploy", "--asset", "a", "--env", "e"}, {"list-apps", "--type", "wrong"}, {"get-user"}, {"update-user", "person"}, {"update-user", "person", "--is-active", "wrong"}, {"deploy", "--asset", "a", "--env", "e", "--timeout", "NaN"}, {"producer-graph", "app", "--revision", "x"}, {"discover", "extra"}, {"discover", "--unknown"}} {
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
	c.assets = []object{{"assetKey": "a", "name": "App"}, {"assetKey": "b", "name": "Apple"}}
	if key, e := c.Resolve("app", "asset"); e != nil || key != "a" {
		t.Fatalf("exact: %q %v", key, e)
	}
	if _, e := c.Resolve("Ap", "asset"); e == nil || !strings.Contains(e.Error(), "Did you mean") {
		t.Fatalf("partial: %v", e)
	}
	c.assets = append(c.assets, object{"assetKey": "c", "name": "APP"})
	if _, e := c.Resolve("app", "asset"); e == nil || !strings.Contains(e.Error(), "ambiguous") {
		t.Fatalf("ambiguous: %v", e)
	}
	if key, e := c.Resolve("Sand", "environment"); e != nil || key != "sandbox" {
		t.Fatalf("environment partial: %q %v", key, e)
	}
	if key, e := c.Resolve(assetKey, "asset"); e != nil || key != assetKey {
		t.Fatalf("UUID: %q %v", key, e)
	}
}
