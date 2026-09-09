package odc

import (
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"net/url"
	"strings"
	"sync"
	"sync/atomic"
	"testing"
	"time"
)

type transportFunc func(*http.Request) (*http.Response, error)

func (f transportFunc) RoundTrip(r *http.Request) (*http.Response, error) { return f(r) }
func jsonResponse(status int, body string) *http.Response {
	return &http.Response{StatusCode: status, Header: make(http.Header), Body: io.NopCloser(strings.NewReader(body))}
}
func testClient(fn transportFunc) *Client {
	c := NewClient(Settings{"https://tenant.example", "test-client", "test-secret"})
	c.HTTP.Transport = fn
	return c
}
func TestAuthenticationPaginationAndCache(t *testing.T) {
	var tokens, discoveries, pages atomic.Int32
	c := testClient(func(r *http.Request) (*http.Response, error) {
		switch r.URL.Path {
		case "/identity/.well-known/openid-configuration":
			discoveries.Add(1)
			return jsonResponse(200, `{"token_endpoint":"https://tenant.example/token"}`), nil
		case "/token":
			tokens.Add(1)
			body, _ := io.ReadAll(r.Body)
			v, _ := url.ParseQuery(string(body))
			if v.Get("grant_type") != "client_credentials" || v.Get("client_id") != "test-client" || v.Get("client_secret") != "test-secret" {
				t.Errorf("wrong credentials form: %v", v)
			}
			return jsonResponse(200, `{"access_token":"test-token"}`), nil
		case "/api/asset-repository/v1/assets":
			pages.Add(1)
			if r.Header.Get("Authorization") != "Bearer test-token" {
				t.Error("missing bearer token")
			}
			if r.URL.Query().Get("limit") != "100" {
				t.Error("wrong page size")
			}
			if r.URL.Query().Get("offset") == "0" {
				return jsonResponse(200, `{"results":[{"assetKey":"a"}],"page":{"nextPageOffset":1}}`), nil
			}
			return jsonResponse(200, `{"results":[{"assetKey":"b"}],"page":{"nextPageOffset":null}}`), nil
		}
		return nil, fmt.Errorf("unexpected URL %s", r.URL)
	})
	var wg sync.WaitGroup
	for i := 0; i < 8; i++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			assets, e := c.ListAssets()
			if e != nil || len(assets) != 2 {
				t.Errorf("ListAssets: %v, %v", assets, e)
			}
		}()
	}
	wg.Wait()
	if tokens.Load() != 1 || discoveries.Load() != 1 || pages.Load() != 2 {
		t.Fatalf("unexpected counts: tokens=%d discovery=%d pages=%d", tokens.Load(), discoveries.Load(), pages.Load())
	}
}
func TestRequestRetriesAndErrors(t *testing.T) {
	for _, code := range []int{200, 204, 403, 429} {
		t.Run(fmt.Sprint(code), func(t *testing.T) {
			attempts := 0
			c := testClient(func(r *http.Request) (*http.Response, error) {
				attempts++
				status := code
				if attempts < 3 {
					status = 429
				}
				body := `{"ok":true}`
				if status == 204 {
					body = ""
				}
				response := jsonResponse(status, body)
				response.Header.Set("Retry-After", "0")
				return response, nil
			})
			d, e := c.request("GET", "https://tenant.example/test", nil, nil, false, false)
			if code >= 400 {
				if e == nil || !strings.Contains(e.Error(), fmt.Sprint(code)) {
					t.Fatalf("expected HTTP error, got %v", e)
				}
			} else if e != nil || d == nil {
				t.Fatalf("unexpected %v, %v", d, e)
			}
			expected := 3
			if code == 429 {
				expected = 6
			}
			if attempts != expected {
				t.Errorf("attempts=%d, want %d", attempts, expected)
			}
		})
	}
	if retryDelay("0", 1) != 0 || retryDelay("bad", 2) != 4*time.Second || retryDelay("", 5) != 30*time.Second {
		t.Error("unexpected retry delay")
	}
}
func TestRequestRejectsMalformedResponses(t *testing.T) {
	for _, body := range []string{"not JSON", "null", "[]"} {
		c := testClient(func(*http.Request) (*http.Response, error) { return jsonResponse(200, body), nil })
		if _, e := c.request("GET", "https://tenant.example/test", nil, nil, false, false); e == nil {
			t.Errorf("accepted %q", body)
		}
	}
}
func TestLatestRevisionRequiresInteger(t *testing.T) {
	for _, value := range []string{`"3"`, `3.5`, `null`} {
		c := testClient(func(*http.Request) (*http.Response, error) { return jsonResponse(200, `{"revision":`+value+`}`), nil })
		c.token = "test-token"
		if _, e := c.LatestRevision("a"); e == nil {
			t.Errorf("accepted %s", value)
		}
	}
}
func TestMutationPayloads(t *testing.T) {
	tests := []struct {
		name, method, path string
		body               object
		run                func(*Client) (object, error)
	}{
		{"build", "POST", "/api/builds/v1/build-operations", object{"assetKey": "a", "assetRevision": 7, "buildType": "Debug"}, func(c *Client) (object, error) { return c.StartBuild("a", 7, "Debug") }},
		{"publish", "POST", "/api/deployments/v1/publish-operations", object{"operation": "Publish", "assetKey": "a", "revision": 7, "environmentKey": "e"}, func(c *Client) (object, error) { return c.Publish("a", 7, "e") }},
		{"deploy", "POST", "/api/deployments/v1/deployment-operations", object{"operation": "Deploy", "assetKey": "a", "revision": 7, "environmentKey": "e", "buildKey": "b"}, func(c *Client) (object, error) { return c.Deploy("a", 7, "b", "e") }},
		{"undeploy", "POST", "/api/deployments/v1/deployment-operations", object{"operation": "Undeploy", "assetKey": "a", "environmentKey": "e"}, func(c *Client) (object, error) { return c.Undeploy("a", "e") }},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			c := testClient(func(r *http.Request) (*http.Response, error) {
				if r.Method != tt.method || r.URL.Path != tt.path {
					t.Errorf("request %s %s", r.Method, r.URL.Path)
				}
				data, _ := io.ReadAll(r.Body)
				want, _ := json.Marshal(tt.body)
				if string(data) != string(want) {
					t.Errorf("body %s, want %s", data, want)
				}
				return jsonResponse(200, `{"key":"operation","buildKey":"build"}`), nil
			})
			c.token = "test-token"
			if _, e := tt.run(c); e != nil {
				t.Fatal(e)
			}
		})
	}
}
