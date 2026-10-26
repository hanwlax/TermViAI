//! Windows UI-only grayscale rasterization. Shaping and terminal rasterization
//! remain independent; unsupported faces and failed glyphs use the configured
//! rasterizer. Grayscale masks work in both OpenGL and WebGPU without RGB fringes.
use super::{FontRasterizer, RasterizedGlyph};
use crate::locator::FontDataSource;
use crate::parser::ParsedFont;
use crate::units::PixelLength;
use anyhow::{ensure, Context};
use std::ptr;
use wezterm_color_types::linear_u8_to_srgb8;
use winapi::shared::windef::RECT;
use winapi::um::dcommon::DWRITE_MEASURING_MODE_NATURAL;
use winapi::um::dwrite::*;
use winapi::um::dwrite_1::DWRITE_TEXT_ANTIALIAS_MODE_GRAYSCALE;
use winapi::um::dwrite_2::{IDWriteFactory2, DWRITE_GRID_FIT_MODE_ENABLED};
use winapi::Interface;

struct Factory(*mut IDWriteFactory2);
impl Drop for Factory {
    fn drop(&mut self) {
        unsafe {
            (*self.0).Release();
        }
    }
}
struct Analysis(*mut IDWriteGlyphRunAnalysis);
impl Drop for Analysis {
    fn drop(&mut self) {
        unsafe {
            (*self.0).Release();
        }
    }
}

pub struct NativeUiRasterizer {
    factory: Factory,
    face: dwrote::FontFace,
    fallback: Box<dyn FontRasterizer>,
}

pub fn with_native_ui(
    parsed: &ParsedFont,
    fallback: Box<dyn FontRasterizer>,
) -> Box<dyn FontRasterizer> {
    match native_face(parsed) {
        Ok((factory, face)) => Box::new(NativeUiRasterizer {
            factory,
            face,
            fallback,
        }),
        Err(err) => {
            log::debug!("UI native font unavailable; keeping configured rasterizer: {err:#}");
            fallback
        }
    }
}

fn native_face(parsed: &ParsedFont) -> anyhow::Result<(Factory, dwrote::FontFace)> {
    // Retain explicit per-face choices, variable instances, bitmap and color
    // fallback behavior rather than silently flattening them to monochrome.
    ensure!(
        parsed.handle.variation == 0
            && parsed.pixel_sizes.is_empty()
            && parsed.palettes.is_empty()
            && !parsed.assume_emoji_presentation
            && !parsed.synthesize_bold
            && !parsed.synthesize_italic
            && !parsed.synthesize_dim
            && parsed.scale.is_none()
            && parsed.freetype_load_flags.is_none()
            && parsed.freetype_load_target.is_none()
            && parsed.freetype_render_target.is_none(),
        "face requires configured rasterization"
    );
    let path = match &parsed.handle.source {
        FontDataSource::OnDisk(path) => path,
        _ => anyhow::bail!("use existing rasterizer for embedded faces"),
    };
    let file = dwrote::FontFile::new_from_path(path).context("Open UI font")?;
    let face = file
        .create_face(parsed.handle.index, DWRITE_FONT_SIMULATIONS_NONE)
        .map_err(|hr| anyhow::anyhow!("Create UI face: {hr:#x}"))?;
    let mut factory = ptr::null_mut();
    let hr = unsafe {
        DWriteCreateFactory(
            DWRITE_FACTORY_TYPE_SHARED,
            &IDWriteFactory2::uuidof(),
            &mut factory,
        )
    };
    ensure!(
        hr >= 0 && !factory.is_null(),
        "Create UI raster factory: {hr:#x}"
    );
    Ok((Factory(factory as *mut IDWriteFactory2), face))
}

impl NativeUiRasterizer {
    fn render(&self, glyph: u32, size: f64, dpi: u32) -> anyhow::Result<RasterizedGlyph> {
        ensure!(glyph <= u16::MAX as u32, "Invalid glyph index");
        let index = glyph as u16;
        let advance = 0_f32;
        let run = DWRITE_GLYPH_RUN {
            fontFace: unsafe { self.face.as_ptr() },
            fontEmSize: (size * dpi as f64 / 72.) as f32,
            glyphCount: 1,
            glyphIndices: &index,
            glyphAdvances: &advance,
            glyphOffsets: ptr::null(),
            isSideways: 0,
            bidiLevel: 0,
        };
        let mut analysis = ptr::null_mut();
        let hr = unsafe {
            (*self.factory.0).CreateGlyphRunAnalysis(
                &run,
                ptr::null(),
                DWRITE_RENDERING_MODE_NATURAL_SYMMETRIC,
                DWRITE_MEASURING_MODE_NATURAL,
                DWRITE_GRID_FIT_MODE_ENABLED,
                DWRITE_TEXT_ANTIALIAS_MODE_GRAYSCALE,
                0.,
                0.,
                &mut analysis,
            )
        };
        ensure!(hr >= 0 && !analysis.is_null(), "Analyze UI glyph: {hr:#x}");
        let analysis = Analysis(analysis);
        let mut bounds = RECT {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        };
        let hr =
            unsafe { (*analysis.0).GetAlphaTextureBounds(DWRITE_TEXTURE_ALIASED_1x1, &mut bounds) };
        ensure!(hr >= 0, "UI glyph bounds: {hr:#x}");
        let width = (bounds.right - bounds.left).max(0) as usize;
        let height = (bounds.bottom - bounds.top).max(0) as usize;
        let len = width
            .checked_mul(height)
            .context("UI glyph size overflow")?;
        ensure!(len <= 16_777_216, "UI glyph texture too large");
        let mut alpha = vec![0; len];
        if len > 0 {
            let hr = unsafe {
                (*analysis.0).CreateAlphaTexture(
                    DWRITE_TEXTURE_ALIASED_1x1,
                    &bounds,
                    alpha.as_mut_ptr(),
                    len as u32,
                )
            };
            ensure!(hr >= 0, "UI glyph coverage: {hr:#x}");
        }
        let mut data = Vec::with_capacity(len * 4);
        for coverage in alpha {
            let rgb = linear_u8_to_srgb8(coverage);
            data.extend_from_slice(&[rgb, rgb, rgb, coverage]);
        }
        Ok(RasterizedGlyph {
            data,
            width,
            height,
            bearing_x: PixelLength::new(bounds.left as f64),
            bearing_y: PixelLength::new(-(bounds.top as f64)),
            has_color: false,
            is_scaled: true,
        })
    }
}
impl FontRasterizer for NativeUiRasterizer {
    fn rasterize_glyph(&self, glyph: u32, size: f64, dpi: u32) -> anyhow::Result<RasterizedGlyph> {
        self.render(glyph, size, dpi).or_else(|err| {
            log::debug!("UI glyph {glyph} native raster failed: {err:#}; using fallback");
            self.fallback.rasterize_glyph(glyph, size, dpi)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::FontConfiguration;
    struct FallbackMarker;
    impl FontRasterizer for FallbackMarker {
        fn rasterize_glyph(&self, _: u32, _: f64, _: u32) -> anyhow::Result<RasterizedGlyph> {
            Ok(RasterizedGlyph {
                data: vec![99; 4],
                width: 1,
                height: 1,
                bearing_x: PixelLength::new(0.),
                bearing_y: PixelLength::new(1.),
                has_color: false,
                is_scaled: true,
            })
        }
    }
    #[test]
    fn unsupported_face_and_native_glyph_error_keep_the_configured_fallback() {
        let fonts =
            FontConfiguration::new(Some(config::ConfigHandle::default_config()), 96).unwrap();
        let mut face = fonts.ui_font(14, false).unwrap().clone_handles().remove(0);
        let (factory, native_face) = native_face(&face).unwrap();
        let native = NativeUiRasterizer {
            factory,
            face: native_face,
            fallback: Box::new(FallbackMarker),
        };
        assert_eq!(
            native.rasterize_glyph(u32::MAX, 10.5, 96).unwrap().data,
            vec![99; 4]
        );
        face.freetype_load_flags = Some(config::FreeTypeLoadFlags::NO_HINTING);
        let raster = with_native_ui(&face, Box::new(FallbackMarker));
        assert_eq!(
            raster.rasterize_glyph(1, 10.5, 96).unwrap().data,
            vec![99; 4]
        );
    }

    #[test]
    fn native_ui_masks_cover_small_text_at_multiple_dpis_without_changing_terminal() {
        let mut preview = image::RgbaImage::from_pixel(760, 720, image::Rgba([21, 22, 26, 255]));
        let mut row = 0;
        for dpi in [96, 120, 144, 192] {
            let fonts =
                FontConfiguration::new(Some(config::ConfigHandle::default_config()), dpi).unwrap();
            assert!(!fonts.default_font().unwrap().native_ui);
            assert!(!fonts.title_font().unwrap().native_ui);
            for bold in [false, true] {
                let font = fonts.ui_font(14, bold).unwrap();
                assert!(font.native_ui);
                let parsed = font.clone_handles().remove(0);
                let fallback = super::super::new_rasterizer(
                    config::FontRasterizerSelection::FreeType,
                    &parsed,
                    config::DisplayPixelGeometry::RGB,
                )
                .unwrap();
                let (factory, face) = native_face(&parsed).unwrap();
                let native = NativeUiRasterizer {
                    factory,
                    face,
                    fallback,
                };
                let text = format!(
                    "Keychain 209 212 Monitor  {dpi}dpi {}",
                    if bold { "600" } else { "400" }
                );
                let glyphs = font
                    .shape(
                        &text,
                        || {},
                        |_| {},
                        None,
                        wezterm_bidi::Direction::LeftToRight,
                        None,
                        None,
                    )
                    .unwrap();
                let mut different = false;
                for native_row in [false, true] {
                    let mut x = 12_f64;
                    let baseline = row * 42 + 32;
                    for info in &glyphs {
                        let before = native
                            .fallback
                            .rasterize_glyph(info.glyph_pos, 10.5, dpi as u32)
                            .unwrap();
                        let after = native.render(info.glyph_pos, 10.5, dpi as u32).unwrap();
                        different |= before.data != after.data;
                        assert_eq!(after.data.len(), after.width * after.height * 4);
                        assert!(!after.has_color);
                        let g = if native_row { after } else { before };
                        let gx = (x + info.x_offset.get() + g.bearing_x.get()).round() as i32;
                        let gy =
                            baseline - (info.y_offset.get() + g.bearing_y.get()).round() as i32;
                        for yy in 0..g.height {
                            for xx in 0..g.width {
                                let px = gx + xx as i32;
                                let py = gy + yy as i32;
                                if px >= 0 && py >= 0 && px < 760 && py < 720 {
                                    let a = g.data[(yy * g.width + xx) * 4 + 3] as f32 / 255.;
                                    let mut rgb = [0u8; 4];
                                    for c in 0..3 {
                                        rgb[c] = ([21., 22., 26.][c] * (1. - a)
                                            + [241., 242., 245.][c] * a)
                                            .round()
                                            as u8;
                                    }
                                    rgb[3] = 255;
                                    preview.put_pixel(px as u32, py as u32, image::Rgba(rgb));
                                }
                            }
                        }
                        x += info.x_advance.get();
                    }
                    row += 1;
                }
                assert!(
                    different,
                    "native raster must be exercised, not silently fall back"
                );
                // Native text has grayscale edge coverage, not binary / aliased pixels.
                let index = native.face.get_glyph_indices(&['S' as u32])[0];
                let mask = native.render(index as u32, 10.5, dpi as u32).unwrap();
                assert!(mask.data.chunks_exact(4).any(|p| p[3] > 0 && p[3] < 255));
            }
        }
        if let Ok(path) = std::env::var("TERMVIAI_UI_RASTER_PREVIEW") {
            preview.save(path).unwrap();
        }
    }
}
