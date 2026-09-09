package odc

import (
	"bytes"
	"encoding/json"
	"fmt"
	"io"
	"os"
	"sort"
	"strings"
	"text/tabwriter"
	"unicode"
)

// Output settings are configured once before command workers start.
var outputJSON bool
var outputColor = "auto"

func colorEnabled() bool {
	if outputColor == "always" {
		return true
	}
	if outputColor == "never" {
		return false
	}
	if _, ok := os.LookupEnv("NO_COLOR"); ok {
		return false
	}
	info, err := os.Stdout.Stat()
	return err == nil && info.Mode()&os.ModeCharDevice != 0 && os.Getenv("TERM") != "dumb"
}

func paint(s, code string, color bool) string {
	if color {
		return "\x1b[" + code + "m" + s + "\x1b[0m"
	}
	return s
}

func label(s string) string {
	s = strings.ReplaceAll(strings.ReplaceAll(s, "asset", "app"), "Asset", "App")
	var b strings.Builder
	var prev rune
	for i, r := range s {
		if r == '_' || r == '-' {
			b.WriteRune(' ')
			prev = r
			continue
		}
		if i > 0 && unicode.IsUpper(r) && (unicode.IsLower(prev) || unicode.IsDigit(prev)) {
			b.WriteRune(' ')
		}
		if i == 0 {
			r = unicode.ToUpper(r)
		}
		b.WriteRune(r)
		prev = r
	}
	return b.String()
}

func fields(m map[string]any) []string {
	keys := make([]string, 0, len(m))
	for k := range m {
		keys = append(keys, k)
	}
	sort.Strings(keys)
	ordered := []string{}
	for _, k := range []string{"name", "key", "assetKey", "type", "status"} {
		if _, ok := m[k]; ok {
			ordered = append(ordered, k)
		}
	}
	for _, k := range keys {
		if !member(k, ordered...) {
			ordered = append(ordered, k)
		}
	}
	return ordered
}

func scalar(v any, color bool) string {
	s := "—"
	if v != nil {
		s = fmt.Sprint(v)
	}
	// Keep remote strings from injecting terminal controls or table separators.
	s = strings.Map(func(r rune) rune {
		if unicode.IsControl(r) {
			return ' '
		}
		return r
	}, s)
	code := ""
	switch strings.ToLower(s) {
	case "success", "succeeded", "completed", "true", "active":
		code = "32"
	case "failed", "failure", "error", "false":
		code = "31"
	case "pending", "running", "inprogress", "in progress", "queued":
		code = "33"
	}
	if code != "" {
		return paint(s, code, color)
	}
	return s
}

func renderPretty(w io.Writer, v any, indent string, color bool) {
	switch x := v.(type) {
	case map[string]any:
		if len(x) == 0 {
			fmt.Fprintln(w, indent+"(empty)")
			return
		}
		for _, k := range fields(x) {
			heading := indent + paint(label(k), "1;36", color)
			switch x[k].(type) {
			case map[string]any, []any:
				fmt.Fprintln(w, heading+":")
				renderPretty(w, x[k], indent+"  ", color)
			default:
				fmt.Fprintf(w, "%s: %s\n", heading, scalar(x[k], color))
			}
		}
	case []any:
		if len(x) == 0 {
			fmt.Fprintln(w, indent+"No results.")
			return
		}
		union := map[string]any{}
		table := true
		for _, row := range x {
			m, ok := row.(map[string]any)
			if !ok || len(m) == 0 {
				table = false
				break
			}
			for k, cell := range m {
				switch cell.(type) {
				case map[string]any, []any:
					table = false
				}
				union[k] = nil
			}
		}
		if table {
			keys := fields(union)
			// Align plain text first; ANSI sequences must not affect column widths.
			var b bytes.Buffer
			tw := tabwriter.NewWriter(&b, 0, 4, 3, ' ', 0)
			heads := []string{}
			for _, k := range keys {
				heads = append(heads, strings.ToUpper(label(k)))
			}
			fmt.Fprintln(tw, strings.Join(heads, "\t"))
			for _, row := range x {
				cells := []string{}
				for _, k := range keys {
					cells = append(cells, scalar(row.(map[string]any)[k], false))
				}
				fmt.Fprintln(tw, strings.Join(cells, "\t"))
			}
			tw.Flush()
			for i, line := range strings.Split(strings.TrimSuffix(b.String(), "\n"), "\n") {
				if i == 0 {
					line = paint(line, "1;36", color)
				} else if color {
					status := scalar(x[i-1].(map[string]any)["status"], false)
					styled := scalar(status, true)
					if styled != status {
						line = strings.Replace(line, status, styled, 1)
					}
				}
				fmt.Fprintln(w, indent+line)
			}
			fmt.Fprintf(w, "%s%d results\n", indent, len(x))
			return
		}
		for i, row := range x {
			switch row.(type) {
			case map[string]any, []any:
				fmt.Fprintf(w, "%s[%d]\n", indent, i+1)
				renderPretty(w, row, indent+"  ", color)
			default:
				fmt.Fprintln(w, indent+"• "+scalar(row, color))
			}
		}
	default:
		fmt.Fprintln(w, indent+scalar(v, color))
	}
}

func writeResult(w io.Writer, payload any, asJSON, color bool) error {
	encoded, err := json.MarshalIndent(payload, "", "  ")
	if err != nil {
		return err
	}
	if asJSON {
		_, err = fmt.Fprintln(w, string(encoded))
		return err
	}
	var normalized any
	decoder := json.NewDecoder(bytes.NewReader(encoded))
	decoder.UseNumber()
	if err = decoder.Decode(&normalized); err != nil {
		return err
	}
	var b bytes.Buffer
	renderPretty(&b, normalized, "", color)
	_, err = w.Write(b.Bytes())
	return err
}
