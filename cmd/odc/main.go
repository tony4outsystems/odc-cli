package main

import (
	"fmt"
	"os"

	"github.com/tony4outsystems/odc-cli/internal/odc"
)

func main() {
	if err := odc.Run(os.Args[1:]); err != nil {
		fmt.Fprintf(os.Stderr, "error: %v\n", err)
		os.Exit(1)
	}
}
