use std::time::Instant;

use shan_shui::render::{render_shapes, RenderOptions};
use shan_shui::rng::Seed;
use shan_shui::scene::{Scene, SceneOptions};

fn main() {
    let mut scene = Scene::new(SceneOptions {
        seed: Some(Seed::Int(42)),
        theme: "qinglv".into(),
        width: 1200,
        height: 800,
        camera: 0.0,
        chunk_width: 512,
        vegetation: true,
        architecture: 1.0,
        people: 1.0,
        boats: 1.0,
        dust: 1.0,
        water_style: "river".into(),
        sky: true,
        sun: true,
        mist: true,
        birds: 5,
        title: "山水清音".into(),
        seal: "清".into(),
        title_size: 24.0,
    })
    .unwrap();

    for (i, camera) in [0.0, 200.0, 600.0, 1200.0, 2400.0, 4000.0]
        .iter()
        .enumerate()
    {
        let started = Instant::now();
        let xmin = camera - 600.0;
        let shapes = scene.render(Some(xmin), Some(xmin + 1200.0), Some(1200.0), Some(800.0));
        let gen = started.elapsed();
        let started = Instant::now();
        let bitmap = render_shapes(
            &shapes,
            &RenderOptions {
                xmin,
                width: 1200,
                height: 800,
                scale: 1.0,
                paper_color: Some(scene.palette.paper),
                supersample: 1,
                paper: true,
                grain: true,
                finish: shan_shui::finish::Finish::Wash,
                grain_seed: 1234,
            },
        );
        let raster = started.elapsed();
        println!(
            "camera {camera:>6.0} shapes {:>6} gen {:>6.0} ms raster {:>6.0} ms ({})",
            shapes.len(),
            gen.as_secs_f64() * 1000.0,
            raster.as_secs_f64() * 1000.0,
            i
        );
        let _ = bitmap;
    }
}
