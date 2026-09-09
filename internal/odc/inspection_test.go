package odc

import (
	"encoding/json"
	"fmt"
	"net/http"
	"strings"
	"testing"
	"time"
)

func TestDeployedAppSearchAndEnvironment(t *testing.T) {
	items := []object{{"key": "ABC", "type": "WebApplication", "deployments": []any{
		object{"environmentKey": "prod", "name": "Portal", "revision": 3},
		object{"environmentKey": "dev", "name": "Different", "revision": 4},
	}}}
	for _, tc := range []struct {
		env, search string
		count       int
	}{
		{"prod", "", 1}, {"prod", "PORT", 1}, {"prod", "abc", 1}, {"prod", "Different", 0}, {"missing", "", 0},
	} {
		rows := deployedAppRows(items, tc.env, tc.search)
		if len(rows) != tc.count {
			t.Fatalf("%+v: %v", tc, rows)
		}
		if len(rows) > 0 && rows[0]["revision"] != 3 {
			t.Fatalf("wrong deployment: %v", rows)
		}
	}
}

func TestAnalysisLifecycle(t *testing.T) {
	for _, command := range []string{"analyze-deployment", "analyze-deletion"} {
		for _, scenario := range []string{"finished", "failed", "timeout", "no-wait", "unknown", "missing-key", "http-error"} {
			t.Run(command+"/"+scenario, func(t *testing.T) {
				polls, posts := 0, 0
				c := testClient(func(r *http.Request) (*http.Response, error) {
					path := "/api/dependency-management/v1/deletion-analyses"
					if command == "analyze-deployment" {
						path = "/api/dependency-management/v1/deployment-analyses"
					}
					if r.Method == "POST" && r.URL.Path == path {
						posts++
						var body object
						if err := json.NewDecoder(r.Body).Decode(&body); err != nil {
							t.Fatal(err)
						}
						if body["assetKey"] != appKey {
							t.Fatalf("API app key: %v", body)
						}
						if command == "analyze-deployment" {
							if body["revision"] != float64(7) || body["environmentKey"] != envKey || len(body) != 3 {
								t.Fatalf("payload: %v", body)
							}
						} else if len(body) != 1 {
							t.Fatalf("deletion payload: %v", body)
						}
						if scenario == "missing-key" {
							return jsonResponse(201, `{}`), nil
						}
						return jsonResponse(201, `{"analysisKey":"analysis"}`), nil
					}
					if r.Method != "GET" || r.URL.Path != path+"/analysis" {
						return nil, fmt.Errorf("unexpected request %s %s", r.Method, r.URL)
					}
					polls++
					switch scenario {
					case "failed":
						return jsonResponse(200, `{"processStatus":"Failed","error":{"detail":"failed"}}`), nil
					case "timeout":
						return jsonResponse(200, `{"processStatus":"InProgress"}`), nil
					case "unknown":
						return jsonResponse(200, `{"processStatus":"Unexpected"}`), nil
					case "http-error":
						return jsonResponse(403, `{}`), nil
					}
					if polls == 1 {
						return jsonResponse(200, `{"processStatus":"InProgress"}`), nil
					}
					return jsonResponse(200, `{"processStatus":"Finished","report":{"status":"ErrorsFound"}}`), nil
				})
				c.token = "test"
				revision := 7
				err := c.analyze(appKey, command, Options{Env: envKey, Revision: &revision, Timeout: time.Millisecond, NoWait: scenario == "no-wait"})
				wantErr := scenario != "finished" && scenario != "no-wait"
				if (err != nil) != wantErr {
					t.Fatalf("error=%v", err)
				}
				if posts != 1 || (scenario == "no-wait" && polls != 0) || (scenario == "finished" && polls != 2) {
					t.Fatalf("posts=%d polls=%d", posts, polls)
				}
			})
		}
	}
}

func TestRevisionPagination(t *testing.T) {
	pages := 0
	c := testClient(func(r *http.Request) (*http.Response, error) {
		if r.Method != "GET" || r.URL.Path != "/api/asset-repository/v1/assets/"+appKey+"/revisions" {
			t.Fatalf("request: %s %s", r.Method, r.URL)
		}
		pages++
		if pages == 1 {
			return jsonResponse(200, `{"results":[{"revision":1}],"page":{"nextPageOffset":1}}`), nil
		}
		if r.URL.Query().Get("offset") != "1" {
			t.Fatal(r.URL)
		}
		return jsonResponse(200, `{"results":[{"revision":2}],"page":{"nextPageOffset":null}}`), nil
	})
	c.token = "test"
	rows, err := c.ListRevisions(appKey)
	if err != nil || len(rows) != 2 || pages != 2 {
		t.Fatalf("%v %v pages=%d", rows, err, pages)
	}
}

func TestInspectionRequiredFlags(t *testing.T) {
	for _, args := range [][]string{
		{"list-deployed-apps"}, {"list-revisions"}, {"get-revision", "App"},
		{"get-revision", "App", "--revision", "0"}, {"analyze-deployment", "App"},
		{"analyze-deletion"}, {"get-app", "--asset", "App"},
	} {
		if _, _, _, err := parseArgs(args); err == nil {
			t.Fatalf("accepted %v", args)
		}
	}
	_, o, _, err := parseArgs([]string{"analyze-deletion", "--app", "App", "--no-wait"})
	if err != nil || !o.NoWait || !strings.EqualFold(o.App, "App") {
		t.Fatalf("%+v %v", o, err)
	}
}
