package odc

import (
	"fmt"
	"os"
	"sync"
)

var printMu sync.Mutex

func PrintResult(payload any) error {
	printMu.Lock()
	defer printMu.Unlock()
	return writeResult(os.Stdout, payload, outputJSON, colorEnabled())
}

func compactMap(payload map[string]any, fields []string) map[string]any {
	out := map[string]any{}
	for _, field := range fields {
		value, ok := payload[field]
		if ok && value != nil && value != "" {
			out[field] = value
		}
	}
	return out
}

func requireString(value any, label string) (string, error) {
	text, ok := value.(string)
	if !ok || text == "" {
		return "", errorf("%s is required", label)
	}
	return text, nil
}

func stderr(format string, args ...any) {
	fmt.Fprintf(os.Stderr, format, args...)
}

func hasFailed(summary []map[string]any) bool {
	for _, entry := range summary {
		if entry["status"] == "failed" {
			return true
		}
	}
	return false
}
