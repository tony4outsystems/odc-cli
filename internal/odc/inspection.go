package odc

import (
	"fmt"
	"io"
	"net/http"
	"os"
	"time"
)

func (c *Client) ListRevisions(key string) ([]object, error) {
	return c.paginated("asset-repository", "/assets/"+esc(key)+"/revisions", nil)
}

func (c *Client) GetRevision(key string, revision int) (object, error) {
	return c.call("GET", "asset-repository", fmt.Sprintf("/assets/%s/revisions/%d", esc(key), revision), nil, nil)
}

func (c *Client) GetRevisionSourceCode(key string, revision int) (object, error) {
	return c.call("GET", "asset-repository", fmt.Sprintf("/assets/%s/revisions/%d/source-code", esc(key), revision), nil, nil)
}

// DownloadSourceCode fetches the revision's source-code metadata, downloads the
// binary from the returned URL, and writes it to output. It returns the bytes written.
func (c *Client) DownloadSourceCode(key string, revision int, output string) (int64, error) {
	meta, err := c.GetRevisionSourceCode(key, revision)
	if err != nil {
		return 0, err
	}
	sourceURL, err := requireString(meta["sourceCodeBinaryUrl"], "sourceCodeBinaryUrl")
	if err != nil {
		return 0, err
	}
	req, err := http.NewRequest("GET", sourceURL, nil)
	if err != nil {
		return 0, err
	}
	response, err := c.HTTP.Do(req)
	if err != nil {
		return 0, err
	}
	defer response.Body.Close()
	if response.StatusCode >= 400 {
		data, _ := io.ReadAll(response.Body)
		return 0, errorf("GET %s failed with %d: %s", sourceURL, response.StatusCode, data)
	}
	file, err := os.Create(output)
	if err != nil {
		return 0, err
	}
	defer file.Close()
	return io.Copy(file, response.Body)
}

func deployedAppRows(items []object, env, search string) []object {
	rows := []object{}
	for _, app := range items {
		for _, deployment := range objects(app["deployments"]) {
			if env != "" && str(deployment["environmentKey"]) != env {
				continue
			}
			if search != "" && !contains(app["key"], search) && !contains(deployment["name"], search) {
				continue
			}
			row := compactMap(deployment, []string{"name", "revision", "tag", "url", "environmentKey", "deploymentDateTime"})
			row["key"], row["type"] = app["key"], app["type"]
			rows = append(rows, row)
		}
	}
	return rows
}

func (c *Client) analyze(key, command string, o Options) error {
	path := "/deletion-analyses"
	body := object{"assetKey": key}
	if command == "analyze-deployment" {
		path = "/deployment-analyses"
		env, err := c.Resolve(o.Env, "environment")
		if err != nil {
			return err
		}
		var revision int
		if o.Revision != nil {
			revision = *o.Revision
		} else {
			revision, err = c.LatestRevision(key)
			if err != nil {
				return err
			}
		}
		body["environmentKey"], body["revision"] = env, revision
	}
	started, err := c.call("POST", "dependency-management", path, body, nil)
	if err != nil {
		return err
	}
	analysisKey, err := requireString(started["analysisKey"], "analysis key")
	if err != nil {
		return err
	}
	if o.NoWait {
		return PrintResult(started)
	}
	deadline := time.Now().Add(o.Timeout)
	lastStatus := ""
	for {
		result, err := c.call("GET", "dependency-management", path+"/"+esc(analysisKey), nil, nil)
		if err != nil {
			return err
		}
		status := str(result["processStatus"])
		if status != lastStatus {
			printlnLocked("Analysis %s: %s", analysisKey, status)
			lastStatus = status
		}
		switch status {
		case "Finished":
			return PrintResult(result)
		case "Failed":
			if err := PrintResult(result); err != nil {
				return err
			}
			return errorf("Analysis %s failed: %v", analysisKey, result["error"])
		case "InProgress":
		default:
			return errorf("Analysis %s returned unexpected processStatus %q", analysisKey, status)
		}
		remaining := time.Until(deadline)
		if remaining <= 0 {
			return errorf("Timed out waiting for analysis %s", analysisKey)
		}
		time.Sleep(min(o.Interval, remaining))
	}
}
