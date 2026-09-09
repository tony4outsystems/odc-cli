package odc

import (
	"fmt"
	"time"
)

func (c *Client) ListRevisions(key string) ([]object, error) {
	return c.paginated("asset-repository", "/assets/"+esc(key)+"/revisions", nil)
}

func (c *Client) GetRevision(key string, revision int) (object, error) {
	return c.call("GET", "asset-repository", fmt.Sprintf("/assets/%s/revisions/%d", esc(key), revision), nil, nil)
}

func deployedAppRows(items []object, env, search string) []object {
	rows := []object{}
	for _, app := range items {
		for _, deployment := range objects(app["deployments"]) {
			if str(deployment["environmentKey"]) != env {
				continue
			}
			if search != "" && !contains(app["key"], search) && !contains(deployment["name"], search) {
				continue
			}
			row := compactMap(deployment, []string{"name", "revision", "tag", "url", "environmentKey", "deploymentKey", "deploymentDateTime"})
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
