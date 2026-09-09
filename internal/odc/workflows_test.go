package odc

import (
	"fmt"
	"net/http"
	"os"
	"path/filepath"
	"reflect"
	"strings"
	"sync/atomic"
	"testing"
	"time"
)

func TestWaitFor(t *testing.T) {
	for _, status := range []string{"Finished", "FinishedWithErrors", "Deleted", "ToBeDeleted"} {
		n := 0
		d, e := WaitFor("build", func() (object, error) {
			n++
			if n == 1 {
				return object{"status": "Running"}, nil
			}
			return object{"status": status}, nil
		}, true, 0, time.Second)
		if e != nil || d["status"] != status || n != 2 {
			t.Fatalf("status=%s d=%v err=%v polls=%d", status, d, e, n)
		}
	}
	if _, e := WaitFor("deployment", func() (object, error) { return object{"status": "Running"}, nil }, false, 0, time.Millisecond); e == nil || !strings.Contains(e.Error(), "Timed out") {
		t.Fatalf("expected timeout, got %v", e)
	}
	expected := fmt.Errorf("network failure")
	if _, e := WaitFor("deployment", func() (object, error) { return nil, expected }, false, 0, time.Second); e != expected {
		t.Fatalf("lost error: %v", e)
	}
}
func TestReadAppsFile(t *testing.T) {
	path := filepath.Join(t.TempDir(), "apps.txt")
	for _, tt := range []struct {
		content string
		valid   bool
	}{{"# comment\n\n App One \nApp Two @ 12\n", true}, {"# empty", false}, {"app@abc", false}, {"@1", false}, {"app@0", false}, {"app@2@3", false}} {
		if e := os.WriteFile(path, []byte(tt.content), 0600); e != nil {
			t.Fatal(e)
		}
		apps, e := ReadAppsFile(path)
		if (e == nil) != tt.valid {
			t.Fatalf("%q: %v", tt.content, e)
		}
		if tt.valid && (len(apps) != 2 || apps[0].Key != "App One" || apps[0].Revision != nil || *apps[1].Revision != 12) {
			t.Fatalf("wrong apps: %v", apps)
		}
	}
}
func TestDependencyLevels(t *testing.T) {
	n := 3
	rev := map[string]*int{"a": &n, "b": &n, "c": &n, "shared": &n}
	deps := map[string]map[string]bool{"a": {"b": true, "c": true}, "b": {"shared": true}, "c": {"shared": true}, "shared": {}}
	plan, e := dependencyLevels(rev, deps)
	if e != nil {
		t.Fatal(e)
	}
	keys := [][]string{}
	for _, level := range plan {
		row := []string{}
		for _, app := range level {
			row = append(row, app.Key)
			if *app.Revision != 3 {
				t.Fatal("revision lost")
			}
		}
		keys = append(keys, row)
	}
	if !reflect.DeepEqual(keys, [][]string{{"shared"}, {"b", "c"}, {"a"}}) {
		t.Fatalf("plan: %v", keys)
	}
	deps["shared"]["a"] = true
	if _, e = dependencyLevels(rev, deps); e == nil || !strings.Contains(e.Error(), "cycle") {
		t.Fatalf("cycle: %v", e)
	}
}
func TestParallelBoundAndStopOnFailure(t *testing.T) {
	apps := make([]App, 10)
	var active, peak atomic.Int32
	results := runParallel(apps, Options{MaxParallel: 3}, func(App) object {
		n := active.Add(1)
		for {
			old := peak.Load()
			if n <= old || peak.CompareAndSwap(old, n) {
				break
			}
		}
		time.Sleep(time.Millisecond)
		active.Add(-1)
		return object{"status": "success"}
	})
	if len(results) != 10 || peak.Load() > 3 || peak.Load() < 2 {
		t.Fatalf("results=%d peak=%d", len(results), peak.Load())
	}
	results = runParallel(apps, Options{MaxParallel: 1}, func(App) object { return object{"status": "failed"} })
	if len(results) != 1 {
		t.Fatalf("did not stop: %d", len(results))
	}
	results = runParallel(apps, Options{MaxParallel: 1, ContinueOnError: true}, func(App) object { return object{"status": "failed"} })
	if len(results) != 10 {
		t.Fatalf("did not continue: %d", len(results))
	}
}
func TestDependencyPlanPinsAndSkipsPlatformApps(t *testing.T) {
	c := testClient(func(r *http.Request) (*http.Response, error) {
		if strings.Contains(r.URL.Path, "producer-graph") {
			if !strings.Contains(r.URL.Path, "/revisions/9/") || r.URL.Query().Get("producerTypeFilter") != "All" {
				t.Errorf("wrong graph request %s", r.URL)
			}
			return jsonResponse(200, `{"results":[{"key":"dep","revision":4},{"key":"platform","revision":1}]}`), nil
		}
		return nil, fmt.Errorf("unexpected request: %s", r.URL)
	})
	c.token = "test-token"
	c.apps = []object{{"name": "App", "assetKey": "app"}, {"assetKey": "dep"}, {"assetKey": "platform", "createdBy": "00000000-0000-0000-0000-000000000000"}}
	n := 9
	plan, e := c.DependencyPlan([]App{{"App", &n}}, "env")
	if e != nil {
		t.Fatal(e)
	}
	if len(plan) != 2 || plan[0][0].Key != "dep" || *plan[0][0].Revision != 4 || plan[1][0].Key != "app" || *plan[1][0].Revision != 9 {
		t.Fatalf("plan=%v", plan)
	}
	c.apps = append(c.apps, object{"name": "Dependency", "assetKey": "dep"})
	conflict := 8
	if _, e = c.DependencyPlan([]App{{"App", &n}, {"Dependency", &conflict}}, "env"); e == nil || !strings.Contains(e.Error(), "Conflicting revisions") {
		t.Fatalf("expected revision conflict: %v", e)
	}
}
func TestFailedBuildNeverDeploys(t *testing.T) {
	var deployed bool
	c := testClient(func(r *http.Request) (*http.Response, error) {
		switch {
		case strings.Contains(r.URL.Path, "/deployment-operations"):
			deployed = true
			return jsonResponse(200, `{"key":"deployment"}`), nil
		case r.URL.Path == "/api/builds/v1/build-operations/build":
			return jsonResponse(200, `{"status":"FinishedWithErrors"}`), nil
		case r.URL.Path == "/api/builds/v1/build-operations":
			return jsonResponse(200, `{"buildKey":"build"}`), nil
		case strings.Contains(r.URL.Path, "producer-graph"):
			return jsonResponse(200, `{"results":[]}`), nil
		case strings.Contains(r.URL.Path, "/environments"):
			return jsonResponse(200, `{"results":[{"key":"`+envKey+`"}]}`), nil
		default:
			return jsonResponse(200, `{"revision":3}`), nil
		}
	})
	c.token = "test-token"
	_, e := c.DeployApp(appKey, envKey, nil, Options{BuildType: "Release", Timeout: time.Second})
	if e == nil || deployed {
		t.Fatalf("failed build: deployed=%v error=%v", deployed, e)
	}
}
