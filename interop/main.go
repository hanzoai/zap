// A live Go ZAP peer, so the Rust runtime is proved against the REAL
// reference implementation rather than a hand-written fixture.
//
//	go run ./interop /path/to/zap.sock
//
// It serves one method — ordinal 1, "upper" — over the canonical
// transport framing, prints "ready" once bound, and runs until killed.
// rust/tests/interop.rs drives it.
package main

import (
	"fmt"
	"os"
	"os/signal"
	"strings"
	"syscall"

	"github.com/zap-proto/go/rpc"
	"github.com/zap-proto/go/transport"
)

func main() {
	if len(os.Args) < 2 {
		fmt.Fprintln(os.Stderr, "usage: interop <addr>")
		os.Exit(2)
	}
	addr := os.Args[1]
	network := "tcp"
	if strings.HasPrefix(addr, "/") || strings.HasPrefix(addr, "./") {
		network = "unix"
	}

	srv, err := transport.Listen(network, addr, func(env []byte) ([]byte, error) {
		call, err := rpc.ParseRequest(env)
		if err != nil {
			return nil, err
		}
		if call.Method != 1 {
			return rpc.BuildResponse(rpc.StatusNotFound, call.PromiseID, nil), nil
		}
		up := strings.ToUpper(string(call.Payload))
		return rpc.BuildResponse(rpc.StatusOK, call.PromiseID, []byte(up)), nil
	})
	if err != nil {
		fmt.Fprintln(os.Stderr, "listen:", err)
		os.Exit(1)
	}
	defer srv.Close()

	fmt.Println("ready", srv.Addr().String())
	os.Stdout.Sync()

	sig := make(chan os.Signal, 1)
	signal.Notify(sig, os.Interrupt, syscall.SIGTERM)
	<-sig
}
