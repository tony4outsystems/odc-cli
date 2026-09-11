package odc

import (
	"bufio"
	"fmt"
	"os"
	"sort"
	"strconv"
	"strings"
	"sync"
	"time"
)

type Options struct {
	JSON                                                           bool
	Color                                                          string
	App, Env, BuildType, BuildKey, Filter, Output, Search, AppType string
	Revision                                                       *int
	Interval, Timeout                                              time.Duration
	MaxParallel, MaxDepth                                          int
	NoWait, ContinueOnError, SkipDependencies, AllProducers        bool
	Updates                                                        object
}

func printlnLocked(format string, args ...any) {
	printMu.Lock()
	defer printMu.Unlock()
	w := os.Stdout
	if outputJSON {
		w = os.Stderr
	}
	fmt.Fprintf(w, format+"\n", args...)
}
func WaitFor(label string, fetch func() (object, error), build bool, interval, timeout time.Duration) (object, error) {
	deadline := time.Now().Add(timeout)
	last := ""
	for {
		details, e := fetch()
		if e != nil {
			return nil, e
		}
		status := str(details["status"])
		if status != last {
			printlnLocked("%s: %s", label, status)
			last = status
		}
		terminal := status == "Finished" || (!build && status == "FinishedWithError") || (build && (status == "FinishedWithErrors" || status == "Deleted" || status == "ToBeDeleted"))
		if terminal {
			return details, nil
		}
		remaining := time.Until(deadline)
		if remaining <= 0 {
			return nil, errorf("Timed out waiting for %s; last response: %v", label, details)
		}
		time.Sleep(min(interval, remaining))
	}
}
func (c *Client) Preflight(key, env string, revision *int) (int, error) {
	app, e := c.GetApp(key)
	if e != nil {
		return 0, e
	}
	environment, e := c.GetEnvironment(env)
	if e != nil {
		return 0, e
	}
	n, ok := integer(app["revision"])
	if revision != nil {
		n = *revision
	} else if !ok {
		n, e = c.LatestRevision(key)
		if e != nil {
			return 0, e
		}
	}
	e = PrintResult(object{"preflight": object{
		"app":         compactMap(app, []string{"assetKey", "name", "assetType", "revision", "tag", "portfolioKey", "createdAt", "createdBy"}),
		"environment": compactMap(environment, []string{"key", "name", "purpose", "defaultDomain", "region", "hosting", "status", "portfolioKey"}), "selectedRevision": n}})
	return n, e
}
func (c *Client) waitOperation(response object, kind string, o Options) (object, error) {
	keyField, api, path := "key", "deployments", "/deployment-operations/"
	if kind == "build" {
		keyField, api, path = "buildKey", "builds", "/build-operations/"
	} else if kind == "publish" {
		path = "/publish-operations/"
	}
	key, e := requireString(response[keyField], kind+" operation key")
	if e != nil {
		return nil, e
	}
	if o.NoWait {
		return response, nil
	}
	d, e := WaitFor(kind+" "+key, func() (object, error) { return c.call("GET", api, path+esc(key), nil, nil) }, kind == "build", o.Interval, o.Timeout)
	if e != nil {
		return nil, e
	}
	if d["status"] != "Finished" {
		return nil, errorf("%s did not finish successfully: %v", kind, d["status"])
	}
	return d, nil
}
func (c *Client) DeployApp(input, env string, revision *int, o Options) (object, error) {
	key, e := c.Resolve(input, "app")
	if e != nil {
		return nil, e
	}
	env, e = c.Resolve(env, "environment")
	if e != nil {
		return nil, e
	}
	rev, e := c.Preflight(key, env, revision)
	if e != nil {
		return nil, e
	}
	graph, e := c.ProducerGraph(key, rev, env, 0, "Deployable")
	if e != nil {
		return nil, e
	}
	producers := []object{}
	for _, p := range objects(graph["results"]) {
		pk, e := requireString(p["key"], "producer key")
		if e != nil {
			return nil, e
		}
		platform, e := c.IsPlatformProvided(pk)
		if e != nil {
			return nil, e
		}
		if !platform {
			producers = append(producers, p)
		}
	}
	printlnLocked("Dependencies (%d):", len(producers))
	for _, p := range producers {
		printlnLocked("  - %v (%v) - %v", first(p["name"], "Unknown"), first(p["type"], "Unknown"), first(p["status"], "Unknown"))
	}
	b, e := c.StartBuild(key, rev, o.BuildType)
	if e != nil {
		return nil, e
	}
	if e = PrintResult(object{"build_started": b}); e != nil {
		return nil, e
	}
	buildKey, e := requireString(b["buildKey"], "buildKey")
	if e != nil {
		return nil, e
	}
	build, e := c.waitOperation(b, "build", o)
	if e != nil {
		return nil, e
	}
	d, e := c.Deploy(key, rev, buildKey, env)
	if e != nil {
		return nil, e
	}
	if e = PrintResult(object{"deploy_started": d}); e != nil {
		return nil, e
	}
	deployment, e := c.waitOperation(d, "deployment", o)
	if e != nil {
		return nil, e
	}
	result := object{"assetKey": key, "environmentKey": env, "revision": rev, "build": build, "deployment": deployment}
	return result, PrintResult(result)
}

type App struct {
	Key      string
	Revision *int
}

func ReadAppsFile(path string) ([]App, error) {
	f, e := os.Open(path)
	if e != nil {
		return nil, errorf("Apps file: %v", e)
	}
	defer f.Close()
	apps := []App{}
	scanner := bufio.NewScanner(f)
	lineNumber := 0
	for scanner.Scan() {
		lineNumber++
		line := strings.TrimSpace(scanner.Text())
		if line == "" || strings.HasPrefix(line, "#") {
			continue
		}
		key, rev, pinned := strings.Cut(line, "@")
		key = strings.TrimSpace(key)
		if key == "" {
			return nil, errorf("Empty app at %s:%d", path, lineNumber)
		}
		app := App{Key: key}
		if pinned {
			n, e := strconv.Atoi(strings.TrimSpace(rev))
			if e != nil || n < 1 {
				return nil, errorf("Invalid revision %q for app %q in %s", rev, key, path)
			}
			app.Revision = &n
		}
		apps = append(apps, app)
	}
	if e = scanner.Err(); e != nil {
		return nil, e
	}
	if len(apps) == 0 {
		return nil, errorf("Apps file is empty: %s", path)
	}
	return apps, nil
}
func (c *Client) DependencyPlan(apps []App, env string) ([][]App, error) {
	revisions := map[string]*int{}
	deps := map[string]map[string]bool{}
	record := func(key string, rev *int) error {
		if deps[key] == nil {
			deps[key] = map[string]bool{}
		}
		if old := revisions[key]; old != nil && rev != nil && *old != *rev {
			return errorf("Conflicting revisions for app %s: %d and %d", key, *old, *rev)
		}
		if rev != nil {
			revisions[key] = rev
		}
		return nil
	}
	resolved := []App{}
	for _, app := range apps {
		key, e := c.Resolve(app.Key, "app")
		if e != nil {
			return nil, e
		}
		rev := app.Revision
		if rev == nil {
			n, e := c.LatestRevision(key)
			if e != nil {
				return nil, e
			}
			rev = &n
		}
		if e = record(key, rev); e != nil {
			return nil, e
		}
		resolved = append(resolved, App{key, rev})
	}
	var visit func(object) (string, error)
	visit = func(node object) (string, error) {
		key, e := requireString(node["key"], "producer key")
		if e != nil {
			return "", e
		}
		n, ok := integer(node["revision"])
		if !ok || n < 1 {
			return "", errorf("Invalid producer revision for %s", key)
		}
		if e = record(key, &n); e != nil {
			return "", e
		}
		for _, child := range objects(node["producers"]) {
			ck, e := requireString(child["key"], "producer key")
			if e != nil {
				return "", e
			}
			platform, e := c.IsPlatformProvided(ck)
			if e != nil {
				return "", e
			}
			if platform {
				continue
			}
			ck, e = visit(child)
			if e != nil {
				return "", e
			}
			deps[key][ck] = true
		}
		return key, nil
	}
	for _, app := range resolved {
		g, e := c.ProducerGraph(app.Key, *app.Revision, env, 0, "All")
		if e != nil {
			return nil, e
		}
		for _, p := range objects(g["results"]) {
			key, e := requireString(p["key"], "producer key")
			if e != nil {
				return nil, e
			}
			platform, e := c.IsPlatformProvided(key)
			if e != nil {
				return nil, e
			}
			if platform {
				continue
			}
			key, e = visit(p)
			if e != nil {
				return nil, e
			}
			deps[app.Key][key] = true
		}
	}
	return dependencyLevels(revisions, deps)
}
func dependencyLevels(revisions map[string]*int, deps map[string]map[string]bool) ([][]App, error) {
	levels := map[string]int{}
	visiting := map[string]bool{}
	var levelOf func(string) (int, error)
	levelOf = func(key string) (int, error) {
		if n, ok := levels[key]; ok {
			return n, nil
		}
		if visiting[key] {
			return 0, errorf("Dependency cycle detected at app %s", key)
		}
		visiting[key] = true
		level := 0
		for dep := range deps[key] {
			n, e := levelOf(dep)
			if e != nil {
				return 0, e
			}
			level = max(level, n+1)
		}
		visiting[key] = false
		levels[key] = level
		return level, nil
	}
	keys := []string{}
	for key := range deps {
		keys = append(keys, key)
	}
	sort.Strings(keys)
	plan := [][]App{}
	for _, key := range keys {
		n, e := levelOf(key)
		if e != nil {
			return nil, e
		}
		for len(plan) <= n {
			plan = append(plan, []App{})
		}
		plan[n] = append(plan[n], App{key, revisions[key]})
	}
	return plan, nil
}
func runParallel(apps []App, o Options, run func(App) object) []object {
	summary := []object{}
	if o.MaxParallel <= 1 {
		for _, app := range apps {
			entry := run(app)
			summary = append(summary, entry)
			if entry["status"] == "failed" && !o.ContinueOnError {
				break
			}
		}
		return summary
	}
	jobs := make(chan App)
	results := make(chan object, len(apps))
	var wg sync.WaitGroup
	for i := 0; i < min(o.MaxParallel, len(apps)); i++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			for app := range jobs {
				results <- run(app)
			}
		}()
	}
	go func() {
		for _, app := range apps {
			jobs <- app
		}
		close(jobs)
		wg.Wait()
		close(results)
	}()
	for entry := range results {
		summary = append(summary, entry)
	}
	return summary
}
func resultEntry(app string, result object, e error) object {
	if e != nil {
		stderr("error: [%s] %v\n", app, e)
		return object{"app": app, "status": "failed", "error": e.Error()}
	}
	return object{"app": app, "status": "success", "result": result}
}
func (c *Client) BatchDeploy(path string, o Options) ([]object, error) {
	apps, e := ReadAppsFile(path)
	if e != nil {
		return nil, e
	}
	env, e := c.Resolve(o.Env, "environment")
	if e != nil {
		return nil, e
	}
	if _, e = c.Token(); e != nil {
		return nil, e
	}
	plan := [][]App{apps}
	if !o.SkipDependencies {
		plan, e = c.DependencyPlan(apps, env)
		if e != nil {
			return nil, e
		}
		explicit := map[string]bool{}
		for _, app := range apps {
			key, e := c.Resolve(app.Key, "app")
			if e != nil {
				return nil, e
			}
			explicit[key] = true
		}
		added := 0
		for _, level := range plan {
			for _, app := range level {
				if !explicit[app.Key] {
					added++
				}
			}
		}
		if added > 0 {
			printlnLocked("Including %d dependency app(s) not listed in %s.", added, path)
		}
	}
	summary := []object{}
	for _, level := range plan {
		summary = append(summary, runParallel(level, o, func(app App) object {
			printlnLocked("\n=== Deploying '%s' ===", app.Key)
			result, e := c.DeployApp(app.Key, env, app.Revision, o)
			return resultEntry(app.Key, result, e)
		})...)
		if !o.ContinueOnError && hasFailed(summary) {
			break
		}
	}
	return summary, nil
}
func (c *Client) BatchUndeploy(path string, o Options) ([]object, error) {
	apps, e := ReadAppsFile(path)
	if e != nil {
		return nil, e
	}
	env, e := c.Resolve(o.Env, "environment")
	if e != nil {
		return nil, e
	}
	if _, e = c.Token(); e != nil {
		return nil, e
	}
	plan := [][]App{apps}
	if !o.SkipDependencies {
		plan, e = c.DependencyPlan(apps, env)
		if e != nil {
			return nil, e
		}
		explicit := map[string]bool{}
		for _, app := range apps {
			key, e := c.Resolve(app.Key, "app")
			if e != nil {
				return nil, e
			}
			explicit[key] = true
		}
		added := 0
		for _, level := range plan {
			for _, app := range level {
				if !explicit[app.Key] {
					added++
				}
			}
		}
		if added > 0 {
			printlnLocked("Including %d dependency app(s) not listed in %s.", added, path)
		}
	}
	summary := []object{}
	for i := len(plan) - 1; i >= 0; i-- {
		level := plan[i]
		summary = append(summary, runParallel(level, o, func(app App) object {
			printlnLocked("\n=== Undeploying '%s' ===", app.Key)
			d, e := c.Undeploy(app.Key, env)
			if e == nil {
				d, e = c.waitOperation(d, "undeploy", o)
			}
			return resultEntry(app.Key, d, e)
		})...)
		if !o.ContinueOnError && hasFailed(summary) {
			break
		}
	}
	return summary, nil
}
func (c *Client) BatchDelete(path string, o Options) ([]object, error) {
	apps, e := ReadAppsFile(path)
	if e != nil {
		return nil, e
	}
	plan := [][]App{apps}
	if !o.SkipDependencies {
		env, e := c.Resolve(o.Env, "environment")
		if e != nil {
			return nil, e
		}
		plan, e = c.DependencyPlan(apps, env)
		if e != nil {
			return nil, e
		}
		explicit := map[string]bool{}
		for _, app := range apps {
			key, e := c.Resolve(app.Key, "app")
			if e != nil {
				return nil, e
			}
			explicit[key] = true
		}
		added := 0
		for _, level := range plan {
			for _, app := range level {
				if !explicit[app.Key] {
					added++
				}
			}
		}
		if added > 0 {
			printlnLocked("Including %d dependency app(s) not listed in %s.", added, path)
		}
	}
	if _, e = c.Token(); e != nil {
		return nil, e
	}
	summary := []object{}
	for i := len(plan) - 1; i >= 0; i-- {
		level := plan[i]
		summary = append(summary, runParallel(level, o, func(app App) object {
			printlnLocked("\n=== Deleting '%s' ===", app.Key)
			key, e := c.Resolve(app.Key, "app")
			if e == nil {
				_, e = c.call("DELETE", "asset-repository", "/assets/"+esc(key), nil, nil)
			}
			return resultEntry(app.Key, object{"status": "success"}, e)
		})...)
		if !o.ContinueOnError && hasFailed(summary) {
			break
		}
	}
	return summary, nil
}
func (c *Client) UndeployAll(o Options) ([]object, error) {
	env, e := c.Resolve(o.Env, "environment")
	if e != nil {
		return nil, e
	}
	deployed, e := c.ListDeployedApps(env)
	if e != nil {
		return nil, e
	}
	apps := []App{}
	names := map[string]string{}
	for _, app := range deployed {
		key := str(app["key"])
		if key == "" {
			continue
		}
		name := key
		for _, d := range objects(app["deployments"]) {
			if str(d["name"]) != "" {
				name = str(d["name"])
				break
			}
		}
		apps = append(apps, App{Key: key})
		names[key] = name
	}
	if len(apps) == 0 {
		printlnLocked("No deployed apps found in environment %s.", env)
		return []object{}, nil
	}
	if _, e = c.Token(); e != nil {
		return nil, e
	}
	return runParallel(apps, o, func(app App) object {
		printlnLocked("\n=== Undeploying '%s' ===", names[app.Key])
		d, e := c.Undeploy(app.Key, env)
		if e == nil {
			d, e = c.waitOperation(d, "undeploy", o)
		}
		entry := resultEntry(names[app.Key], d, e)
		entry["assetKey"] = app.Key
		return entry
	}), nil
}
