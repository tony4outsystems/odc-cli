package odc

import (
	"errors"
	"flag"
	"fmt"
	"math"
	"os"
	"path/filepath"
	"strconv"
	"strings"
	"time"
)

var commands = []string{"discover", "validate", "latest-revision", "list-environments", "list-apps", "get-app", "producer-graph", "get-user", "deploy", "batch-deploy", "undeploy", "dangerous-batch-undeploy-all", "delete-app", "update-user", "internal-build", "internal-publish", "internal-deploy"}
var assetTypes = []string{"WebApplication", "MobileApplication", "LowCodeLibrary", "ExtensionLibrary", "ExternalConnection", "ExternalLibrary", "Workflow", "WidgetLibrary", "AIModelConnection", "SearchServiceConnection", "Agent", "MCPConnection", "A2AConnection", "KnowledgeBase"}

func member(value string, values ...string) bool {
	for _, v := range values {
		if v == value {
			return true
		}
	}
	return false
}
func usage() {
	fmt.Println("OutSystems ODC CLI\n\nUsage: odc <command> [options]\n\nCommands:")
	for _, command := range commands {
		fmt.Println("  " + command)
	}
	fmt.Println("\nUse odc <command> --help for command options.")
}
func parseArgs(args []string) (string, Options, []string, error) {
	o := Options{Updates: object{}}
	if len(args) == 0 {
		return "", o, nil, errorf("A command is required; use odc --help")
	}
	cmd := args[0]
	if cmd == "--help" || cmd == "-h" || cmd == "help" {
		usage()
		return "", o, nil, nil
	}
	if !member(cmd, commands...) {
		return "", o, nil, errorf("Unknown command %q; use odc --help", cmd)
	}
	fs := flag.NewFlagSet(cmd, flag.ContinueOnError)
	fs.SetOutput(os.Stdout)
	positionalName := ""
	switch cmd {
	case "get-app", "producer-graph":
		positionalName = "[asset-name-or-key]"
	case "get-user", "update-user":
		positionalName = "<user-key-or-email>"
	case "batch-deploy":
		positionalName = "<apps-file>"
	}
	fs.Usage = func() {
		fmt.Fprintf(fs.Output(), "Usage: odc %s %s [options]\n", cmd, positionalName)
		fs.PrintDefaults()
	}
	common := member(cmd, "validate", "deploy", "internal-build", "internal-publish", "internal-deploy", "undeploy")
	batch := member(cmd, "batch-deploy", "dangerous-batch-undeploy-all")
	if common || member(cmd, "latest-revision", "get-app", "delete-app", "producer-graph") {
		fs.StringVar(&o.Asset, "asset", "", "Asset name or key.")
	}
	if common || batch || cmd == "producer-graph" {
		fs.StringVar(&o.Env, "env", "", "Environment name or key.")
	}
	if common || cmd == "producer-graph" {
		fs.Func("revision", "Asset revision (defaults to current; graph defaults to latest).", func(value string) error {
			n, e := strconv.Atoi(value)
			if e != nil || n < 1 {
				return errorf("revision must be a positive integer")
			}
			o.Revision = &n
			return nil
		})
	}
	interval, timeout := 10.0, 1800.0
	if common || batch {
		fs.Float64Var(&interval, "poll-interval", 10, "Seconds between status polls.")
		fs.Float64Var(&timeout, "timeout", 1800, "Seconds to wait before giving up.")
	}
	o.BuildType = "Release"
	if member(cmd, "deploy", "internal-build", "batch-deploy") {
		fs.StringVar(&o.BuildType, "build-type", "Release", "Debug or Release.")
	}
	if member(cmd, "internal-build", "internal-publish", "internal-deploy", "undeploy") {
		fs.BoolVar(&o.NoWait, "no-wait", false, "Return after starting the operation.")
	}
	if cmd == "internal-deploy" {
		fs.StringVar(&o.BuildKey, "build-key", "", "Existing build key (required).")
	}
	o.MaxParallel = 3
	if batch {
		fs.IntVar(&o.MaxParallel, "max-parallel", 3, "Maximum concurrent apps.")
		fs.BoolVar(&o.ContinueOnError, "continue-on-error", false, "Continue after failures; concurrent in-flight apps always finish.")
	}
	if cmd == "batch-deploy" {
		fs.BoolVar(&o.SkipDependencies, "skip-dependencies", false, "Deploy only apps explicitly listed in the file.")
	}
	if cmd == "producer-graph" {
		fs.IntVar(&o.MaxDepth, "max-depth", 0, "Maximum producer depth; 0 means unlimited.")
		fs.StringVar(&o.Filter, "producer-type-filter", "Deployable", "Deployable, Libraries, or All.")
		fs.BoolVar(&o.AllProducers, "all-producers", false, "Shortcut for --producer-type-filter All.")
		fs.StringVar(&o.Output, "output", "", "Mermaid output file.")
	}
	if cmd == "list-apps" {
		fs.StringVar(&o.AssetType, "type", "", "Asset type: "+strings.Join(assetTypes, ", "))
		fs.StringVar(&o.Search, "search", "", "Name/key substring, case-insensitive.")
	}
	if cmd == "update-user" {
		fs.Func("name", "User name.", func(v string) error { o.Updates["name"] = v; return nil })
		fs.Func("photo-url", "User photo URL.", func(v string) error { o.Updates["photoUrl"] = v; return nil })
		fs.Func("is-active", "true/false, 1/0, or yes/no.", func(v string) error {
			switch strings.ToLower(v) {
			case "true", "1", "yes":
				o.Updates["isActive"] = true
			case "false", "0", "no":
				o.Updates["isActive"] = false
			default:
				return errorf("is-active must be true/false, 1/0, or yes/no")
			}
			return nil
		})
	}
	// Go's flag package stops at the first positional argument. Reorder only
	// positional arguments, preserving each option and its value together.
	flags := []string{}
	positionals := []string{}
	for i := 1; i < len(args); i++ {
		a := args[i]
		if a == "--" {
			positionals = append(positionals, args[i+1:]...)
			break
		}
		if !strings.HasPrefix(a, "-") || a == "-" {
			positionals = append(positionals, a)
			continue
		}
		flags = append(flags, a)
		name, _, hasValue := strings.Cut(strings.TrimLeft(a, "-"), "=")
		f := fs.Lookup(name)
		if f == nil || hasValue {
			continue
		}
		isBool := false
		if b, ok := f.Value.(interface{ IsBoolFlag() bool }); ok {
			isBool = b.IsBoolFlag()
		}
		if !isBool && i+1 < len(args) {
			i++
			flags = append(flags, args[i])
		}
	}
	if e := fs.Parse(flags); e != nil {
		if errors.Is(e, flag.ErrHelp) {
			return "", o, nil, nil
		}
		return "", o, nil, e
	}
	if positionalName == "" && len(positionals) > 0 || len(positionals) > 1 {
		return "", o, nil, errorf("Unexpected positional arguments for %s", cmd)
	}
	if member(cmd, "get-user", "update-user", "batch-deploy") && len(positionals) != 1 {
		return "", o, nil, errorf("%s requires %s", cmd, positionalName)
	}
	if member(cmd, "get-app", "producer-graph") && len(positionals) == 1 {
		o.Asset = positionals[0]
	}
	if common || member(cmd, "latest-revision", "get-app", "delete-app", "producer-graph") {
		if o.Asset == "" {
			return "", o, nil, errorf("--asset or an asset positional argument is required")
		}
	}
	if (common || batch) && o.Env == "" {
		return "", o, nil, errorf("--env is required")
	}
	if cmd == "internal-deploy" && o.BuildKey == "" {
		return "", o, nil, errorf("--build-key is required")
	}
	if !member(o.BuildType, "Debug", "Release") {
		return "", o, nil, errorf("--build-type must be Debug or Release")
	}
	if cmd == "producer-graph" && (!member(o.Filter, "Deployable", "Libraries", "All") || o.MaxDepth < 0) {
		return "", o, nil, errorf("Use a nonnegative --max-depth and --producer-type-filter Deployable, Libraries, or All")
	}
	if o.AssetType != "" && !member(o.AssetType, assetTypes...) {
		return "", o, nil, errorf("Invalid --type %q", o.AssetType)
	}
	if cmd == "update-user" && len(o.Updates) == 0 {
		return "", o, nil, errorf("At least one field must be specified for update (--name, --is-active, or --photo-url)")
	}
	maxSeconds := float64(math.MaxInt64) / float64(time.Second)
	if math.IsNaN(interval) || math.IsInf(interval, 0) || interval < 0 || interval >= maxSeconds || math.IsNaN(timeout) || math.IsInf(timeout, 0) || timeout <= 0 || timeout >= maxSeconds {
		return "", o, nil, errorf("--poll-interval must be finite and nonnegative; --timeout must be finite and positive")
	}
	o.Interval = time.Duration(interval * float64(time.Second))
	o.Timeout = time.Duration(timeout * float64(time.Second))
	o.MaxParallel = max(1, o.MaxParallel)
	return cmd, o, positionals, nil
}
func Run(args []string) error {
	cmd, o, pos, e := parseArgs(args)
	if e != nil || cmd == "" {
		return e
	}
	s, e := LoadSettings()
	if e != nil {
		return e
	}
	c := NewClient(s)
	defer c.HTTP.CloseIdleConnections()
	return execute(c, cmd, o, pos)
}
func execute(c *Client, cmd string, o Options, pos []string) error {
	switch cmd {
	case "discover":
		d, e := c.Discover()
		if e != nil {
			return e
		}
		return PrintJSON(object{"issuer": d["issuer"], "token_endpoint": d["token_endpoint"], "scopes_supported": d["scopes_supported"]})
	case "list-environments":
		items, e := c.ListEnvironments()
		if e != nil {
			return e
		}
		out := []object{}
		for _, item := range items {
			out = append(out, object{"name": item["name"], "key": item["key"], "type": first(item["type"], item["stage"])})
		}
		return PrintJSON(out)
	case "list-apps":
		items, e := c.ListAssets()
		if e != nil {
			return e
		}
		out := []object{}
		for _, item := range items {
			kind := first(item["assetType"], item["type"])
			if o.Search != "" && !contains(item["name"], o.Search) && !contains(item["assetKey"], o.Search) {
				continue
			}
			if o.AssetType != "" && !strings.EqualFold(str(kind), o.AssetType) {
				continue
			}
			out = append(out, object{"name": item["name"], "key": item["assetKey"], "type": kind})
		}
		return PrintJSON(out)
	case "get-user", "update-user":
		key, e := c.Resolve(pos[0], "user")
		if e != nil {
			return e
		}
		if cmd == "get-user" {
			d, e := c.call("GET", "identity", "/users/"+esc(key), nil, nil)
			if e != nil {
				return e
			}
			return PrintJSON(d)
		}
		if _, e = c.call("PATCH", "identity", "/users/"+esc(key), o.Updates, nil); e != nil {
			return e
		}
		return PrintJSON(object{"status": "success", "message": fmt.Sprintf("User %s updated successfully", key)})
	case "deploy":
		_, e := c.DeployAsset(o.Asset, o.Env, o.Revision, o)
		return e
	case "batch-deploy", "dangerous-batch-undeploy-all":
		var summary []object
		var e error
		label := "deploy"
		if cmd == "batch-deploy" {
			summary, e = c.BatchDeploy(pos[0], o)
		} else {
			label = "undeploy-all"
			summary, e = c.UndeployAll(o)
		}
		if e != nil {
			return e
		}
		printlnLocked("\n=== Batch %s summary ===", label)
		if e = PrintJSON(summary); e != nil {
			return e
		}
		if hasFailed(summary) {
			return errorf("One or more apps failed to %s; see summary above.", label)
		}
		return nil
	}
	key, e := c.Resolve(o.Asset, "asset")
	if e != nil {
		return e
	}
	switch cmd {
	case "latest-revision":
		n, e := c.LatestRevision(key)
		if e != nil {
			return e
		}
		printlnLocked("%d", n)
		return nil
	case "get-app":
		d, e := c.GetAsset(key)
		if e != nil {
			return e
		}
		return PrintJSON(d)
	case "delete-app":
		if _, e = c.call("DELETE", "asset-repository", "/assets/"+esc(key), nil, nil); e != nil {
			return e
		}
		return PrintJSON(object{"status": "success", "message": fmt.Sprintf("Asset %s deleted successfully", key)})
	case "producer-graph":
		return c.writeGraph(key, o)
	}
	env, e := c.Resolve(o.Env, "environment")
	if e != nil {
		return e
	}
	if cmd == "undeploy" {
		d, e := c.Undeploy(key, env)
		if e != nil {
			return e
		}
		if e = PrintJSON(d); e != nil {
			return e
		}
		d, e = c.waitOperation(d, "undeploy", o)
		if e != nil {
			return e
		}
		if !o.NoWait {
			return PrintJSON(d)
		}
		return nil
	}
	rev, e := c.Preflight(key, env, o.Revision)
	if e != nil {
		return e
	}
	if cmd == "validate" {
		return nil
	}
	var d object
	kind := ""
	switch cmd {
	case "internal-build":
		kind = "build"
		d, e = c.StartBuild(key, rev, o.BuildType)
	case "internal-publish":
		kind = "publish"
		d, e = c.Publish(key, rev, env)
	case "internal-deploy":
		kind = "deployment"
		d, e = c.Deploy(key, rev, o.BuildKey, env)
	default:
		return errorf("Unhandled command %s", cmd)
	}
	if e != nil {
		return e
	}
	if e = PrintJSON(d); e != nil {
		return e
	}
	d, e = c.waitOperation(d, kind, o)
	if e != nil {
		return e
	}
	if !o.NoWait {
		return PrintJSON(d)
	}
	return nil
}
func (c *Client) writeGraph(key string, o Options) error {
	env := ""
	var e error
	if o.Env != "" {
		env, e = c.Resolve(o.Env, "environment")
		if e != nil {
			return e
		}
	}
	var rev int
	if o.Revision != nil {
		rev = *o.Revision
	} else {
		rev, e = c.LatestRevision(key)
		if e != nil {
			return e
		}
	}
	asset, e := c.GetAsset(key)
	if e != nil {
		return e
	}
	filter := o.Filter
	if o.AllProducers {
		filter = "All"
	}
	g, e := c.ProducerGraph(key, rev, env, o.MaxDepth, filter)
	if e != nil {
		return e
	}
	producers := objects(g["results"])
	root := object{"key": key, "name": asset["name"], "revision": rev, "type": first(asset["assetType"], asset["type"])}
	output := o.Output
	if output == "" {
		output = defaultMermaidPath(key, rev)
	}
	if e = os.MkdirAll(filepath.Dir(output), 0755); e != nil {
		return e
	}
	if e = os.WriteFile(output, []byte(RenderProducerGraph(root, producers)), 0644); e != nil {
		return e
	}
	return PrintJSON(object{"assetKey": o.Asset, "producerTypeFilter": filter, "revision": rev, "topLevelProducerCount": len(producers), "output": output})
}
