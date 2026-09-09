package odc

import (
	"bytes"
	"encoding/json"
	"strings"
	"testing"
)

func TestResultFormats(t *testing.T) {
	payload := []object{{"name": "Example", "key": "abc", "status": "failed"}}
	for _, color := range []bool{false, true} {
		var b bytes.Buffer
		if err := writeResult(&b, payload, false, color); err != nil {
			t.Fatal(err)
		}
		s := b.String()
		for _, want := range []string{"NAME", "KEY", "STATUS", "Example", "abc", "failed", "1 results"} {
			if !strings.Contains(s, want) {
				t.Fatalf("missing %q: %s", want, s)
			}
		}
		if strings.Contains(s, "\x1b[") != color {
			t.Fatalf("unexpected color: %q", s)
		}
	}
	var b bytes.Buffer
	if err := writeResult(&b, payload, true, true); err != nil {
		t.Fatal(err)
	}
	if !json.Valid(b.Bytes()) || strings.Contains(b.String(), "\x1b") {
		t.Fatalf("invalid JSON: %s", b.String())
	}
}

func TestPrettyNestedAndEmpty(t *testing.T) {
	var b bytes.Buffer
	payload := object{"preflight": object{"revision": json.Number("9007199254740993"), "isActive": true}, "items": []string{}, "message": "hello\x1b[31m\tworld"}
	if err := writeResult(&b, payload, false, false); err != nil {
		t.Fatal(err)
	}
	for _, want := range []string{"Preflight:", "Revision: 9007199254740993", "Is Active: true", "No results."} {
		if !strings.Contains(b.String(), want) {
			t.Fatalf("missing %q: %s", want, b.String())
		}
	}
	if strings.ContainsAny(b.String(), "\x1b\t") {
		t.Fatalf("unescaped controls: %q", b.String())
	}
}

func TestOutputFlags(t *testing.T) {
	_, o, _, err := parseArgs([]string{"list-apps", "--json", "--color", "never"})
	if err != nil || !o.JSON || o.Color != "never" {
		t.Fatalf("options: %+v, %v", o, err)
	}
	if _, _, _, err := parseArgs([]string{"list-apps", "--color", "invalid"}); err == nil {
		t.Fatal("invalid color accepted")
	}
}
