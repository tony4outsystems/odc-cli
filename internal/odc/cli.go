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

var commands = []string{"list-deployed-apps", "analyze-deployment", "analyze-deletion", "list-revisions", "get-revision", "download-source-code", "upload-source-code", "login", "discover", "validate", "latest-revision", "list-environments", "list-apps", "get-app", "producer-graph", "get-user", "deploy", "batch-deploy", "batch-undeploy", "batch-delete", "undeploy", "dangerous-batch-undeploy-all", "delete-app", "update-user", "grant-role", "revoke-role", "internal-build", "internal-publish", "internal-deploy"}
var appTypes = []string{"WebApplication", "MobileApplication", "LowCodeLibrary", "ExtensionLibrary", "ExternalConnection", "ExternalLibrary", "Workflow", "WidgetLibrary", "AIModelConnection", "SearchServiceConnection", "Agent", "MCPConnection", "A2AConnection", "KnowledgeBase"}

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
	command.Short = map[string]string{
		"list-deployed-apps":   "List deployed apps, optionally filtered by environment, name or key.",
		"list-revisions":       "List all revisions of an app.",
		"get-revision":         "Retrieve a specific app revision.",
		"download-source-code": "Download the OML source code of an app revision.",
		"upload-source-code":   "Upload an OML/XIF file, creating a new asset or revision.",
		"analyze-deployment":   "Analyze the impact of deploying an app revision.",
		"analyze-deletion":     "Analyze the impact of deleting an app.",
		"grant-role":           "Grant an application role to a user.",
		"revoke-role":          "Revoke an application role from a user.",
	}[cmd]
	fs := command.Flags()
	positionalName := ""
	switch cmd {
	case "login":
		positionalName = "<tenant-url> <client-id>"
		command.Short = "Save credentials in ~/.odc/config.json (prompts for client secret)."
	case "get-app", "producer-graph", "list-revisions", "get-revision", "download-source-code", "analyze-deployment", "analyze-deletion":
		positionalName = "[app-name-or-key]"
	case "get-user", "update-user":
		positionalName = "<user-key-or-email>"
	case "grant-role", "revoke-role":
		positionalName = "<user-key-or-email> <role-name-or-key>"
	case "batch-deploy", "batch-undeploy", "batch-delete":
		positionalName = "<apps-file>"
	case "upload-source-code":
		positionalName = "<oml-file>"
	}
	if positionalName != "" {
		command.Use += " " + positionalName
	}
	analysis := member(cmd, "analyze-deployment", "analyze-deletion")
	appCommand := member(cmd, "latest-revision", "get-app", "delete-app", "producer-graph", "list-revisions", "get-revision", "download-source-code") || analysis
	common := member(cmd, "validate", "deploy", "internal-build", "internal-publish", "internal-deploy", "undeploy")
	batch := member(cmd, "batch-deploy", "batch-undeploy", "batch-delete", "dangerous-batch-undeploy-all")
	if common || appCommand {
		fs.StringVar(&o.App, "app", "", "App name or key.")
	}
	if member(cmd, "grant-role", "revoke-role") {
		fs.StringVar(&o.App, "app", "", "App name or key, to disambiguate roles with the same name across apps.")
	}
	if common || batch || member(cmd, "producer-graph", "list-deployed-apps", "analyze-deployment") {
		fs.StringVar(&o.Env, "env", "", "Environment name or key.")
	}
	if common || member(cmd, "producer-graph", "get-revision", "download-source-code", "analyze-deployment") {
		fs.Func("revision", "App revision (required for get-revision; graph, download, and analysis default to latest; otherwise current).", func(value string) error {
			n, e := strconv.Atoi(value)
			if e != nil || n < 1 {
				return errorf("revision must be a positive integer")
			}
			o.Revision = &n
			return nil
		})
	}
	interval, timeout := 10.0, 1800.0
	if common || (batch && cmd != "batch-delete") || analysis {
		fs.Float64Var(&interval, "poll-interval", 10, "Seconds between status polls.")
		fs.Float64Var(&timeout, "timeout", 1800, "Seconds to wait before giving up.")
	}
	o.BuildType = "Release"
	if member(cmd, "deploy", "internal-build", "batch-deploy") {
		fs.StringVar(&o.BuildType, "build-type", "Release", "Debug or Release.")
	}
	if analysis || member(cmd, "internal-build", "internal-publish", "internal-deploy", "undeploy", "batch-undeploy") {
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
	if cmd == "batch-undeploy" {
		fs.BoolVar(&o.SkipDependencies, "skip-dependencies", false, "Undeploy only apps explicitly listed in the file.")
	}
	if cmd == "batch-delete" {
		fs.BoolVar(&o.SkipDependencies, "skip-dependencies", false, "Delete only apps explicitly listed in the file.")
	}
	if cmd == "producer-graph" {
		fs.IntVar(&o.MaxDepth, "max-depth", 0, "Maximum producer depth; 0 means unlimited.")
		fs.StringVar(&o.Filter, "producer-type-filter", "Deployable", "Deployable, Libraries, or All.")
		fs.BoolVar(&o.AllProducers, "all-producers", false, "Shortcut for --producer-type-filter All.")
		fs.StringVar(&o.Output, "output", "", "Mermaid output file.")
	}
	if cmd == "download-source-code" {
		fs.StringVar(&o.Output, "output", "", "Output file path; defaults to <app>-rev-<revision>.oml.")
	}
	if cmd == "list-apps" {
		fs.StringVar(&o.AppType, "type", "", "App type: "+strings.Join(appTypes, ", "))
	}
	if member(cmd, "list-apps", "list-deployed-apps") {
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
		if cmd == "login" {
			if len(positionals) != 2 {
				return errorf("login requires <tenant-url> <client-id>")
			}
			return accept(cmd, o, positionals)
		}
		maxPositionals := 1
		if member(cmd, "grant-role", "revoke-role") {
			maxPositionals = 2
		}
		if positionalName == "" && len(positionals) > 0 || len(positionals) > maxPositionals {
			return errorf("Unexpected positional arguments for %s", cmd)
		}
		if member(cmd, "get-user", "update-user", "batch-deploy", "batch-undeploy", "batch-delete", "upload-source-code") && len(positionals) != 1 {
			return errorf("%s requires %s", cmd, positionalName)
		}
		if member(cmd, "grant-role", "revoke-role") && len(positionals) != 2 {
			return errorf("%s requires %s", cmd, positionalName)
		}
		if member(cmd, "get-app", "producer-graph", "list-revisions", "get-revision", "download-source-code", "analyze-deployment", "analyze-deletion") && len(positionals) == 1 {
			o.App = positionals[0]
		}
		if common || appCommand {
			if o.App == "" {
				return errorf("--app or an app positional argument is required")
			}
		}
		if (common || (batch && cmd != "batch-delete") || cmd == "analyze-deployment") && o.Env == "" {
			return errorf("--env is required")
		}
		if cmd == "batch-delete" && o.Env == "" && !o.SkipDependencies {
			return errorf("--env is required unless --skip-dependencies is set")
		}
		if cmd == "get-revision" && o.Revision == nil {
			return errorf("--revision is required")
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
		if o.AppType != "" && !member(o.AppType, appTypes...) {
			return errorf("Invalid --type %q", o.AppType)
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
	if cmd == "login" {
		return login(pos[0], pos[1], promptSecret)
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
	case "list-deployed-apps":
		env := ""
		if o.Env != "" {
			var e error
			env, e = c.Resolve(o.Env, "environment")
			if e != nil {
				return e
			}
		}
		items, e := c.ListDeployedApps(env)
		if e != nil {
			return e
		}
		return PrintResult(deployedAppRows(items, env, o.Search))
	case "list-apps":
		items, e := c.ListApps()
		if e != nil {
			return e
		}
		out := []object{}
		for _, item := range items {
			kind := first(item["assetType"], item["type"])
			if o.Search != "" && !contains(item["name"], o.Search) && !contains(item["assetKey"], o.Search) {
				continue
			}
			if o.AppType != "" && !strings.EqualFold(str(kind), o.AppType) {
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
	case "grant-role", "revoke-role":
		userKey, e := c.Resolve(pos[0], "user")
		if e != nil {
			return e
		}
		appKey := ""
		if o.App != "" {
			appKey, e = c.Resolve(o.App, "app")
			if e != nil {
				return e
			}
		}
		roleKey, e := c.ResolveRole(pos[1], appKey)
		if e != nil {
			return e
		}
		method, verb := "POST", "granted to"
		if cmd == "revoke-role" {
			method, verb = "DELETE", "revoked from"
		}
		if _, e = c.call(method, "identity", "/users/"+esc(userKey)+"/application-roles/"+esc(roleKey), nil, nil); e != nil {
			return e
		}
		return PrintResult(object{"status": "success", "message": fmt.Sprintf("Role %s user %s successfully", verb, userKey)})
	case "deploy":
		_, e := c.DeployApp(o.App, o.Env, o.Revision, o)
		return e
	case "upload-source-code":
		d, e := c.UploadSourceCode(pos[0])
		if e != nil {
			return e
		}
		return PrintResult(d)
	case "batch-deploy", "batch-undeploy", "batch-delete", "dangerous-batch-undeploy-all":
		var summary []object
		var e error
		label := "deploy"
		switch cmd {
		case "batch-deploy":
			summary, e = c.BatchDeploy(pos[0], o)
		case "batch-undeploy":
			label = "undeploy"
			summary, e = c.BatchUndeploy(pos[0], o)
		case "batch-delete":
			label = "delete"
			summary, e = c.BatchDelete(pos[0], o)
		default:
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
	key, e := c.Resolve(o.App, "app")
	if e != nil {
		return e
	}
	switch cmd {
	case "list-revisions":
		d, e := c.ListRevisions(key)
		if e != nil {
			return e
		}
		return PrintResult(d)
	case "get-revision":
		d, e := c.GetRevision(key, *o.Revision)
		if e != nil {
			return e
		}
		return PrintResult(d)
	case "download-source-code":
		return c.downloadSourceCode(key, o)
	case "analyze-deletion", "analyze-deployment":
		return c.analyze(key, cmd, o)
	case "latest-revision":
		n, e := c.LatestRevision(key)
		if e != nil {
			return e
		}
		return PrintResult(n)
	case "get-app":
		d, e := c.GetApp(key)
		if e != nil {
			return e
		}
		return PrintResult(d)
	case "delete-app":
		if _, e = c.call("DELETE", "asset-repository", "/assets/"+esc(key), nil, nil); e != nil {
			return e
		}
		return PrintResult(object{"status": "success", "message": fmt.Sprintf("App %s deleted successfully", key)})
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
func (c *Client) downloadSourceCode(key string, o Options) error {
	var rev int
	var e error
	if o.Revision != nil {
		rev = *o.Revision
	} else {
		rev, e = c.LatestRevision(key)
		if e != nil {
			return e
		}
	}
	output := o.Output
	if output == "" {
		output = fmt.Sprintf("%s-rev-%d.oml", safeFileToken(key), rev)
	}
	written, e := c.DownloadSourceCode(key, rev, output)
	if e != nil {
		return e
	}
	return PrintResult(object{"assetKey": key, "revision": rev, "output": output, "bytes": written})
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
	app, e := c.GetApp(key)
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
	root := object{"key": key, "name": app["name"], "revision": rev, "type": first(app["assetType"], app["type"])}
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
	return PrintResult(object{"assetKey": o.App, "producerTypeFilter": filter, "revision": rev, "topLevelProducerCount": len(producers), "output": output})
}
