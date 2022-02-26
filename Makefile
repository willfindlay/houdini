CONTAINER_ENGINE = docker
IMAGE = houdini
TAG   = latest

.PHONY: image
image:
	$(CONTAINER_ENGINE) build -t $(IMAGE):$(TAG) .
	$(QUIET)echo "Push like this when ready:"
	$(QUIET)echo "$(CONTAINER_ENGINE) push $(IMAGE):$(TAG)"
