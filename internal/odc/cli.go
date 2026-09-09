package odc

import (
	"fmt"
	"math"
	"os"
	"path/filepath"
	"strconv"
	"strings"
	"time"

	"github.com/spf13/cobra"
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

// parseArgs uses a fresh command tree so repeated invocations cannot retain flags.
func parseArgs(args []string) (string, Options, []string, error) {
	var selected string
	var options Options
	var positionals []string
	var jsonOutput bool
	var color string
	root := &cobra.Command{
		Use:           "odc <command>",
		Short:         "OutSystems ODC CLI",
		SilenceErrors: true,
		SilenceUsage:  true,
		RunE: func(_ *cobra.Command, _ []string) error {
			return errorf("A command is required; use odc --help")
		},
		Args: cobra.NoArgs,
	}
	root.SetOut(os.Stdout)
	root.SetErr(os.Stderr)
	root.SetArgs(args)
	root.PersistentFlags().BoolVar(&jsonOutput, "json", false, "Print JSON results (progress goes to stderr).")
	root.PersistentFlags().StringVar(&color, "color", "auto", "Color mode: auto, always, or never; auto respects NO_COLOR.")
	for _, name := range commands {
		root.AddCommand(newCLICommand(name, func(cmd string, o Options, pos []string) error {
			o.JSON, o.Color = jsonOutput, color
			if !member(o.Color, "auto", "always", "never") {
				return errorf("--color must be auto, always, or never")
			}
			selected, options, positionals = cmd, o, pos
			return nil
		}))
	}
	err := root.Execute()
	return selected, options, positionals, err
}

func newCLICommand(cmd string, accept func(string, Options, []string) error) *cobra.Command {
	o := Options{Updates: object{}}
	command := &cobra.Command{Use: cmd}
	fs := command.Flags()
	positionalName := ""
	switch cmd {
	case "get-app", "producer-graph":
		positionalName = "[asset-name-or-key]"
	case "get-user", "update-user":
		positionalName = "<user-key-or-email>"
	case "batch-deploy":
		positionalName = "<apps-file>"
	}
	if positionalName != "" {
		command.Use += " " + positionalName
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
	command.RunE = func(_ *cobra.Command, positionals []string) error {
		if positionalName == "" && len(positionals) > 0 || len(positionals) > 1 {
			return errorf("Unexpected positional arguments for %s", cmd)
		}
		if member(cmd, "get-user", "update-user", "batch-deploy") && len(positionals) != 1 {
			return errorf("%s requires %s", cmd, positionalName)
		}
		if member(cmd, "get-app", "producer-graph") && len(positionals) == 1 {
			o.Asset = positionals[0]
		}
		if common || member(cmd, "latest-revision", "get-app", "delete-app", "producer-graph") {
			if o.Asset == "" {
				return errorf("--asset or an asset positional argument is required")
			}
		}
		if (common || batch) && o.Env == "" {
			return errorf("--env is required")
		}
		if cmd == "internal-deploy" && o.BuildKey == "" {
			return errorf("--build-key is required")
		}
		if !member(o.BuildType, "Debug", "Release") {
			return errorf("--build-type must be Debug or Release")
		}
		if cmd == "producer-graph" && (!member(o.Filter, "Deployable", "Libraries", "All") || o.MaxDepth < 0) {
			return errorf("Use a nonnegative --max-depth and --producer-type-filter Deployable, Libraries, or All")
		}
		if o.AssetType != "" && !member(o.AssetType, assetTypes...) {
			return errorf("Invalid --type %q", o.AssetType)
		}
		if cmd == "update-user" && len(o.Updates) == 0 {
			return errorf("At least one field must be specified for update (--name, --is-active, or --photo-url)")
		}
		maxSeconds := float64(math.MaxInt64) / float64(time.Second)
		if math.IsNaN(interval) || math.IsInf(interval, 0) || interval < 0 || interval >= maxSeconds || math.IsNaN(timeout) || math.IsInf(timeout, 0) || timeout <= 0 || timeout >= maxSeconds {
			return errorf("--poll-interval must be finite and nonnegative; --timeout must be finite and positive")
		}
		o.Interval = time.Duration(interval * float64(time.Second))
		o.Timeout = time.Duration(timeout * float64(time.Second))
		o.MaxParallel = max(1, o.MaxParallel)
		return accept(cmd, o, positionals)
	}
	return command
}
func Run(args []string) error {
	cmd, o, pos, e := parseArgs(args)
	if e != nil || cmd == "" {
		return e
	}
	outputJSON, outputColor = o.JSON, o.Color
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
		return PrintResult(object{"issuer": d["issuer"], "token_endpoint": d["token_endpoint"], "scopes_supported": d["scopes_supported"]})
	case "list-environments":
		items, e := c.ListEnvironments()
		if e != nil {
			return e
		}
		out := []object{}
		for _, item := range items {
			out = append(out, object{"name": item["name"], "key": item["key"], "type": first(item["type"], item["stage"])})
		}
		return PrintResult(out)
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
		return PrintResult(out)
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
			return PrintResult(d)
		}
		if _, e = c.call("PATCH", "identity", "/users/"+esc(key), o.Updates, nil); e != nil {
			return e
		}
		return PrintResult(object{"status": "success", "message": fmt.Sprintf("User %s updated successfully", key)})
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
		if e = PrintResult(summary); e != nil {
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
		return PrintResult(n)
	case "get-app":
		d, e := c.GetAsset(key)
		if e != nil {
			return e
		}
		return PrintResult(d)
	case "delete-app":
		if _, e = c.call("DELETE", "asset-repository", "/assets/"+esc(key), nil, nil); e != nil {
			return e
		}
		return PrintResult(object{"status": "success", "message": fmt.Sprintf("Asset %s deleted successfully", key)})
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
		if e = PrintResult(d); e != nil {
			return e
		}
		d, e = c.waitOperation(d, "undeploy", o)
		if e != nil {
			return e
		}
		if !o.NoWait {
			return PrintResult(d)
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
	if e = PrintResult(d); e != nil {
		return e
	}
	d, e = c.waitOperation(d, kind, o)
	if e != nil {
		return e
	}
	if !o.NoWait {
		return PrintResult(d)
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
	return PrintResult(object{"assetKey": o.Asset, "producerTypeFilter": filter, "revision": rev, "topLevelProducerCount": len(producers), "output": output})
}
