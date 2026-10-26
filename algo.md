# How the landscape is grown

This document describes the generator and renderer behind Shan Shui Studio: how a seed becomes a composition, how the composition becomes geometry, and how the geometry becomes a PNG or SVG. It follows the same order as the code.

The guiding constraint is **determinism**. Every run with the same seed and options must produce exactly the same shapes in exactly the same order. There is no global random state, no clock read during composition, and no dependence on iteration order of a hash map. The few places that use a time-based seed do so only when the caller deliberately omits a seed.

---

## 1. Seeds and random streams

### 1.1 A seed is a JSON value

A seed is either an integer or a string:

```rust
pub enum Seed {
    Int(i64),
    Text(String),
}
```

The seed is converted to its JSON literal (`42` or `"moon-gate"`), then Base64-encoded. The resulting bytes are interpreted as little-endian base-128 digits and accumulated modulo `M`:

```
z = sum(byte[i] * 128^i) mod M
```

`M` is the product of two primes:

```
P = 999979
Q = 999983
M = P * Q = 999962000357
```

This mirrors the Python reference and gives integer and text seeds the same treatment.

### 1.2 Blum-Blum-Shub

The internal state is advanced by squaring modulo `M`:

```
s[n+1] = s[n]^2 mod M
u[n]   = s[n] / M
```

The result `u[n]` is in `[0, 1)`. The generator is not used for cryptography; a Blum-Blum-Shub sequence is simply a compact, deterministic source of uniform values that is easy to reproduce in another language.

Before the first value is returned, the state is advanced ten times. The initial state is found by adding a counter to the hash until the value is coprime with `M` (not divisible by `P` or `Q`, not `0` or `1`).

### 1.3 Labelled child streams

Every part of the painting receives its own generator:

```rust
let base = Rng::new(Some(seed));
let noise = base.child("noise");
let brush = base.child("brush");
```

`child(label)` constructs a new seed from the parent's seed text and the label:

```
"<parent-seed>|<label>"
```

and hashes it again. The labels are deliberately Python-repr-shaped, because the Python reference uses the same strings:

```
('chunk', 3)
('item', 7, 'mount')
('mist', 3)
('sun', -350.0)
```

This is what makes the composition stable under local changes. If a tree gains an extra random call, it only shifts the stream of that tree's item; the mountains in another chunk are untouched.

**Important:** the order of calls inside a stream still matters. Reordering two `random()` calls, or adding one before existing calls, changes every subsequent value in that stream. When porting or editing generator code, preserve call order unless you intend to change the artwork.

---

## 2. Noise

The noise field is a faithful port of the Perlin-style `noise()` from p5.js, including its table size and wrapping rules:

- 4096 table entries (`PERLIN_SIZE = 4095`, inclusive),
- x/y wrapping at 16 and z wrapping at 256,
- four octaves,
- amplitude falloff of 0.5,
- quintic interpolation.

The table itself is filled from the `noise` child stream. This is critical for parity: the noise is not a separate library with its own seeding scheme; it is part of the painting's seed.

Noise is used for:

- placement of mountain massifs,
- ridge displacement and surface texture,
- rock and tree clustering,
- water and mist modulation,
- the "empty column" test that allows flat mountains,
- small colour and density decisions.

---

## 3. The world

The painting has a fixed vertical extent and an effectively infinite horizontal extent:

```rust
const WORLD_HEIGHT: f64 = 800.0;
```

The camera is the horizontal centre of the view. The CLI passes a `camera` and a window width; the Studio keeps its own camera within `-4000..4000` world units.

### 3.1 Chunks

The world is generated in 512-unit-wide chunks:

```
chunk i covers [i * 512, (i + 1) * 512)
```

Each chunk owns a `Scene` child stream labelled `('chunk', i)`. Inside it, `plan_range` decides what exists. Items are then expanded one by one with labels such as `('item', k, 'mount')`.

Chunks are cached in a `HashMap<i32, Chunk>`. Rendering a window builds only the chunks it needs (plus one on either side for oversized geometry). This is what makes moving the camera cheap.

### 3.2 Planning a range

`plan_range` samples a noise-based mountain mask every 5 world units. The mask is:

```
z(x) = max(noise(x * 0.03, 0, 0) - 0.55, 0) * 2
```

A point is a local maximum when `z > 0.3` and no neighbour within a small radius has a higher value. Local maxima become mountain items, with a random horizontal offset and a vertical anchor around `y = 300`. A minimum spacing rule prevents two mountains from collapsing into the same spot.

Other items are placed by different rules:

- **Distant mountains** every 1000 units, around `y = 280`.
- **Flat mountains** in columns that contain no mountain massif, with low probability, stacked upward.
- **Boats** with 20% probability per sample, spaced at least 400 units apart.
- **Mist** per chunk, at `y = 560`, when enabled.
- **Trees, huts, pavilions, towers, rocks and people** are not separate planner items; they are spawned by the mountain generator at sampled ridge positions and gated by density controls.

The planner returns a list of `PlanItem { tag, x, y, h }`. The `tag` selects the generator.

---

## 4. The painting grammar

The whole scene is built from five primitives:

| Primitive | Contents |
| --- | --- |
| `Poly` | point list, optional fill, optional stroke, stroke width |
| `Circle` | centre, radius, optional fill/stroke |
| `GradientRect` | rectangle with a two-stop linear gradient |
| `Text` | position, string, size, colour, rotation, family, anchor |
| `Seal` | square, size, text, fill and text colour |

Colours carry alpha. The palettes use depth-aware shading:

```
color(depth) = base.mix(haze, (1 - depth) * amount)
```

Items further back receive more haze, which produces aerial perspective without a separate fog pass.

### 4.1 Mountains

`mountain()` builds a stack of ridge rings. Each ring is a polygon strip with noise-displaced edges, triangulated into smaller facets. The generator then paints:

- dark base and silhouette,
- ridge strokes,
- interior texture lines,
- foot and rock details,
- dense tree clusters at selected ridge rows,
- huts, pavilions and pagodas at sampled positions,
- transmission towers at rarer noise thresholds,
- rocks along the base.

`flat_mount()` builds the lower, broader landforms from a parameterised cosine-like profile, while `dist_mount()` creates the pale silhouettes on the horizon. All three use the same palette depth rules.

Density gates are applied where objects are spawned. For example, an architecture candidate is kept only when:

```rust
if ctx.rng.random() >= ctx.architecture { continue; }
```

The RNG call still happens even when the object is skipped, so the density slider changes how many objects appear without reflowing the rest of the chunk's random sequence. The same pattern is used for people, boats, dust, trees and birds.

### 4.2 Trees

The tree library contains eight species (pines, broadleaf trees, shrubs and bare branches) plus `branch`, `bark`, `barkify` and leaf helpers. A tree is a recursive branch skeleton expanded into tapered polygons and foliage clusters. Tree colour is chosen from the palette's foliage list with a small brightness jitter, then hazed by depth.

Trees are placed on mountain rows where a noise condition is met, with spacing and size jitter. Vegetation can be disabled entirely; when it is, those generator calls are skipped at the scene level rather than drawn invisibly.

### 4.3 Architecture and people

Architecture includes huts, boxes, decorated roofs, railings, multi-storey pavilions, pagodas, boats and transmission towers. Each is a small construction kit of polygons and strokes: roof curves, posts, railings, steps and tile lines.

People are simplified figures made from hats, sticks and body polygons. At typical viewing sizes they read as small marks of scale and habitation rather than detailed characters.

### 4.4 Water, mist and sky

`water()` draws wave strokes across a river or lake; `sea()` and `lake()` provide wide and bounded variants. `mist()` draws soft translucent bands. The scene adds a large vertical gradient over the lower part of the frame when mist is enabled.

The sky is a vertical `GradientRect` using the palette's `sky_top` and `sky_bottom`. The sun is a stack of twelve translucent circles plus a solid core, which produces a soft halo without a blur pass. Birds are two short strokes each.

### 4.5 Inscription and seal

The title is a `Text` shape at:

```
x = xmin + 46
y = 64
```

The seal is a 30-unit square below it:

```
x = xmin + 46
y = 64 + title_size + 22
```

Because these are scene shapes, they are exported to both PNG and SVG at the same position. The Studio's panorama overlay reproduces the same layout at the current zoom (see §6.2).

---

## 5. Composition and rendering

### 5.1 Window rendering

`render_window(xmin, xmax, height, sun_anchor, birds_anchor)` assembles a frame:

1. Paint the sky gradient.
2. Paint the sun halo and core.
3. Paint the birds.
4. Fetch chunks covering the window plus one chunk of margin.
5. Cull items more than one chunk plus 400 units outside the window.
6. Sort items by `(z, y, insertion order)`.
7. Append each item's shapes.
8. Paint the mist gradient.
9. Append the title and seal.

The sort is stable and uses explicit tie-breakers, so shape order is deterministic even when two items share a z and y.

### 5.2 PNG rasterisation

The rasteriser approximates Pillow's behaviour:

1. Create a bitmap at `width * supersample` by `height * supersample`.
2. Draw aliased polygons with a scanline fill.
3. Draw strokes as thick quads with round joins.
4. Draw circles and ellipse caps.
5. Draw gradients by interpolating vertically or horizontally per pixel.
6. Rasterise text as coverage using the CJK font (see §5.4).
7. Downsample to the requested size with a separable Lanczos-3 filter.
8. Apply paper grain, then the finish preset.

`supersample` is clamped to `1..=8`. The CLI default is 2, which gives 2× linear resolution before filtering.

#### Paper grain

Grain is generated from a small deterministic PRNG seeded from the seed string. For each pixel:

```
u1, u2  = two uniform values
g       = sqrt(-2 ln u1) * cos(2π u2)      // Box-Muller
noise   = clamp(128 + 7 * g, 0, 255)
grain   = 232 + floor(noise * 23 / 255)
factor  = grain / 255
out     = round(orig * 0.35 + (orig * factor) * 0.65)
```

This mirrors Pillow's `effect_noise` with a mean of 128 and a small sigma, then multiplies the paper. Grain is applied after downsampling, so it is fine-grained rather than blurred by the Lanczos filter. It is skipped when `paper` or `grain` is off.

#### Finish presets

Finishes are per-pixel filters:

| Preset | Operation |
| --- | --- |
| `wash` | no change |
| `crisp` | contrast and saturation around 128, both × 1.06 |
| `vivid` | saturation × 1.32, then contrast × 1.08 |
| `vintage` | 62% original + 38% sepia matrix |
| `noir` | luminance, contrast around 128 × 1.15, neutral grey |

All channels are clamped to `0..255` and rounded. The SVG path keeps the original palette and does not embed these raster grades; the PNG is the finished artefact.

### 5.3 SVG serialisation

The SVG writer consumes the same shape list as the rasteriser:

- `Poly` → `<polygon>` with optional `stroke` and `stroke-width`
- `Circle` → `<circle>`
- `GradientRect` → `<rect>` referencing a generated `<linearGradient>`
- `Text` → `<text>` with font size, family, anchor and optional rotation
- `Seal` → a rounded `<rect>` and centred `<text>`

The viewport is:

```xml
<svg viewBox="xmin 0 width height" width="output_width" height="output_height">
```

so scene units stay 1:1 while the exported pixel dimensions can be larger. Numbers are formatted to two decimals with trailing zeros removed, which keeps diffs readable and matches the Python writer. The paper layer is an optional background rect. Grain is an SVG `feTurbulence` filter applied to a translucent multiply rectangle.

Text uses a CJK font stack in the SVG (`SimSun`, `Songti SC`, `STSong`, `serif`), so vector text remains selectable and editable in a browser or vector editor.

### 5.4 Fonts and the seal

The raster path uses `ab_glyph` with the first available font from:

- Windows: `simsun.ttc`, `msyh.ttc`, `simhei.ttf`, `arialuni.ttf`, then Arial;
- the display UI uses the bundled/system sans stack.

Glyphs are rasterised into a coverage buffer, then composited with alpha. Rotated text is drawn into a layer and sampled with bilinear interpolation so the edges stay smooth. The seal is a filled rounded square with the character centred in a contrasting cream colour.

The SVG path does not embed fonts. If a viewer lacks a CJK font, it will substitute; the PNG path is the reliable way to get an exact inscription.

---

## 6. The Studio

The Studio is a thin realtime layer around the same generator. It never forks the painting logic.

### 6.1 Preview pipeline

There is one long-lived preview worker. It owns a cached `Scene` and a cached shape list, so changing only the finish or camera does not rebuild the world.

- **Draft**: half the preview dimensions, no supersampling. Starts 28 ms after a change, or immediately while the pointer is down.
- **Final**: full preview dimensions at 2× supersampling. Starts 180 ms after the last change, provided the pointer is up.
- **Cancellation**: every job has a sequence number. The worker checks the current sequence between shapes and returns early when it is stale. A final render that is interrupted does not block the next draft.
- **Camera-only changes**: after the panorama is ready, a camera move skips the draft and waits for the final pass. The panorama supplies the intermediate frames.

The preview texture is uploaded to egui and drawn with a UV window for zoom. The canvas is a fixed fit rectangle; zoom crops the texture around its centre.

### 6.2 The cached panorama

A normal preview only covers the current viewport. To make camera travel instant, the Studio also renders one wide strip covering every reachable camera position:

```
half_view   = WORLD_HEIGHT * (16/9) / 2
xmin        = -4000 - half_view - 64
xmax        = +4000 + half_view + 64
output      = (xmax - xmin) * 0.9 wide, WORLD_HEIGHT * 0.9 tall
```

The panorama is rendered at 0.9 scale with no supersampling. It uses the same finish and grain as the preview; those are baked in. The title and seal are **not** baked in, and camera, zoom, aspect and inscription are excluded from the cache key. Those can change without re-rendering the strip.

For a camera `c` and zoom `z`, the UV window is:

```
span   = xmax - xmin
centre = (clamp(c) - xmin) / span
half_u = (world_width / z / span) / 2
half_v = (0.5 / z)
u      = [centre - half_u, centre + half_u]
v      = [0.5 - half_v, 0.5 + half_v]
```

The image is drawn into the viewport rect with that UV rectangle. Moving the camera changes only `centre`; zooming changes both the UV size and the scale. The title and seal are painted on top as a fixed overlay:

```
scale = viewport_width * zoom / world_width
title at (viewport_left + 46 * scale, viewport_top + 64 * scale)
seal  at (viewport_left + 46 * scale,
          viewport_top + (64 + title_size + 22) * scale)
```

This is the same classic inset as the baked composition, scaled with the current view. Because the overlay is independent of the panorama, the inscription stays crisp and does not slide with the landscape.

The panorama is regenerated after 420 ms of stable non-camera settings. Camera travel itself never triggers a regeneration.

### 6.3 Export

Export runs on its own worker, separate from previews. It:

1. Takes a snapshot of the current `PreviewSpec` and export settings.
2. Builds the full scene at the target aspect and resolution.
3. Renders the PNG at the selected long edge with 2× supersampling.
4. Serialises the SVG from the same shape list.
5. Writes the requested files and reports paths, shape count and elapsed time.

The long edge is 1600, 2400 or 3000 px. Landscape aspects set the width; portrait sets the height. The composited title, seal, sun, birds and mist are included in both formats.

The native Save dialog only selects a **base path**; the export button commits the files. `.png` and `.svg` suffixes are stripped from the chosen path and re-added per selected format, so `painting.png` and `painting` both produce `painting.png` / `painting.svg`.

### 6.4 UI state and persistence rules

- The export base path follows the seed until the user edits the **Save to** field or chooses a path in the dialog.
- **Use seed name** restores tracking.
- Changing the seed while tracking updates the path immediately.
- Camera changes are not written into the seed or path.
- The release build uses the Windows GUI subsystem; the console is hidden unless built with `--features gui-console`.

---

## 7. Determinism and parity checks

The following invariants are enforced by tests and by construction:

- The same seed and `SceneOptions` produce the same `Shape` list, in the same order.
- The RNG child label for a component depends only on the seed and the component's position in the plan, not on wall-clock time or hash-map iteration.
- Chunk generation is independent: building chunk 3 before chunk 1 does not change either chunk.
- The preview and export use the same `Scene`, so the exported composition matches what the Studio shows (aside from preview resolution and the fixed panorama overlay).
- SVG numbers use a fixed two-decimal format, so repeated exports diff cleanly.

The repository includes:

```bash
cargo test --lib
cargo test --bin shan-shui-gui
```

The library tests cover RNG determinism and normalisation, Base64 compatibility, point-in-polygon, triangulation and render cancellation. The GUI tests cover camera clamping and rail mapping, panorama UV coverage across every camera/zoom/aspect combination, preview and export sizing, output path rules and seed slugs.

For a deeper comparison against the Python reference, use:

```bash
cargo run --release --example dump_scene -- --seed 42 --width 480 --height 320
```

The output is a stable text dump of the scene. Compare it with the Python `tools/dump_scene.py` output for the same seed and dimensions to inspect shape-level parity.

---

## 8. Performance notes

The generator is CPU-bound and allocation-heavy, but the work is highly parallelisable at the job level.

- A typical 1200×800 scene contains roughly 15,000–50,000 shapes, depending on seed and densities.
- A preview draft at half resolution is fast enough to provide feedback during a drag.
- A final preview at 2× supersampling is the main interactive cost; stale finals are cancelled between shapes.
- The panorama is larger than a single frame but is generated once per composition, not per camera move.
- Export is intentionally isolated on a worker so the Studio remains interactive while a 3000 px PNG is written.
- The rasteriser is single-threaded per job. Running preview, panorama and export simultaneously is bounded by the number of workers (one each), not by the number of cores.

If you need faster batch rendering, run multiple CLI processes with different seeds. If you need faster interactive rendering, reduce the preview supersampling or add a lower-resolution draft path; the worker boundary and cancellation checks are already in place.

---

## 9. Extending the painting

### Add a palette

Add a `p()` entry in `src/color.rs` with the required colour roles, then add its display name in `gui_main.rs`. The theme list and CLI `--list-themes` update automatically from the palette table.

### Add a finish

Add a variant to `Finish`, implement its pixel operation in `apply_finish`, and add a label in the Studio. Finish is part of the panorama key and the preview spec, so changing it invalidates the cached strip.

### Add a shape primitive

Extend `Shape`, implement rasterisation in `render/raster.rs` and serialisation in `render/svg.rs`, and make sure the deterministic shape order is preserved. If the primitive affects composition, add it to the scene key used by the workers.

### Add a scene object

Add a planner tag or spawn it from an existing generator, give it a stable child-stream label, and gate it with a density value if users should be able to turn it down. Never consume randomness conditionally in a way that depends on UI state beyond the documented density gates: the stream must stay stable for a given seed.

---

## 10. Known boundaries

- The native Save dialog is Windows-only. Other platforms can edit the base path directly.
- SVG text depends on the viewer's CJK fonts; the PNG path embeds the rendered glyphs.
- The panorama trades a little sharpness for instant travel; the final preview replaces it when the camera settles.
- Grain and finish are raster effects. The SVG is pure geometry with its own optional paper-noise filter, so the two formats are visually close but not pixel-identical.
- The camera range is deliberately bounded to `-4000..4000`; the underlying world is infinite, but the cached panorama and the rail are not.

The point of the system is not to hide these boundaries, but to keep the painting itself deterministic, inspectable and pleasant to explore.
