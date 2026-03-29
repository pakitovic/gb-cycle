#define GB_INTERNAL

#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

#include "gb.h"
#include "timing.h"

typedef struct {
    uint8_t current_byte;
    uint8_t bits_shifted;
    uint8_t *bytes;
    size_t length;
    size_t capacity;
} serial_capture_t;

typedef struct {
    const char *model;
    const char *rom_path;
    const char *serial_hex_out_path;
    uint64_t timeout_tcycles;
    uint64_t startup_cartridge_rtc_seconds;
    bool has_timeout_tcycles;
    bool has_timeout_frames;
    uint32_t timeout_frames;
} options_t;

static serial_capture_t serial_capture;

static void usage(const char *argv0)
{
    fprintf(stderr,
            "Usage: %s --model <dmg|mgb|cgb> --rom <path> --serial-hex-out <path> "
            "[--timeout-tcycles <n> | --timeout-frames <n>] "
            "[--startup-cartridge-rtc-seconds <n>]\n",
            argv0);
}

static bool ensure_capacity(serial_capture_t *capture)
{
    if (capture->length < capture->capacity) {
        return true;
    }

    size_t new_capacity = capture->capacity == 0 ? 16 : capture->capacity * 2;
    uint8_t *new_bytes = realloc(capture->bytes, new_capacity);
    if (!new_bytes) {
        return false;
    }

    capture->bytes = new_bytes;
    capture->capacity = new_capacity;
    return true;
}

static void serial_start(GB_gameboy_t *gb, bool bit_to_send)
{
    (void)gb;
    serial_capture.current_byte = (uint8_t)((serial_capture.current_byte << 1) | (bit_to_send ? 1 : 0));
    serial_capture.bits_shifted++;
    if (serial_capture.bits_shifted == 8) {
        if (!ensure_capacity(&serial_capture)) {
            fprintf(stderr, "failed to grow serial capture buffer\n");
            exit(2);
        }
        serial_capture.bytes[serial_capture.length++] = serial_capture.current_byte;
        serial_capture.current_byte = 0;
        serial_capture.bits_shifted = 0;
    }
}

static bool serial_end(GB_gameboy_t *gb)
{
    (void)gb;
    return true;
}

static bool parse_u64(const char *text, uint64_t *out)
{
    char *end = NULL;
    unsigned long long parsed = strtoull(text, &end, 10);
    if (!text[0] || !end || *end != 0) {
        return false;
    }
    *out = parsed;
    return true;
}

static bool parse_u32(const char *text, uint32_t *out)
{
    uint64_t parsed = 0;
    if (!parse_u64(text, &parsed) || parsed > UINT32_MAX) {
        return false;
    }
    *out = (uint32_t)parsed;
    return true;
}

static bool parse_arguments(int argc, char **argv, options_t *options)
{
    memset(options, 0, sizeof(*options));

    for (int i = 1; i < argc; i++) {
        if (strcmp(argv[i], "--model") == 0 && i + 1 < argc) {
            options->model = argv[++i];
        }
        else if (strcmp(argv[i], "--rom") == 0 && i + 1 < argc) {
            options->rom_path = argv[++i];
        }
        else if (strcmp(argv[i], "--serial-hex-out") == 0 && i + 1 < argc) {
            options->serial_hex_out_path = argv[++i];
        }
        else if (strcmp(argv[i], "--timeout-tcycles") == 0 && i + 1 < argc) {
            options->has_timeout_tcycles = parse_u64(argv[++i], &options->timeout_tcycles);
            if (!options->has_timeout_tcycles) {
                return false;
            }
        }
        else if (strcmp(argv[i], "--timeout-frames") == 0 && i + 1 < argc) {
            options->has_timeout_frames = parse_u32(argv[++i], &options->timeout_frames);
            if (!options->has_timeout_frames) {
                return false;
            }
        }
        else if (strcmp(argv[i], "--startup-cartridge-rtc-seconds") == 0 && i + 1 < argc) {
            if (!parse_u64(argv[++i], &options->startup_cartridge_rtc_seconds)) {
                return false;
            }
        }
        else {
            return false;
        }
    }

    if (!options->model || !options->rom_path || !options->serial_hex_out_path) {
        return false;
    }

    if (options->has_timeout_tcycles == options->has_timeout_frames) {
        return false;
    }

    return true;
}

static GB_model_t parse_model(const char *model)
{
    if (strcmp(model, "dmg") == 0) {
        return GB_MODEL_DMG_B;
    }
    if (strcmp(model, "mgb") == 0) {
        return GB_MODEL_MGB;
    }
    if (strcmp(model, "cgb") == 0) {
        return GB_MODEL_CGB_E;
    }

    fprintf(stderr, "unsupported model %s\n", model);
    exit(2);
}

static void apply_skip_boot_startup(GB_gameboy_t *gb)
{
    gb->boot_rom_finished = true;
    gb->pc = 0x100;
    gb->sp = 0xFFFE;
    gb->ime = false;
    gb->interrupt_enable = 0;
    gb->io_registers[GB_IO_BANK] = 1;
    gb->io_registers[GB_IO_IF] = 0xE1;
}

static void apply_startup_rtc(GB_gameboy_t *gb, uint64_t seconds)
{
    if (seconds == 0) {
        return;
    }

    GB_set_rtc_mode(gb, GB_RTC_MODE_ACCURATE);
    memset(&gb->rtc_real, 0, sizeof(gb->rtc_real));
    memset(&gb->rtc_latched, 0, sizeof(gb->rtc_latched));

    uint64_t days = seconds / 86400ULL;
    seconds %= 86400ULL;
    gb->rtc_real.hours = (uint8_t) (seconds / 3600ULL);
    seconds %= 3600ULL;
    gb->rtc_real.minutes = (uint8_t) (seconds / 60ULL);
    gb->rtc_real.seconds = (uint8_t) (seconds % 60ULL);
    gb->rtc_real.days = (uint8_t) (days & 0xFF);
    gb->rtc_real.high = (uint8_t) ((days >> 8) & 1);
    gb->rtc_latched = gb->rtc_real;
    gb->last_rtc_second = (uint64_t) time(NULL);
}

static bool write_serial_hex_file(const char *path)
{
    FILE *file = fopen(path, "w");
    if (!file) {
        return false;
    }

    for (size_t i = 0; i < serial_capture.length; i++) {
        if (fprintf(file, "%02X", serial_capture.bytes[i]) < 0) {
            fclose(file);
            return false;
        }
    }

    fclose(file);
    return true;
}

int main(int argc, char **argv)
{
    options_t options;
    if (!parse_arguments(argc, argv, &options)) {
        usage(argv[0]);
        return 2;
    }

    GB_gameboy_t gb;
    GB_init(&gb, parse_model(options.model));
    GB_set_rendering_disabled(&gb, true);
    GB_set_serial_transfer_bit_start_callback(&gb, serial_start);
    GB_set_serial_transfer_bit_end_callback(&gb, serial_end);

    if (GB_load_rom(&gb, options.rom_path)) {
        fprintf(stderr, "failed to load ROM %s\n", options.rom_path);
        GB_free(&gb);
        return 2;
    }

    apply_skip_boot_startup(&gb);
    apply_startup_rtc(&gb, options.startup_cartridge_rtc_seconds);

    uint64_t remaining_tcycles = options.has_timeout_tcycles
        ? options.timeout_tcycles
        : (uint64_t) options.timeout_frames * 70224ULL;
    while (remaining_tcycles > 0) {
        unsigned elapsed = GB_run(&gb);
        if (elapsed == 0) {
            break;
        }
        if (elapsed >= remaining_tcycles) {
            remaining_tcycles = 0;
        }
        else {
            remaining_tcycles -= elapsed;
        }
    }

    if (!write_serial_hex_file(options.serial_hex_out_path)) {
        fprintf(stderr, "failed to write %s\n", options.serial_hex_out_path);
        GB_free(&gb);
        return 2;
    }

    GB_free(&gb);
    free(serial_capture.bytes);
    return 0;
}
