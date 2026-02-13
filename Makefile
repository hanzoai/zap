.PHONY: build test docker clean

IMAGE := ghcr.io/hanzoai/zap
TAG := latest

build:
	go build -o bin/zap ./cmd/zap-sidecar

test:
	go test -v -race ./...

docker:
	docker build -t $(IMAGE):$(TAG) .

push: docker
	docker push $(IMAGE):$(TAG)

clean:
	rm -rf bin/
