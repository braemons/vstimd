PROTO_SRC  := $(shell find ../../proto -name '*.proto' | sort)
PROTO_OUT  := vstimd/_proto

.PHONY: build publish proto test test-integration test-e2e test-e2e-review \
        test-e2e-step test-e2e-null typecheck docs clean

# Extra arguments for the e2e targets, e.g. make test-e2e-step PYTEST_ARGS="-k grating"
PYTEST_ARGS ?=
E2E_SUITES  := tests/e2e/test_e2e.py tests/e2e/test_psychopy_visual.py

proto:
	uv run --group dev python -m grpc_tools.protoc \
		--proto_path=../../proto \
		--python_out=$(PROTO_OUT) \
		--pyi_out=$(PROTO_OUT) \
		$(PROTO_SRC)

build: proto
	uv build

publish:
	uv publish

test: proto
	uv pip install -r tests/unit/requirements-psychopy.txt
	uv pip install psychopy --no-deps
	uv run --group dev pytest tests/unit/

test-integration:
	@echo "Not implemented yet: integration tests with MockServer"

test-e2e: proto
	uv run --group dev pytest $(E2E_SUITES) -v $(PYTEST_ARGS)

# Watched runs. Both wait for a keypress rather than a fixed dwell, and both
# take notes: `f` at the prompt flags what is on screen as wrong and writes it,
# with the id and the pytest node, to e2e-review.md at the end of the run.
#   test-e2e-review — one pause per test
#   test-e2e-step   — a pause at every caption change, i.e. step by step
# Run these from a terminal you can type into, with the server windowed (see
# README) so the prompt is not hidden behind a fullscreen stimulus window.
test-e2e-review: proto
	uv run --group dev pytest $(E2E_SUITES) -v --pause=test $(PYTEST_ARGS)

test-e2e-step: proto
	uv run --group dev pytest $(E2E_SUITES) -v --pause=step $(PYTEST_ARGS)

test-e2e-null: proto
	uv run --group dev pytest tests/e2e/test_e2e_null.py tests/e2e/test_psychopy_visual_null.py -v $(PYTEST_ARGS)

typecheck: proto
	uv run --group dev ty check

docs: proto
	uv run --group dev sphinx-build -b html docs docs/_build/html

clean:
	rm -rf dist/ .venv/ docs/_build/
