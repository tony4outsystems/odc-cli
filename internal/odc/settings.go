package odc

import (
	"bufio"
	"fmt"
	"os"
	"path/filepath"
	"regexp"
	"strings"
)

type Settings struct{ TenantURL, ClientID, ClientSecret string }

func (s Settings) TenantOrigin() string { return strings.TrimRight(s.TenantURL, "/") }
func LoadSettings() (Settings, error) {
	if e := loadEnvironment(); e != nil {
		return Settings{}, e
	}
	required := []string{"ODC_TENANT_URL", "ODC_CLIENT_ID", "ODC_CLIENT_SECRET"}
	missing := []string{}
	for _, name := range required {
		if os.Getenv(name) == "" {
			missing = append(missing, name)
		}
	}
	if len(missing) > 0 {
		return Settings{}, errorf("Missing required environment variables: %s", strings.Join(missing, ", "))
	}
	return Settings{os.Getenv(required[0]), os.Getenv(required[1]), os.Getenv(required[2])}, nil
}

func loadEnvironment() error {
	dir, e := os.Getwd()
	if e != nil {
		return e
	}
	for {
		path := filepath.Join(dir, ".env")
		_, e = os.Stat(path)
		if e == nil {
			if e = loadDotEnv(path); e != nil {
				return e
			}
			break
		}
		if !os.IsNotExist(e) {
			return e
		}
		parent := filepath.Dir(dir)
		if parent == dir {
			break
		}
		dir = parent
	}
	return nil
}

var envReference = regexp.MustCompile(`\$\{([A-Za-z_][A-Za-z0-9_]*)(?::-([^}]*))?\}`)

func loadDotEnv(path string) error {
	f, e := os.Open(path)
	if os.IsNotExist(e) {
		return nil
	}
	if e != nil {
		return e
	}
	defer f.Close()
	scanner := bufio.NewScanner(f)
	scanner.Buffer(make([]byte, 4096), 1024*1024)
	for scanner.Scan() {
		line := strings.TrimSpace(scanner.Text())
		if line == "" || strings.HasPrefix(line, "#") {
			continue
		}
		line = strings.TrimSpace(strings.TrimPrefix(line, "export "))
		key, value, ok := strings.Cut(line, "=")
		if !ok {
			continue
		}
		key = strings.TrimSpace(key)
		if key == "" {
			continue
		}
		value = strings.TrimSpace(value)
		if len(value) > 0 && (value[0] == '\'' || value[0] == '"') {
			quote := value[0]
			end := quotedEnd(value, quote)
			for end < 0 && scanner.Scan() {
				value += "\n" + scanner.Text()
				end = quotedEnd(value, quote)
			}
			if end < 0 {
				return fmt.Errorf("Unterminated quoted value for %s in %s", key, path)
			}
			value = value[1:end]
			if quote == '"' {
				value = strings.NewReplacer(`\n`, "\n", `\r`, "\r", `\t`, "\t", `\"`, `"`, `\\`, `\`).Replace(value)
			} else {
				value = strings.NewReplacer(`\'`, `'`, `\\`, `\`).Replace(value)
			}
		} else {
			for i, r := range value {
				if r == '#' && i > 0 && (value[i-1] == ' ' || value[i-1] == '\t') {
					value = strings.TrimSpace(value[:i])
					break
				}
			}
		}
		value = envReference.ReplaceAllStringFunc(value, func(ref string) string {
			m := envReference.FindStringSubmatch(ref)
			if v, ok := os.LookupEnv(m[1]); ok {
				return v
			}
			return m[2]
		})
		if _, exists := os.LookupEnv(key); !exists {
			if e = os.Setenv(key, value); e != nil {
				return e
			}
		}
	}
	return scanner.Err()
}
func quotedEnd(value string, quote byte) int {
	escaped := false
	for i := 1; i < len(value); i++ {
		if !escaped && value[i] == quote {
			return i
		}
		if !escaped && value[i] == '\\' {
			escaped = true
		} else {
			escaped = false
		}
	}
	return -1
}
