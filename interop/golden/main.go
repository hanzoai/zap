// Regenerates the golden hex constants in rust/tests/crosswire.rs from the
// canonical Go runtime, so the fixture is never hand-written:
//
//	go run ./interop/golden
//
// It prints the Rust `const` lines verbatim — paste them over the ones in
// crosswire.rs and run `make fmt` to normalise the wrapping. Run this when
// github.com/zap-proto/go changes the envelope; if the bytes move, the
// crosswire tests go red until Rust is brought back into line.
//
// The generator emits the finished `const` line rather than a bare hex
// string on purpose: transcribing hex by hand is the one way this fixture
// could silently drift away from the runtime it is supposed to pin.
package main

import (
	"encoding/hex"
	"fmt"

	"github.com/zap-proto/go/rpc"
)

// A golden case: the Rust const it produces, the doc line above it, and
// the bytes the Go runtime emits for it.
type golden struct {
	name  string
	doc   string
	bytes []byte
}

func main() {
	for _, g := range []golden{
		{
			"GO_REQ",
			`rpc.Call{Method: 7, PromiseID: 1, Target: NO_TARGET, Cap: "cap-token", Payload: "hello-zap"}`,
			rpc.BuildRequest(rpc.Call{
				Method:    7,
				PromiseID: 1,
				Target:    rpc.NoTarget,
				Cap:       []byte("cap-token"),
				Payload:   []byte("hello-zap"),
			}),
		},
		{
			"GO_REQ_EMPTY",
			`rpc.Call{Method: 2, PromiseID: 9, Target: 1, Cap: nil, Payload: nil}`,
			rpc.BuildRequest(rpc.Call{Method: 2, PromiseID: 9, Target: 1}),
		},
		{
			"GO_RESP",
			`rpc.BuildResponse(STATUS_OK, 1, "world")`,
			rpc.BuildResponse(rpc.StatusOK, 1, []byte("world")),
		},
		{
			"GO_RESP_EMPTY",
			`rpc.BuildResponse(STATUS_NOT_FOUND, 42, nil)`,
			rpc.BuildResponse(rpc.StatusNotFound, 42, nil),
		},
	} {
		fmt.Printf("/// %s\nconst %s: &str = \"%s\";\n\n",
			g.doc, g.name, hex.EncodeToString(g.bytes))
	}
}
