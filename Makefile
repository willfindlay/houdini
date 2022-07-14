# SPDX-License-Identifier: Apache-2.0
#
# Houdini  A container escape artist
# Copyright (c) 2022  William Findlay
#
# February 25, 2022  William Findlay  Created this.
#

CONTAINER_ENGINE = docker
IMAGE = houdini
TAG   = latest

QUIET = @

.PHONY: all
all: | test

.PHONY: test
test:
	cargo test

.PHONY: image
image:
	$(CONTAINER_ENGINE) build -t $(IMAGE):$(TAG) .
	$(QUIET)echo "Push like this when ready:"
	$(QUIET)echo "    $(CONTAINER_ENGINE) push $(IMAGE):$(TAG)"
