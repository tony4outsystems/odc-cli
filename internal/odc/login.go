package odc

import (
	"encoding/json"
	"fmt"
	"net/url"
	"os"
	"path/filepath"
	"strings"

	"golang.org/x/term"
)

func configPath() (string, error) {
	home, err := os.UserHomeDir()
	if err != nil {
		return "", err
	}
	return filepath.Join(home, ".odc", "config.json"), nil
}

func promptSecret() (string, error) {
	fd := int(os.Stdin.Fd())
	if !term.IsTerminal(fd) {
		return "", errorf("login requires an interactive terminal to enter the client secret")
	}
	fmt.Fprint(os.Stderr, "Client secret: ")
	secret, err := term.ReadPassword(fd)
	fmt.Fprintln(os.Stderr)
	return string(secret), err
}

func login(tenantURL, clientID string, readSecret func() (string, error)) error {
	u, err := url.Parse(tenantURL)
	if err != nil || u.Hostname() == "" || (u.Scheme != "https" && u.Scheme != "http") || u.User != nil || u.RawQuery != "" || u.Fragment != "" {
		return errorf("tenant URL must be an absolute HTTP or HTTPS URL without credentials, query, or fragment")
	}
	if strings.TrimSpace(clientID) == "" {
		return errorf("client ID must not be empty")
	}
	secret, err := readSecret()
	if err != nil {
		return err
	}
	if strings.TrimSpace(secret) == "" {
		return errorf("client secret must not be empty")
	}
	path, err := saveSettings(Settings{tenantURL, clientID, secret})
	if err != nil {
		return err
	}
	return PrintResult(object{"configuration": path})
}

func saveSettings(s Settings) (string, error) {
	path, err := configPath()
	if err != nil {
		return "", err
	}
	if err = os.MkdirAll(filepath.Dir(path), 0700); err != nil {
		return "", err
	}
	data, err := json.MarshalIndent(s, "", "  ")
	if err != nil {
		return "", err
	}
	// Replace atomically so failures preserve the previous credentials. Temporary
	// files are owner-only, including when replacing an existing configuration.
	f, err := os.CreateTemp(filepath.Dir(path), ".config-*")
	if err != nil {
		return "", err
	}
	defer os.Remove(f.Name())
	if _, err = f.Write(append(data, '\n')); err != nil {
		f.Close()
		return "", err
	}
	if err = f.Close(); err != nil {
		return "", err
	}
	if err = os.Rename(f.Name(), path); err != nil {
		return "", err
	}
	return path, nil
}
