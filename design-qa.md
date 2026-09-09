# Atlas Home Logo Design QA

## Evidence

- Source visual truth: `/Users/roger/Downloads/ChatGPT Image 8 sept 2026, 14_42_37.png`
- Rendered implementation: `dist/visual-evidence/atlas-home-logo-warmup-06.png`
- Focused side-by-side comparison: `dist/visual-evidence/atlas-home-logo-comparison-06.png`
- Source dimensions: 2804x561 px; near-white canvas trimmed to the 880x205 px mark.
- Normalized source and rendered crop: 360x84 px, 1-bit, density 1.
- Firmware viewport: 480x800 px portrait, density 1.
- State: Home, Atlas connected, Library selected, populated summary snapshots.

## Full-view comparison

The 480x800 framebuffer keeps the existing topbar, Atlas wordmark, status,
section label and six-row Home navigation unchanged. The replacement mark is
centered at screen bounds x=60..419 and y=75..158, inside the existing bounded
hero zone.

## Focused-region comparison

The normalized source mark and the exact framebuffer crop are placed together
in `atlas-home-logo-comparison-06.png`. ImageMagick absolute-error comparison
reports 0 differing pixels across all 30,240 pixels.

## Required fidelity surfaces

- Fonts and typography: unchanged; no source-image text is recreated.
- Spacing and layout rhythm: the 360x84 mark is centered inside a 456x106
  firmware-local bitmap, preserving Home's surrounding vertical rhythm.
- Colors and visual tokens: exact monochrome black mark on white background.
- Image quality and asset fidelity: source canvas is trimmed, scaled once with
  Lanczos, thresholded to 1-bit and embedded directly; no drawn approximation,
  runtime decoder, SD dependency or network dependency is used.
- Copy and content: unchanged.

## Findings

No actionable P0, P1 or P2 differences remain.

## Comparison history

1. Initial comparison found a P2 raster mismatch: bits inside every packed byte
   were read in reverse order, producing serrated diagonals.
2. The reader was corrected to consume ImageMagick MONO bytes least-significant
   bit first. The post-fix focused comparison reports 0 differing pixels.

## Implementation checklist

- [x] Replace only the Home hero artwork.
- [x] Preserve the topbar's official Atlas logo and all Home behavior.
- [x] Verify exact source-to-framebuffer raster parity.
- [x] Verify focused tests, full host tests and the ESP32-S3 release target.

final result: passed
