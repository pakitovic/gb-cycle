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
    uint16_t address;
    uint8_t value;
} memory_write_t;

typedef struct {
    const char *model;
    const char *rom_path;
    const char *serial_hex_out_path;
    const char *framebuffer_pgm_out_path;
    const char *probe_json_out_path;
    uint64_t probe_interval_tcycles;
    uint64_t timeout_tcycles;
    uint64_t startup_cartridge_rtc_seconds;
    bool has_probe_interval_tcycles;
    bool has_timeout_tcycles;
    bool has_timeout_frames;
    uint32_t timeout_frames;
    memory_write_t startup_writes[4096];
    size_t startup_write_count;
} options_t;

static serial_capture_t serial_capture;
static uint32_t framebuffer[160 * 144];

static void usage(const char *argv0)
{
    fprintf(stderr,
            "Usage: %s --model <dmg|mgb|cgb> --rom <path> "
            "(--serial-hex-out <path> | --framebuffer-pgm-out <path> | --probe-json-out <path>) "
            "[--write-memory <address> <value>]... "
            "[--probe-interval-tcycles <n>] "
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
    unsigned long long parsed = strtoull(text, &end, 0);
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

static bool parse_u16(const char *text, uint16_t *out)
{
    uint64_t parsed = 0;
    if (!parse_u64(text, &parsed) || parsed > UINT16_MAX) {
        return false;
    }
    *out = (uint16_t) parsed;
    return true;
}

static bool parse_u8(const char *text, uint8_t *out)
{
    uint64_t parsed = 0;
    if (!parse_u64(text, &parsed) || parsed > UINT8_MAX) {
        return false;
    }
    *out = (uint8_t) parsed;
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
        else if (strcmp(argv[i], "--framebuffer-pgm-out") == 0 && i + 1 < argc) {
            options->framebuffer_pgm_out_path = argv[++i];
        }
        else if (strcmp(argv[i], "--probe-json-out") == 0 && i + 1 < argc) {
            options->probe_json_out_path = argv[++i];
        }
        else if (strcmp(argv[i], "--probe-interval-tcycles") == 0 && i + 1 < argc) {
            options->has_probe_interval_tcycles = parse_u64(argv[++i], &options->probe_interval_tcycles);
            if (!options->has_probe_interval_tcycles || options->probe_interval_tcycles == 0) {
                return false;
            }
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
        else if (strcmp(argv[i], "--write-memory") == 0 && i + 2 < argc) {
            if (options->startup_write_count >= sizeof(options->startup_writes) / sizeof(options->startup_writes[0])) {
                return false;
            }
            memory_write_t *write = &options->startup_writes[options->startup_write_count];
            if (!parse_u16(argv[++i], &write->address) || !parse_u8(argv[++i], &write->value)) {
                return false;
            }
            options->startup_write_count++;
        }
        else {
            return false;
        }
    }

    if (!options->model || !options->rom_path ||
        (!options->serial_hex_out_path && !options->framebuffer_pgm_out_path && !options->probe_json_out_path)) {
        return false;
    }

    if ((options->probe_json_out_path != NULL) != options->has_probe_interval_tcycles) {
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

static uint32_t rgb_encode(GB_gameboy_t *gb, uint8_t r, uint8_t g, uint8_t b)
{
    (void)gb;
    return ((uint32_t) r << 16) | ((uint32_t) g << 8) | b;
}

static uint8_t header_checksum(const GB_gameboy_t *gb)
{
    if (!gb->rom || gb->rom_size <= 0x14D) {
        return 0xFF;
    }
    return gb->rom[0x14D];
}

static void apply_skip_boot_startup(GB_gameboy_t *gb)
{
    gb->boot_rom_finished = true;
    gb->a = gb->model == GB_MODEL_MGB ? 0xFF : 0x01;
    gb->f = header_checksum(gb) == 0 ? 0x80 : 0xB0;
    gb->b = 0x00;
    gb->c = 0x13;
    gb->d = 0x00;
    gb->e = 0xD8;
    gb->h = 0x01;
    gb->l = 0x4D;
    gb->pc = 0x100;
    gb->sp = 0xFFFE;
    gb->ime = false;
    gb->interrupt_enable = 0;
    gb->io_registers[GB_IO_JOYP] = 0xCF;
    gb->io_registers[GB_IO_SB] = 0x00;
    gb->io_registers[GB_IO_SC] = 0x7E;
    gb->io_registers[GB_IO_DIV] = 0xAB;
    /* Mirror gb-cycle's synthetic SkipBoot divider phase; DIV reads come from
       SameBoy's internal counter, not io_registers[GB_IO_DIV]. */
    gb->div_counter = 0xABC8;
    gb->io_registers[GB_IO_TIMA] = 0x00;
    gb->io_registers[GB_IO_TMA] = 0x00;
    gb->io_registers[GB_IO_TAC] = 0xF8;
    gb->io_registers[GB_IO_IF] = 0xE1;
    gb->io_registers[GB_IO_LCDC] = 0x91;
    gb->io_registers[GB_IO_STAT] = 0x85;
    gb->io_registers[GB_IO_SCY] = 0x00;
    gb->io_registers[GB_IO_SCX] = 0x00;
    gb->io_registers[GB_IO_LY] = 0x00;
    gb->io_registers[GB_IO_LYC] = 0x00;
    gb->io_registers[GB_IO_DMA] = 0xFF;
    gb->io_registers[GB_IO_BGP] = 0xFC;
    gb->io_registers[GB_IO_WY] = 0x00;
    gb->io_registers[GB_IO_WX] = 0x00;
    gb->io_registers[GB_IO_BANK] = 1;
    gb->current_line = 0;
    gb->ly_for_comparison = 0;
    gb->cycles_for_line = 0;
    gb->position_in_line = 0;
    gb->stat_interrupt_line = false;
    gb->cgb_double_speed = false;
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

static void apply_startup_memory_writes(GB_gameboy_t *gb, const options_t *options)
{
    for (size_t i = 0; i < options->startup_write_count; i++) {
        GB_write_memory(gb, options->startup_writes[i].address, options->startup_writes[i].value);
    }
}

static bool write_serial_hex_file(const char *path)
{
    if (!path) {
        return true;
    }
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

static bool write_framebuffer_pgm_file(const char *path, GB_gameboy_t *gb)
{
    if (!path) {
        return true;
    }

    unsigned width = GB_get_screen_width(gb);
    unsigned height = GB_get_screen_height(gb);
    if (width == 0 || height == 0 || width * height > sizeof(framebuffer) / sizeof(framebuffer[0])) {
        return false;
    }

    FILE *file = fopen(path, "wb");
    if (!file) {
        return false;
    }

    if (fprintf(file, "P5\n%u %u\n255\n", width, height) < 0) {
        fclose(file);
        return false;
    }
    for (unsigned i = 0; i < width * height; i++) {
        uint32_t pixel = framebuffer[i];
        uint8_t red = (uint8_t) (pixel >> 16);
        uint8_t green = (uint8_t) (pixel >> 8);
        uint8_t blue = (uint8_t) pixel;
        uint8_t shade = (uint8_t) ((red * 77u + green * 150u + blue * 29u) >> 8);
        if (fputc(shade, file) == EOF) {
            fclose(file);
            return false;
        }
    }

    fclose(file);
    return true;
}

static uint64_t fnv1a64_update(uint64_t hash, const uint8_t *bytes, size_t len)
{
    for (size_t i = 0; i < len; i++) {
        hash ^= bytes[i];
        hash *= 1099511628211ULL;
    }
    return hash;
}

static uint64_t fnv1a64_bytes(const uint8_t *bytes, size_t len)
{
    return fnv1a64_update(14695981039346656037ULL, bytes, len);
}

static uint64_t direct_access_hash(GB_gameboy_t *gb, GB_direct_access_t access)
{
    size_t size = 0;
    uint16_t bank = 0;
    uint8_t *bytes = GB_get_direct_access(gb, access, &size, &bank);
    if (!bytes || size == 0) {
        return fnv1a64_bytes(NULL, 0);
    }
    return fnv1a64_bytes(bytes, size);
}

static uint64_t framebuffer_rank_hash(GB_gameboy_t *gb)
{
    unsigned width = GB_get_screen_width(gb);
    unsigned height = GB_get_screen_height(gb);
    if (width == 0 || height == 0 || width * height > sizeof(framebuffer) / sizeof(framebuffer[0])) {
        return fnv1a64_bytes(NULL, 0);
    }

    bool present[256] = {false};
    uint8_t shades[160 * 144];
    unsigned pixel_count = width * height;
    for (unsigned i = 0; i < pixel_count; i++) {
        uint32_t pixel = framebuffer[i];
        uint8_t red = (uint8_t) (pixel >> 16);
        uint8_t green = (uint8_t) (pixel >> 8);
        uint8_t blue = (uint8_t) pixel;
        uint8_t shade = (uint8_t) ((red * 77u + green * 150u + blue * 29u) >> 8);
        shades[i] = shade;
        present[shade] = true;
    }

    uint8_t ranks[256];
    uint8_t next_rank = 0;
    for (int shade = 255; shade >= 0; shade--) {
        if (present[shade]) {
            ranks[shade] = next_rank++;
        }
    }

    uint64_t hash = 14695981039346656037ULL;
    for (unsigned i = 0; i < pixel_count; i++) {
        uint8_t rank = ranks[shades[i]];
        hash = fnv1a64_update(hash, &rank, 1);
    }
    return hash;
}

static void write_serial_hex_json(FILE *file)
{
    for (size_t i = 0; i < serial_capture.length; i++) {
        fprintf(file, "%02X", serial_capture.bytes[i]);
    }
}

static bool write_probe_json_line(FILE *file, GB_gameboy_t *gb, uint64_t elapsed_tcycles)
{
    uint16_t af = ((uint16_t) gb->a << 8) | gb->f;
    uint16_t bc = ((uint16_t) gb->b << 8) | gb->c;
    uint16_t de = ((uint16_t) gb->d << 8) | gb->e;
    uint16_t hl = ((uint16_t) gb->h << 8) | gb->l;
    int written = fprintf(
        file,
        "{\"t_cycles\":%llu,\"pc\":%u,\"sp\":%u,\"af\":%u,\"bc\":%u,\"de\":%u,\"hl\":%u,"
        "\"ime\":%s,\"div\":%u,\"tima\":%u,\"tma\":%u,\"tac\":%u,"
        "\"interrupt_flags\":%u,\"interrupt_enable\":%u,"
        "\"lcdc\":%u,\"stat\":%u,\"ly\":%u,\"line_dot\":%u,"
        "\"scy\":%u,\"scx\":%u,\"lyc\":%u,\"bgp\":%u,\"obp0\":%u,\"obp1\":%u,\"wy\":%u,\"wx\":%u,"
        "\"vram_hash\":\"%016llx\",\"oam_hash\":\"%016llx\",\"wram_hash\":\"%016llx\",\"hram_hash\":\"%016llx\","
        "\"framebuffer_hash\":\"%016llx\",\"serial_hex\":\"",
        (unsigned long long) elapsed_tcycles,
        gb->pc,
        gb->sp,
        af,
        bc,
        de,
        hl,
        gb->ime ? "true" : "false",
        (unsigned) (gb->div_counter >> 8),
        gb->io_registers[GB_IO_TIMA],
        gb->io_registers[GB_IO_TMA],
        gb->io_registers[GB_IO_TAC],
        gb->io_registers[GB_IO_IF],
        gb->interrupt_enable,
        gb->io_registers[GB_IO_LCDC],
        gb->io_registers[GB_IO_STAT],
        gb->current_line,
        gb->cycles_for_line,
        gb->io_registers[GB_IO_SCY],
        gb->io_registers[GB_IO_SCX],
        gb->io_registers[GB_IO_LYC],
        gb->io_registers[GB_IO_BGP],
        gb->io_registers[GB_IO_OBP0],
        gb->io_registers[GB_IO_OBP1],
        gb->io_registers[GB_IO_WY],
        gb->io_registers[GB_IO_WX],
        (unsigned long long) direct_access_hash(gb, GB_DIRECT_ACCESS_VRAM),
        (unsigned long long) direct_access_hash(gb, GB_DIRECT_ACCESS_OAM),
        (unsigned long long) direct_access_hash(gb, GB_DIRECT_ACCESS_RAM),
        (unsigned long long) direct_access_hash(gb, GB_DIRECT_ACCESS_HRAM),
        (unsigned long long) framebuffer_rank_hash(gb));
    if (written < 0) {
        return false;
    }
    write_serial_hex_json(file);
    return fprintf(file, "\"}\n") >= 0;
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
    GB_random_set_enabled(false);
    GB_set_rendering_disabled(&gb, options.framebuffer_pgm_out_path == NULL && options.probe_json_out_path == NULL);
    if (options.framebuffer_pgm_out_path || options.probe_json_out_path) {
        GB_set_pixels_output(&gb, framebuffer);
        GB_set_rgb_encode_callback(&gb, rgb_encode);
        GB_set_color_correction_mode(&gb, GB_COLOR_CORRECTION_DISABLED);
    }
    GB_set_serial_transfer_bit_start_callback(&gb, serial_start);
    GB_set_serial_transfer_bit_end_callback(&gb, serial_end);

    if (GB_load_rom(&gb, options.rom_path)) {
        fprintf(stderr, "failed to load ROM %s\n", options.rom_path);
        GB_free(&gb);
        return 2;
    }

    apply_skip_boot_startup(&gb);
    apply_startup_rtc(&gb, options.startup_cartridge_rtc_seconds);
    apply_startup_memory_writes(&gb, &options);

    FILE *probe_file = NULL;
    if (options.probe_json_out_path) {
        probe_file = fopen(options.probe_json_out_path, "w");
        if (!probe_file) {
            fprintf(stderr, "failed to write %s\n", options.probe_json_out_path);
            GB_free(&gb);
            return 2;
        }
        if (!write_probe_json_line(probe_file, &gb, 0)) {
            fprintf(stderr, "failed to write %s\n", options.probe_json_out_path);
            fclose(probe_file);
            GB_free(&gb);
            return 2;
        }
    }

    uint64_t remaining_tcycles = options.has_timeout_tcycles
        ? options.timeout_tcycles
        : (uint64_t) options.timeout_frames * 70224ULL;
    uint64_t elapsed_tcycles = 0;
    uint64_t next_probe_tcycles = options.has_probe_interval_tcycles
        ? options.probe_interval_tcycles
        : 0;
    uint64_t last_probe_tcycles = 0;
    while (remaining_tcycles > 0) {
        unsigned elapsed = GB_run(&gb);
        if (elapsed == 0) {
            break;
        }
        elapsed_tcycles += elapsed;
        if (elapsed >= remaining_tcycles) {
            remaining_tcycles = 0;
        }
        else {
            remaining_tcycles -= elapsed;
        }
        if (probe_file) {
            while (next_probe_tcycles != 0 && elapsed_tcycles >= next_probe_tcycles) {
                if (!write_probe_json_line(probe_file, &gb, elapsed_tcycles)) {
                    fprintf(stderr, "failed to write %s\n", options.probe_json_out_path);
                    fclose(probe_file);
                    GB_free(&gb);
                    return 2;
                }
                last_probe_tcycles = elapsed_tcycles;
                next_probe_tcycles += options.probe_interval_tcycles;
            }
        }
    }

    if (probe_file) {
        if (elapsed_tcycles != last_probe_tcycles) {
            if (!write_probe_json_line(probe_file, &gb, elapsed_tcycles)) {
                fprintf(stderr, "failed to write %s\n", options.probe_json_out_path);
                fclose(probe_file);
                GB_free(&gb);
                return 2;
            }
        }
        fclose(probe_file);
    }

    if (!write_serial_hex_file(options.serial_hex_out_path)) {
        fprintf(stderr, "failed to write %s\n", options.serial_hex_out_path);
        GB_free(&gb);
        return 2;
    }
    if (!write_framebuffer_pgm_file(options.framebuffer_pgm_out_path, &gb)) {
        fprintf(stderr, "failed to write %s\n", options.framebuffer_pgm_out_path);
        GB_free(&gb);
        return 2;
    }

    GB_free(&gb);
    free(serial_capture.bytes);
    return 0;
}
