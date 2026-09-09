package odc

import (
	"fmt"
	"regexp"
	"strings"
)

var guidPattern = regexp.MustCompile(`(?i)^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$`)

func (c *Client) Resolve(input, kind string) (string, error) {
	if input == "" {
		return "", errorf("%s is required", kind)
	}
	if guidPattern.MatchString(input) {
		return input, nil
	}
	var items []object
	var e error
	keyField := "key"
	switch kind {
	case "asset":
		items, e = c.ListAssets()
		keyField = "assetKey"
	case "environment":
		items, e = c.ListEnvironments()
	case "user":
		items, e = c.QueryUsers(input)
	default:
		return "", errorf("Unknown identifier kind: %s", kind)
	}
	if e != nil {
		return "", e
	}
	matches := []object{}
	exact := []object{}
	for _, item := range items {
		if kind == "user" || contains(item["name"], input) || contains(item[keyField], input) {
			matches = append(matches, item)
		}
		if strings.EqualFold(str(item["name"]), input) {
			exact = append(exact, item)
		}
	}
	if kind == "user" {
		emails := []object{}
		for _, item := range items {
			if strings.EqualFold(str(item["email"]), input) {
				emails = append(emails, item)
			}
		}
		if len(emails) == 1 {
			return requireString(emails[0][keyField], kind+" key")
		}
	}
	if len(exact) == 1 {
		return requireString(exact[0][keyField], kind+" key")
	}
	if len(exact) == 0 && kind != "asset" && len(matches) == 1 {
		return requireString(matches[0][keyField], kind+" key")
	}
	message := fmt.Sprintf("No exact match for %q. Did you mean:", input)
	if len(exact) > 1 {
		message = fmt.Sprintf("Multiple %ss match the name (ambiguous):", kind)
		matches = exact
	}
	if len(matches) == 0 {
		message = fmt.Sprintf("No %ss found matching %q", kind, input)
		if kind == "environment" {
			message += "\nAvailable environments:"
			matches = items
		}
	}
	for _, item := range matches[:min(10, len(matches))] {
		field := keyField
		if kind == "user" {
			field = "email"
		}
		message += fmt.Sprintf("\n  - %v (%v)", item["name"], item[field])
	}
	if len(matches) > 10 {
		message += fmt.Sprintf("\n  ... and %d more", len(matches)-10)
	}
	return "", errorf("%s", message)
}
