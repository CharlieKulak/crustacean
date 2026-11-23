# macroquad 0.4.16, patched for Crustacean

Unchanged from crates.io except `src/text/atlas.rs` (`Atlas::cache_sprite`): a glyph
was checked against the row's end without the gap placed before it, so one landing
there doubled the font atlas (and repacked it) instead of starting a new row. Fonts
grew to 4096x4096 and 8192x8192 textures that Chromebook GPUs can't make, and the
web game went black. (The examples and tests are left out.)
