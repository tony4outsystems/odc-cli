package odc

import (
	"bytes"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"net/url"
	"strconv"
	"strings"
	"sync"
	"time"
)

type object = map[string]any

var apiPaths = map[string]string{
	"asset-repository": "/api/asset-repository/v1", "builds": "/api/builds/v1",
	"dependency-management": "/api/dependency-management/v1", "deployments": "/api/deployments/v1",
	"portfolios": "/api/portfolios/v2", "identity": "/api/identity/v1",
}

type Client struct {
	Settings  Settings
	HTTP      *http.Client
	authMu    sync.Mutex
	discovery object
	token     string
	assetsMu  sync.Mutex
	assets    []object
}

func NewClient(s Settings) *Client {
	return &Client{Settings: s, HTTP: &http.Client{Timeout: 60 * time.Second}}
}
func (c *Client) Discover() (object, error) {
	c.authMu.Lock()
	defer c.authMu.Unlock()
	return c.discoverLocked()
}
func (c *Client) discoverLocked() (object, error) {
	if c.discovery != nil {
		return c.discovery, nil
	}
	d, e := c.request("GET", c.Settings.TenantOrigin()+"/identity/.well-known/openid-configuration", nil, nil, false, false)
	if e == nil {
		c.discovery = d
	}
	return d, e
}
func (c *Client) Token() (string, error) {
	c.authMu.Lock()
	defer c.authMu.Unlock()
	if c.token != "" {
		return c.token, nil
	}
	d, e := c.discoverLocked()
	if e != nil {
		return "", e
	}
	endpoint, e := requireString(d["token_endpoint"], "Discovery token_endpoint")
	if e != nil {
		return "", e
	}
	d, e = c.request("POST", endpoint, object{"grant_type": "client_credentials", "client_id": c.Settings.ClientID, "client_secret": c.Settings.ClientSecret}, nil, false, true)
	if e != nil {
		return "", e
	}
	c.token, e = requireString(d["access_token"], "Token response access_token")
	return c.token, e
}
func (c *Client) call(method, api, path string, body object, query url.Values) (object, error) {
	return c.request(method, c.Settings.TenantOrigin()+apiPaths[api]+path, body, query, true, false)
}
func (c *Client) request(method, address string, body object, query url.Values, auth, form bool) (object, error) {
	var encoded []byte
	var err error
	if body != nil {
		if form {
			v := url.Values{}
			for k, x := range body {
				v.Set(k, fmt.Sprint(x))
			}
			encoded = []byte(v.Encode())
		} else {
			encoded, err = json.Marshal(body)
			if err != nil {
				return nil, err
			}
		}
	}
	u, err := url.Parse(address)
	if err != nil {
		return nil, err
	}
	q := u.Query()
	for k, v := range query {
		q[k] = v
	}
	u.RawQuery = q.Encode()
	token := ""
	if auth {
		token, err = c.Token()
		if err != nil {
			return nil, err
		}
	}
	for attempt := 1; attempt <= 6; attempt++ {
		req, e := http.NewRequest(method, u.String(), bytes.NewReader(encoded))
		if e != nil {
			return nil, e
		}
		req.Header.Set("Accept", "application/json")
		if auth {
			req.Header.Set("Authorization", "Bearer "+token)
		}
		if body != nil {
			if form {
				req.Header.Set("Content-Type", "application/x-www-form-urlencoded")
			} else {
				req.Header.Set("Content-Type", "application/json")
			}
		}
		response, e := c.HTTP.Do(req)
		if e != nil {
			return nil, e
		}
		data, e := io.ReadAll(response.Body)
		response.Body.Close()
		if e != nil {
			return nil, e
		}
		if response.StatusCode == 429 && attempt < 6 {
			time.Sleep(retryDelay(response.Header.Get("Retry-After"), attempt))
			continue
		}
		if response.StatusCode >= 400 {
			return nil, errorf("%s %s failed with %d: %s", method, u.String(), response.StatusCode, data)
		}
		if len(bytes.TrimSpace(data)) == 0 {
			return object{}, nil
		}
		var result object
		decoder := json.NewDecoder(bytes.NewReader(data))
		decoder.UseNumber()
		if e = decoder.Decode(&result); e != nil {
			return nil, errorf("Invalid JSON response from %s: %v", u.String(), e)
		}
		if result == nil {
			return nil, errorf("Expected JSON object from %s", u.String())
		}
		return result, nil
	}
	return nil, errorf("Request attempts exhausted")
}
func retryDelay(value string, attempt int) time.Duration {
	if n, e := strconv.ParseFloat(value, 64); e == nil && n >= 0 {
		return time.Duration(n * float64(time.Second))
	}
	if t, e := http.ParseTime(value); e == nil {
		return max(0, time.Until(t))
	}
	return time.Duration(min(30, 1<<attempt)) * time.Second
}
func str(v any) string { s, _ := v.(string); return s }
func integer(v any) (int, bool) {
	switch n := v.(type) {
	case int:
		return n, true
	case json.Number:
		i, e := strconv.Atoi(string(n))
		return i, e == nil
	case float64:
		i := int(n)
		return i, float64(i) == n
	}
	return 0, false
}
func objects(v any) []object {
	out := []object{}
	switch values := v.(type) {
	case []any:
		for _, x := range values {
			if m, ok := x.(map[string]any); ok {
				out = append(out, m)
			}
		}
	case []object:
		return values
	}
	return out
}
func first(values ...any) any {
	for _, v := range values {
		if v != nil {
			if s, ok := v.(string); !ok || s != "" {
				return v
			}
		}
	}
	return nil
}
func esc(s string) string { return url.PathEscape(s) }
func (c *Client) LatestRevision(key string) (int, error) {
	d, e := c.call("GET", "asset-repository", "/assets/"+esc(key)+"/latest-revision", nil, nil)
	if e != nil {
		return 0, e
	}
	n, ok := integer(d["revision"])
	if !ok {
		return 0, errorf("Latest revision response did not include an integer revision: %v", d)
	}
	return n, nil
}
func (c *Client) GetAsset(key string) (object, error) {
	return c.call("GET", "asset-repository", "/assets/"+esc(key), nil, nil)
}
func (c *Client) ListEnvironments() ([]object, error) {
	d, e := c.call("GET", "portfolios", "/environments", nil, nil)
	return objects(d["results"]), e
}
func (c *Client) GetEnvironment(key string) (object, error) {
	items, e := c.ListEnvironments()
	if e != nil {
		return nil, e
	}
	for _, item := range items {
		if str(item["key"]) == key {
			return item, nil
		}
	}
	return nil, errorf("Environment key was not found or is not visible: %s", key)
}
func (c *Client) paginated(api, path string, q url.Values) ([]object, error) {
	if q == nil {
		q = url.Values{}
	}
	out := []object{}
	offset := 0
	for {
		q.Set("limit", "100")
		q.Set("offset", strconv.Itoa(offset))
		d, e := c.call("GET", api, path, nil, q)
		if e != nil {
			return nil, e
		}
		out = append(out, objects(d["results"])...)
		page, _ := d["page"].(map[string]any)
		next, ok := integer(page["nextPageOffset"])
		total, hasTotal := integer(page["totalResults"])
		if !ok || next <= offset || (hasTotal && len(out) >= total) {
			break
		}
		offset = next
	}
	return out, nil
}
func (c *Client) ListAssets() ([]object, error) {
	c.assetsMu.Lock()
	defer c.assetsMu.Unlock()
	if c.assets != nil {
		return c.assets, nil
	}
	items, e := c.paginated("asset-repository", "/assets", nil)
	if e == nil {
		c.assets = items
	}
	return items, e
}
func (c *Client) ListDeployedAssets(env string) ([]object, error) {
	return c.paginated("portfolios", "/deployed-assets", url.Values{"environmentKey": {env}})
}
func (c *Client) IsPlatformProvided(key string) (bool, error) {
	items, e := c.ListAssets()
	if e != nil {
		return false, e
	}
	for _, item := range items {
		if str(item["assetKey"]) == key {
			return str(item["createdBy"]) == "00000000-0000-0000-0000-000000000000", nil
		}
	}
	return false, nil
}
func (c *Client) StartBuild(key string, revision int, kind string) (object, error) {
	return c.call("POST", "builds", "/build-operations", object{"assetKey": key, "assetRevision": revision, "buildType": kind}, nil)
}
func (c *Client) Publish(key string, revision int, env string) (object, error) {
	return c.call("POST", "deployments", "/publish-operations", object{"operation": "Publish", "assetKey": key, "revision": revision, "environmentKey": env}, nil)
}
func (c *Client) Deploy(key string, revision int, build, env string) (object, error) {
	return c.call("POST", "deployments", "/deployment-operations", object{"operation": "Deploy", "assetKey": key, "revision": revision, "buildKey": build, "environmentKey": env}, nil)
}
func (c *Client) Undeploy(key, env string) (object, error) {
	return c.call("POST", "deployments", "/deployment-operations", object{"operation": "Undeploy", "assetKey": key, "environmentKey": env}, nil)
}
func (c *Client) ProducerGraph(key string, revision int, env string, depth int, filter string) (object, error) {
	q := url.Values{"maxDepth": {strconv.Itoa(depth)}, "producerTypeFilter": {filter}, "sort": {"name"}}
	if env != "" {
		q.Set("environmentKey", env)
	}
	return c.call("GET", "dependency-management", fmt.Sprintf("/assets/%s/revisions/%d/producer-graph", esc(key), revision), nil, q)
}
func (c *Client) QueryUsers(input string) ([]object, error) {
	d, e := c.call("GET", "identity", "/users", nil, url.Values{"limit": {"100"}, "nameOrEmailContains": {input}})
	return objects(d["results"]), e
}
func contains(value any, query string) bool {
	return strings.Contains(strings.ToLower(str(value)), strings.ToLower(query))
}
