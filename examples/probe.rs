use shan_shui::noise::Noise;
use shan_shui::rng::Rng;

fn main() {
    let mut rng = Rng::from_i64(42);
    let vals: Vec<String> = (0..5).map(|_| format!("{:.16}", rng.random())).collect();
    println!("base {}", vals.join(","));
    let noise = Noise::new(Rng::from_i64(42).child("noise"));
    println!("noise {:.16}", noise.noise(-1024.0 * 0.03, 0.0, 0.0));
    println!("noise -1000 {:.16}", noise.noise(-1000.0 * 0.03, 0.0, 0.0));
    let mut chunk = Rng::from_i64(42).child("brush").child("('chunk', -2)");
    let vals: Vec<String> = (0..5).map(|_| format!("{:.16}", chunk.random())).collect();
    println!("chunk {}", vals.join(","));
}
