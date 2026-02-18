ARG GOLANG_VERSION=1.26
ARG ALPINE_VERSION=3.21

# Stage 1: Build using Go's native cross-compilation
FROM --platform=$BUILDPLATFORM golang:${GOLANG_VERSION}-alpine${ALPINE_VERSION} AS builder

ARG TARGETARCH
ARG TARGETOS=linux

RUN apk add --no-cache git

COPY . /app
WORKDIR /app

RUN go mod download && \
    CGO_ENABLED=0 GOOS=${TARGETOS} GOARCH=${TARGETARCH} go build -ldflags="-s -w" -o /zap ./cmd/zap-sidecar

# Stage 2: Runtime
FROM alpine:${ALPINE_VERSION}

LABEL maintainer="dev@hanzo.ai"
LABEL org.opencontainers.image.source="https://github.com/hanzoai/zap-sidecar"
LABEL org.opencontainers.image.description="Hanzo ZAP - ZAP protocol sidecar for infrastructure services"

RUN apk add --no-cache ca-certificates tzdata && \
    adduser -u 1000 -S -D -H zap

COPY --from=builder /zap /usr/local/bin/zap

USER 1000

ENTRYPOINT ["zap"]

HEALTHCHECK --interval=15s --timeout=3s --start-period=5s --retries=3 \
    CMD wget -qO- http://localhost:9651/health || exit 1
