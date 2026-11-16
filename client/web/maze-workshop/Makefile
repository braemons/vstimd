.PHONY: build check serve
build:
	node build.mjs
check:
	node --check build.mjs
	node --check app.mjs
	node --check resizer.mjs
	node --check model.mjs
	node --check collisions.mjs
	node --check navigation.mjs
	node --check wheel.mjs
	node --check return.mjs
	node --check shortcuts.mjs
	node --check landmarks.mjs
	node --check surfaces.mjs
	node --check surface-preview.mjs
	node --check preview.mjs
	node --check measurements.mjs
	node --test shortcuts.test.mjs return.test.mjs collisions.test.mjs model.test.mjs preview.test.mjs measurements.test.mjs landmarks.test.mjs surfaces.test.mjs navigation.test.mjs wheel.test.mjs
serve:
	python3 -m http.server 8765 --bind 127.0.0.1
