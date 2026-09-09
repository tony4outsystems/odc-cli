package odc

import "fmt"

type APIError struct {
	Message string
}

func (e APIError) Error() string {
	return e.Message
}

func errorf(format string, args ...any) error {
	return APIError{Message: fmt.Sprintf(format, args...)}
}
