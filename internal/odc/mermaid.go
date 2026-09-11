package odc

import (
	"crypto/sha1"
	"fmt"
	"sort"
	"strings"
	"unicode"
)

func mermaidNodeID(app object) string {
	revision := ""
	if n, ok := integer(app["revision"]); ok {
		revision = fmt.Sprint(n)
	}
	key := str(first(app["key"], "unknown"))
	digest := sha1.Sum([]byte(key + ":" + revision))
	return fmt.Sprintf("app_%x", digest[:6])
}
func mermaidLabel(app object) string {
	label := fmt.Sprint(first(app["name"], app["key"], "Unknown app"))
	details := []string{}
	if app["revision"] != nil {
		details = append(details, fmt.Sprintf("rev %v", app["revision"]))
	}
	if s := str(app["type"]); s != "" {
		details = append(details, s)
	}
	if len(details) > 0 {
		label += "\n" + strings.Join(details, " / ")
	}
	return strings.NewReplacer("\\", "\\\\", "\"", "\\\"").Replace(label)
}
func RenderProducerGraph(root object, producers []object) string {
	nodes := map[string]string{}
	edges := map[string]bool{}
	var visit func(object, []object)
	visit = func(parent object, children []object) {
		id := mermaidNodeID(parent)
		nodes[id] = mermaidLabel(parent)
		for _, child := range children {
			childID := mermaidNodeID(child)
			edges[id+" --> "+childID] = true
			visit(child, objects(child["producers"]))
		}
	}
	visit(root, producers)
	lines := []string{"---", "title: Producer dependency graph", "---", "flowchart LR"}
	ids := []string{}
	for id := range nodes {
		ids = append(ids, id)
	}
	sort.Strings(ids)
	for _, id := range ids {
		lines = append(lines, fmt.Sprintf("    %s[\"%s\"]", id, nodes[id]))
	}
	ids = nil
	for edge := range edges {
		ids = append(ids, edge)
	}
	sort.Strings(ids)
	for _, edge := range ids {
		lines = append(lines, "    "+edge)
	}
	return strings.Join(lines, "\n") + "\n"
}
func safeFileToken(key string) string {
	return strings.Map(func(r rune) rune {
		if unicode.IsLetter(r) || unicode.IsNumber(r) || r == '-' || r == '_' {
			return r
		}
		return '_'
	}, key)
}
func defaultMermaidPath(key string, revision int) string {
	return fmt.Sprintf("producer-graph-%s-rev-%d.mmd", safeFileToken(key), revision)
}
