//go:build windows

// arnawsl -- a small command-line harness for the Stage A WSL2 workspace
// adapter, so the create -> harden -> verify -> run -> destroy pipeline can be
// exercised and watched without the rest of Arna.
//
//	go run ./cmd/arnawsl detect          # is WSL2 usable? (read-only)
//	go run ./cmd/arnawsl demo            # full lifecycle on a throwaway ws, then clean up
//	go run ./cmd/arnawsl provision <id>  # create + harden + verify (leaves it)
//	go run ./cmd/arnawsl desktop <id>    # setup + start the desktop, print the browser URL
//	go run ./cmd/arnawsl status <id>
//	go run ./cmd/arnawsl destroy <id>
package main

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"

	"arna-services/wsl"
)

func main() {
	if len(os.Args) < 2 {
		fmt.Println("usage: arnawsl <detect|demo|provision|status|destroy> [id]")
		os.Exit(2)
	}
	dataDir := filepath.Join(os.TempDir(), "arna-workspaces")

	switch os.Args[1] {
	case "detect":
		if _, err := wsl.New(dataDir); err != nil {
			fmt.Println("NOT available:", err)
			os.Exit(1)
		}
		names, err := wsl.List()
		check(err)
		fmt.Println("WSL2 available. Arna-owned workspaces:", names)

	case "demo":
		a, err := wsl.New(dataDir)
		check(err)
		id := "demo01"

		fmt.Println("1/5 provisioning a throwaway workspace (downloads a ~3MB rootfs once)…")
		rep, err := a.Provision(id)
		if err != nil {
			fmt.Println("   provision failed:", err)
			printJSON("isolation", rep)
			os.Exit(1)
		}
		fmt.Println("   provisioned and passed isolation checks.")

		// Guarantee cleanup no matter what happens next.
		defer func() {
			fmt.Println("5/5 destroying the workspace…")
			if err := a.Destroy(id); err != nil {
				fmt.Println("   destroy failed (remove manually with: wsl --unregister", wsl.DistroName(id)+"):", err)
			} else {
				fmt.Println("   destroyed. Machine left clean.")
			}
		}()

		fmt.Println("2/5 starting it…")
		rep, err = a.Start(id)
		check(err)
		fmt.Println("3/5 verifying it's actually sealed:")
		printJSON("   isolation", rep)

		fmt.Println("4/5 status:")
		st, err := a.Status(id)
		check(err)
		printJSON("   status", st)

	case "provision":
		id := arg(2)
		a, err := wsl.New(dataDir)
		check(err)
		rep, err := a.Provision(id)
		printJSON("isolation", rep)
		check(err)
		fmt.Println("provisioned:", wsl.DistroName(id))

	case "desktop":
		id := arg(2)
		a, err := wsl.New(dataDir)
		check(err)
		// Make sure it exists (provision if not), then set up + start the desktop.
		if st, _ := a.Status(id); !st.Exists {
			fmt.Println("provisioning", wsl.DistroName(id), "…")
			if _, err := a.Provision(id); err != nil {
				check(err)
			}
		}
		fmt.Println("starting it…")
		if _, err := a.Start(id); err != nil {
			check(err)
		}
		fmt.Println("installing the display stack (first time downloads a few MB)…")
		check(a.SetupDisplay(id))
		fmt.Println("bringing up the desktop…")
		info, err := a.StartSession(id, "")
		check(err)
		fmt.Println()
		fmt.Println("  ✓ workspace desktop is live. Open this in a browser:")
		fmt.Println("    " + info.URL)
		fmt.Println()
		fmt.Println("  when done:  go run ./cmd/arnawsl destroy " + id)

	case "status":
		a, err := wsl.New(dataDir)
		check(err)
		st, err := a.Status(arg(2))
		check(err)
		printJSON("status", st)

	case "destroy":
		a, err := wsl.New(dataDir)
		check(err)
		check(a.Destroy(arg(2)))
		fmt.Println("destroyed")

	default:
		fmt.Println("unknown command:", os.Args[1])
		os.Exit(2)
	}
}

func arg(i int) string {
	if len(os.Args) <= i {
		fmt.Println("missing <id> argument")
		os.Exit(2)
	}
	return os.Args[i]
}

func check(err error) {
	if err != nil {
		fmt.Println("error:", err)
		os.Exit(1)
	}
}

func printJSON(label string, v any) {
	b, _ := json.MarshalIndent(v, "", "  ")
	fmt.Println(label+":", string(b))
}
