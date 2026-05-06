CC      = gcc
CFLAGS  = -std=c11 -Wall -Wextra -g $(shell pkg-config --cflags sdl2)
LIBS    = $(shell pkg-config --libs sdl2)
TARGET  = sdl-kmsdrm-test

.PHONY: all clean run

all: $(TARGET)

$(TARGET): sdl-kmsdrm-test.c
	$(CC) $(CFLAGS) -o $@ $< $(LIBS)

# Run with sudo so we have permission to open /dev/dri/card* and /dev/tty
run: $(TARGET)
	sudo SDL_VIDEODRIVER=kmsdrm ./$(TARGET)

clean:
	rm -f $(TARGET)
