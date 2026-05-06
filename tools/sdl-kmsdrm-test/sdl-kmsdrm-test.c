/*
 * sdl-kmsdrm-test.c
 *
 * Minimal SDL2 KMSDRM smoke-test.
 *
 * Run with:
 *   sudo SDL_VIDEODRIVER=kmsdrm ./sdl-kmsdrm-test
 *
 * What it does:
 *   1. Forces the SDL KMS/DRM backend (bypasses any running Wayland/X session).
 *   2. Opens a fullscreen window on the first display.
 *   3. Cycles through solid colours (red → green → blue) for ~3 seconds each,
 *      printing a counter to stderr every frame so you can tell it is running.
 *   4. Exits cleanly after the colour cycle (or immediately on Escape / q).
 *
 * After exit we want to see the original desktop / console return.
 * If the screen stays blank or shows "no signal" that is the SDL team's
 * reference bug to compare against our Vulkan implementation.
 *
 * Build:
 *   make          (uses the Makefile in this directory)
 */

#include <SDL2/SDL.h>
#include <stdio.h>
#include <stdlib.h>

/* 3 phases × 3 seconds at ~60 fps */
#define PHASE_FRAMES  180
#define NUM_PHASES    3

static const struct { Uint8 r, g, b; const char *name; } phases[NUM_PHASES] = {
    { 200,  40,  40, "RED"   },
    {  40, 200,  40, "GREEN" },
    {  40,  40, 200, "BLUE"  },
};

int main(void)
{
    /* Force SDL to use the KMS/DRM backend even if Wayland/X11 is running.
     * Must be set before SDL_Init. */
    SDL_setenv("SDL_VIDEODRIVER", "kmsdrm", 1);

    fprintf(stderr, "[sdl-kmsdrm-test] SDL_VIDEODRIVER=kmsdrm\n");
    fprintf(stderr, "[sdl-kmsdrm-test] SDL version %d.%d.%d\n",
            SDL_MAJOR_VERSION, SDL_MINOR_VERSION, SDL_PATCHLEVEL);

    if (SDL_Init(SDL_INIT_VIDEO | SDL_INIT_EVENTS) < 0) {
        fprintf(stderr, "[sdl-kmsdrm-test] SDL_Init failed: %s\n", SDL_GetError());
        return 1;
    }

    fprintf(stderr, "[sdl-kmsdrm-test] video driver: %s\n", SDL_GetCurrentVideoDriver());

    /* Print every display SDL found. */
    int num_displays = SDL_GetNumVideoDisplays();
    fprintf(stderr, "[sdl-kmsdrm-test] %d display(s) found\n", num_displays);
    for (int i = 0; i < num_displays; i++) {
        SDL_DisplayMode mode;
        SDL_GetCurrentDisplayMode(i, &mode);
        fprintf(stderr, "  display %d: %dx%d @ %d Hz  name=%s\n",
                i, mode.w, mode.h, mode.refresh_rate,
                SDL_GetDisplayName(i));
    }

    /* Create a fullscreen window on display 0. */
    SDL_Window *win = SDL_CreateWindow(
        "sdl-kmsdrm-test",
        SDL_WINDOWPOS_UNDEFINED_DISPLAY(0),
        SDL_WINDOWPOS_UNDEFINED_DISPLAY(0),
        0, 0,   /* size ignored for fullscreen */
        SDL_WINDOW_FULLSCREEN_DESKTOP
    );
    if (!win) {
        fprintf(stderr, "[sdl-kmsdrm-test] SDL_CreateWindow failed: %s\n", SDL_GetError());
        SDL_Quit();
        return 1;
    }

    SDL_Renderer *ren = SDL_CreateRenderer(win, -1,
        SDL_RENDERER_ACCELERATED | SDL_RENDERER_PRESENTVSYNC);
    if (!ren) {
        /* Fall back to software renderer — still useful for the cleanup test. */
        fprintf(stderr, "[sdl-kmsdrm-test] hardware renderer unavailable (%s), "
                "trying software\n", SDL_GetError());
        ren = SDL_CreateRenderer(win, -1, SDL_RENDERER_SOFTWARE);
    }
    if (!ren) {
        fprintf(stderr, "[sdl-kmsdrm-test] SDL_CreateRenderer failed: %s\n", SDL_GetError());
        SDL_DestroyWindow(win);
        SDL_Quit();
        return 1;
    }

    SDL_RendererInfo rinfo;
    SDL_GetRendererInfo(ren, &rinfo);
    fprintf(stderr, "[sdl-kmsdrm-test] renderer: %s\n", rinfo.name);
    fprintf(stderr, "[sdl-kmsdrm-test] starting colour cycle — press Escape/q to quit early\n");

    int phase = 0;
    int frame  = 0;
    int running = 1;

    while (running) {
        /* -- Event handling -- */
        SDL_Event ev;
        while (SDL_PollEvent(&ev)) {
            if (ev.type == SDL_QUIT) {
                running = 0;
            } else if (ev.type == SDL_KEYDOWN) {
                SDL_Keycode k = ev.key.keysym.sym;
                if (k == SDLK_ESCAPE || k == SDLK_q) {
                    running = 0;
                }
            }
        }

        /* -- Render -- */
        SDL_SetRenderDrawColor(ren,
            phases[phase].r, phases[phase].g, phases[phase].b, 255);
        SDL_RenderClear(ren);
        SDL_RenderPresent(ren);

        fprintf(stderr, "\r[sdl-kmsdrm-test] phase %d/%d (%s)  frame %d/%d   ",
                phase + 1, NUM_PHASES, phases[phase].name,
                frame + 1, PHASE_FRAMES);

        /* -- Advance phase -- */
        frame++;
        if (frame >= PHASE_FRAMES) {
            frame = 0;
            phase++;
            if (phase >= NUM_PHASES) {
                running = 0;   /* all phases done, exit cleanly */
            } else {
                fprintf(stderr, "\n");
            }
        }
    }

    fprintf(stderr, "\n[sdl-kmsdrm-test] done — cleaning up\n");

    /* SDL cleanup — this is the sequence under test.
     * SDL_DestroyRenderer → SDL_DestroyWindow → SDL_Quit triggers:
     *   KMSDRM_DestroySurfaces  (drmModeSetCrtc to original fb)
     *   KMSDRM_VideoQuit
     *   SDL_EVDEV_Quit / kbd_vt_quit (VT_SETMODE VT_AUTO, KDSKBMODE restore)
     */
    SDL_DestroyRenderer(ren);
    SDL_DestroyWindow(win);

    fprintf(stderr, "[sdl-kmsdrm-test] SDL_Quit...\n");
    SDL_Quit();
    fprintf(stderr, "[sdl-kmsdrm-test] exited cleanly\n");

    return 0;
}
