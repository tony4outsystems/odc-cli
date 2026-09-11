package odc

import (
	"bytes"
	"encoding/json"
	"io"
	"mime/multipart"
	"net/http"
	"net/textproto"
	"os"
	"path/filepath"
)

// UploadSourceCode creates a new asset revision from an OML/XIF file using the
// Asset Repository API's multipart CreateAssetRevision endpoint. The asset key
// is derived server-side from the file's embedded module key.
func (c *Client) UploadSourceCode(filePath string) (object, error) {
	file, err := os.Open(filePath)
	if err != nil {
		return nil, err
	}
	defer file.Close()

	var body bytes.Buffer
	writer := multipart.NewWriter(&body)
	creationPart, err := writer.CreatePart(mimeHeader("creationRequest", "", "application/json"))
	if err != nil {
		return nil, err
	}
	if _, err = creationPart.Write([]byte("{}")); err != nil {
		return nil, err
	}
	filePart, err := writer.CreatePart(mimeHeader("assetFile", filepath.Base(filePath), "application/octet-stream"))
	if err != nil {
		return nil, err
	}
	if _, err = io.Copy(filePart, file); err != nil {
		return nil, err
	}
	if err = writer.Close(); err != nil {
		return nil, err
	}

	token, err := c.Token()
	if err != nil {
		return nil, err
	}
	req, err := http.NewRequest("POST", c.Settings.TenantOrigin()+apiPaths["asset-repository"]+"/assets", &body)
	if err != nil {
		return nil, err
	}
	req.Header.Set("Authorization", "Bearer "+token)
	req.Header.Set("Content-Type", writer.FormDataContentType())
	req.Header.Set("Accept", "application/json")

	response, err := c.HTTP.Do(req)
	if err != nil {
		return nil, err
	}
	defer response.Body.Close()
	data, err := io.ReadAll(response.Body)
	if err != nil {
		return nil, err
	}
	if response.StatusCode >= 400 {
		return nil, errorf("POST %s failed with %d: %s", req.URL, response.StatusCode, data)
	}
	var result object
	decoder := json.NewDecoder(bytes.NewReader(data))
	decoder.UseNumber()
	if err = decoder.Decode(&result); err != nil {
		return nil, errorf("Invalid JSON response from %s: %v", req.URL, err)
	}
	return result, nil
}

func mimeHeader(field, filename, contentType string) textproto.MIMEHeader {
	disposition := `form-data; name="` + field + `"`
	if filename != "" {
		disposition += `; filename="` + filename + `"`
	}
	return textproto.MIMEHeader{"Content-Disposition": {disposition}, "Content-Type": {contentType}}
}
