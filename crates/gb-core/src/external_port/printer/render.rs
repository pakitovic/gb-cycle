use super::{PRINTER_PAGE_WIDTH_PIXELS, PRINTER_TILE_WIDTH, PrintedPage, PrinterPrintArgs};

pub(super) fn render_printed_page(
    image_buffer: &[u8],
    print_args: PrinterPrintArgs,
) -> PrintedPage {
    let total_tiles = image_buffer.len() / 16;
    let tile_rows = total_tiles.div_ceil(PRINTER_TILE_WIDTH);
    let height = (tile_rows * 8) as u16;
    let mut pixels = vec![0; usize::from(PRINTER_PAGE_WIDTH_PIXELS) * usize::from(height)];

    for tile_index in 0..total_tiles {
        let tile_base = tile_index * 16;
        let tile_x = tile_index % PRINTER_TILE_WIDTH;
        let tile_y = tile_index / PRINTER_TILE_WIDTH;

        for row in 0..8 {
            let plane_lo = image_buffer[tile_base + row * 2];
            let plane_hi = image_buffer[tile_base + row * 2 + 1];

            for bit in 0..8 {
                let shift = 7 - bit;
                let color = ((plane_lo >> shift) & 1) | (((plane_hi >> shift) & 1) << 1);
                let x = tile_x * 8 + bit;
                let y = tile_y * 8 + row;
                let pixel_index = y * usize::from(PRINTER_PAGE_WIDTH_PIXELS) + x;
                pixels[pixel_index] = color;
            }
        }
    }

    PrintedPage {
        width: PRINTER_PAGE_WIDTH_PIXELS,
        height,
        pixels,
        print_args,
    }
}
