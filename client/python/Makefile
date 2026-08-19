PROTO_SRC  := $(shell find ../../proto -name '*.proto' | sort)
PROTO_OUT  := vstimd/_proto

.PHONY: build publish proto test test-integration test-e2e test-e2e-review \
        test-e2e-null typecheck docs clean

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

# The suite that renders for real, start to finish, no interaction.
test-e2e: proto
	uv run --group dev pytest $(E2E_SUITES) -v $(PYTEST_ARGS)

# The same suite, reviewed by hand: it pauses on every test with what should be
# on screen, takes vim motions to move around the suite (k back, 5j forward
# five, 42G to a number, / to search) and `f` to flag one with a note, which
# lands in e2e-review.md. Runs its own windowed server so this terminal stays
# visible; pass PYTEST_ARGS="--pause=step" to stop at every caption change.
test-e2e-review: proto
	uv run --group dev python tests/e2e/browse.py $(PYTEST_ARGS)

test-e2e-null: proto
	uv run --group dev pytest tests/e2e/test_e2e_null.py tests/e2e/test_psychopy_visual_null.py -v $(PYTEST_ARGS)

typecheck: proto
	uv run --group dev ty check

docs: proto
	uv run --group dev sphinx-build -b html docs docs/_build/html

clean:
	rm -rf dist/ .venv/ docs/_build/
