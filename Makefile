.PHONY: tidy

# This repo no longer ships a binary. The former `cmd/zap-sidecar` has been
# removed; ZAP is implemented natively inside each backend service (sql, kv,
# docdb, datastore, etc.) and exposed on TCP/9999 by the service itself.
#
# See README.md for the deprecation notice and migration history.

tidy:
	go mod tidy
